import { expect, test as base } from '@playwright/test';
import { once } from 'node:events';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawn, type ChildProcess } from 'node:child_process';

const ADMIN_PASSWORD = 'uma senha local muito segura';

type ServerFixture = {
	baseURL: string;
	stop: () => Promise<void>;
};

// A bounded grace period is only the escalation boundary; cleanup otherwise waits on
// the child's close event rather than sleeping or abandoning a live server.
const SHUTDOWN_GRACE_MS = 5_000;

function childClosed(process: ChildProcess) {
	process.once('error', () => undefined);
	return once(process, 'close').then(() => undefined);
}

async function waitForServerAddress(process: ChildProcess) {
	const prefix = 'LEARNVAULT_READY ';
	if (!process.stdout) throw new Error('server stdout is not available for readiness handoff');
	let output = '';
	return new Promise<string>((resolve, reject) => {
		const cleanup = () => {
			process.stdout?.off('data', onData);
			process.off('close', onClose);
			process.off('error', onError);
		};
		const onData = (chunk: Buffer | string) => {
			output += chunk.toString();
			for (const line of output.split(/\r?\n/).slice(0, -1)) {
				if (line.startsWith(prefix)) {
					cleanup();
					resolve(line.slice(prefix.length).trim());
					return;
				}
			}
			output = output.split(/\r?\n/).at(-1) ?? '';
		};
		const onClose = () => {
			cleanup();
			reject(new Error(`server exited before readiness: ${output.trim()}`));
		};
		const onError = (error: Error) => {
			cleanup();
			reject(error);
		};
		process.stdout.on('data', onData);
		process.once('close', onClose);
		process.once('error', onError);
	});
}

async function stopProcess(process: ChildProcess, closed: Promise<void>) {
	if (process.exitCode !== null || process.signalCode !== null) {
		await closed;
		return;
	}

	process.kill('SIGTERM');
	let timeout: NodeJS.Timeout | undefined;
	const exited = await Promise.race([
		closed.then(() => true),
		new Promise<boolean>((resolve) => {
			timeout = setTimeout(() => resolve(false), SHUTDOWN_GRACE_MS);
		})
	]);
	if (timeout) clearTimeout(timeout);
	if (!exited) {
		process.kill('SIGKILL');
		await closed;
	}
}

async function waitForHealth(url: string, process: ChildProcess) {
	const deadline = Date.now() + 30_000;
	while (Date.now() < deadline) {
		if (process.exitCode !== null) throw new Error(`server exited before readiness (${process.exitCode})`);
		try {
			const response = await fetch(`${url}/api/healthz`);
			if (response.ok) return;
		} catch {
			// The listener may still be starting.
		}
		await new Promise((resolve) => setTimeout(resolve, 100));
	}
	throw new Error('server did not become ready within 30 seconds');
}

async function startServer(): Promise<ServerFixture> {
	const root = await mkdtemp(join(tmpdir(), 'learnvault-e2e-'));
	const library = join(root, 'library');
	await mkdir(join(library, 'Categoria', 'Curso'), { recursive: true });
	await writeFile(join(library, 'Categoria', 'Curso', 'Aula.mp4'), Buffer.from('video-fixture'));
	await writeFile(join(library, 'Categoria', 'Curso', 'Aula.vtt'), 'WEBVTT\n');
	await writeFile(join(library, 'Categoria', 'Curso', 'Material.pdf'), Buffer.from('material-fixture'));

	const webRoot = resolve(process.cwd(), 'services/go/cmd/learnvault/web/build');
	const defaultBinary = process.platform === 'win32' ? 'target/debug/learnvault-server.exe' : 'target/debug/learnvault-server';
	const binary = process.env.LEARNVAULT_SERVER_BIN ?? resolve(defaultBinary);
	const server = spawn(binary, [
		'--listen-addr', '127.0.0.1:0',
		'--database', join(root, 'learnvault.sqlite'),
		'--library-root', library,
		'--web-root', webRoot,
		'--insecure-http'
	], {
		env: { ...process.env, ADMIN_USERNAME: 'admin', ADMIN_PASSWORD },
		stdio: ['ignore', 'pipe', 'ignore']
	});
	const closed = childClosed(server);
	let baseURL = '';
	try {
		const address = await waitForServerAddress(server);
		baseURL = `http://${address}`;
		await waitForHealth(baseURL, server);
	} catch (error) {
		await Promise.allSettled([
			stopProcess(server, closed),
			rm(root, { recursive: true, force: true })
		]);
		throw error;
	}

	return {
		baseURL,
		stop: async () => {
			try {
				await stopProcess(server, closed);
			} finally {
				await rm(root, { recursive: true, force: true });
			}
		}
	};
}

const test = base.extend<{ server: ServerFixture }>({
	server: [async ({}, use) => {
		const server = await startServer();
		try {
			await use(server);
		} finally {
			await server.stop();
		}
	}, { scope: 'worker' }]
});

test('serves the built SPA through the Rust server for a complete study-library journey', async ({ page, server }) => {
	await page.goto(`${server.baseURL}/#/login`);
	await expect(page.getByRole('heading', { name: /Conhecimento/ })).toBeVisible();

	await page.getByLabel('Usuário').fill('admin');
	await page.getByLabel('Senha').fill(ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Entrar' }).click();
	await expect(page.getByRole('heading', { name: 'Olá, admin.' })).toBeVisible();

	await page.getByRole('link', { name: 'Catálogo', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Seus cursos' })).toBeVisible();
	await page.getByRole('button', { name: 'Indexar biblioteca' }).click();
	await expect(page.getByText('Catálogo atualizado.')).toBeVisible();
	await expect(page.getByRole('button', { name: /Aula/ })).toBeVisible();

	await page.getByRole('button', { name: /Aula/ }).click();
	await expect(page.getByRole('heading', { name: 'Aula' })).toBeVisible();
	const videoSource = await page.locator('video').getAttribute('src');
	expect(videoSource).toMatch(/\/api\/media\?/);
	const videoResponse = await page.request.get(new URL(videoSource as string, server.baseURL).toString());
	await expect(videoResponse).toBeOK();
	await expect(videoResponse.body()).resolves.toEqual(Buffer.from('video-fixture'));

	const material = page.getByRole('link', { name: 'Material.pdf' });
	await expect(material).toHaveAttribute('href', /\/api\/media\?path=/);
	const materialResponse = await page.request.get(new URL(await material.getAttribute('href') as string, server.baseURL).toString());
	await expect(materialResponse).toBeOK();
	await expect(materialResponse.body()).resolves.toEqual(Buffer.from('material-fixture'));

	const traversal = await page.request.get(`${server.baseURL}/api/media?path=${encodeURIComponent('../outside.mp4')}`);
	await expect(traversal).toHaveStatus(404);

	await page.getByRole('link', { name: 'Voltar ao catálogo' }).click();
	await page.getByRole('button', { name: 'Sair' }).click();
	await expect(page.getByRole('button', { name: 'Entrar' })).toBeVisible();
	const afterLogoutMedia = await page.request.get(new URL(videoSource as string, server.baseURL).toString());
	await expect(afterLogoutMedia).toHaveStatus(401);
	const afterLogoutCatalog = await page.request.get(`${server.baseURL}/api/catalog`);
	await expect(afterLogoutCatalog).toHaveStatus(401);
});
