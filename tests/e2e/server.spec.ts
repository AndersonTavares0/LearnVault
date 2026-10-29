import { expect, test as base, type Page } from '@playwright/test';
import { once } from 'node:events';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawn, type ChildProcess } from 'node:child_process';

const ADMIN_PASSWORD = 'uma senha local muito segura';

function firstMediaId(value: unknown): string | undefined {
	if (Array.isArray(value)) {
		for (const item of value) {
			const mediaId = firstMediaId(item);
			if (mediaId) return mediaId;
		}
		return undefined;
	}
	if (!value || typeof value !== 'object') return undefined;
	const record = value as Record<string, unknown>;
	if (Array.isArray(record.files)) {
		for (const file of record.files) {
			if (file && typeof file === 'object' && typeof (file as Record<string, unknown>).media_id === 'string') {
				return (file as Record<string, string>).media_id;
			}
		}
	}
	return firstMediaId(record.children) ?? firstMediaId(record.nodes) ?? firstMediaId(record.categories);
}

function parseRgb(value: string): [number, number, number] {
	const match = value.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
	if (!match) throw new Error(`cor inválida: ${value}`);
	return [Number(match[1]), Number(match[2]), Number(match[3])];
}

function contrastRatio(foreground: string, background: string) {
	const luminance = (color: string) => parseRgb(color).reduce((sum, channel, index) => {
		const normalized = channel / 255;
		const linear = normalized <= 0.03928 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
		return sum + linear * [0.2126, 0.7152, 0.0722][index];
	}, 0);
	const foregroundLuminance = luminance(foreground);
	const backgroundLuminance = luminance(background);
	return (Math.max(foregroundLuminance, backgroundLuminance) + 0.05) / (Math.min(foregroundLuminance, backgroundLuminance) + 0.05);
}

async function captureVisual(page: Page, filename: string) {
	const directory = process.env.LEARNVAULT_VISUAL_CAPTURE_DIR;
	if (!directory) return;
	await mkdir(directory, { recursive: true });
	await page.screenshot({ path: join(directory, filename), fullPage: true, animations: 'disabled' });
}

type ServerFixture = {
	baseURL: string;
	libraryPath: string;
	outsidePath: string;
	stop: () => Promise<void>;
};

// A bounded grace period is only the escalation boundary; cleanup otherwise waits on
// the child's close event rather than sleeping or abandoning a live server.
const SHUTDOWN_GRACE_MS = 5_000;
const HEALTH_REQUEST_TIMEOUT_MS = 1_000;

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
			const response = await fetch(`${url}/api/healthz`, {
				signal: AbortSignal.timeout(HEALTH_REQUEST_TIMEOUT_MS)
			});
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
	const outsidePath = join(root, 'outside.mp4');
	await writeFile(outsidePath, Buffer.from('outside-sentinel'));
	await mkdir(join(library, 'Categoria', 'Curso'), { recursive: true });
	await writeFile(join(library, 'Categoria', 'Curso', 'Aula.mp4'), Buffer.from('video-fixture'));
	await writeFile(join(library, 'Categoria', 'Curso', 'Aula.vtt'), 'WEBVTT\n');
	await writeFile(join(library, 'Categoria', 'Curso', 'Material.pdf'), Buffer.from('material-fixture'));

	const webRoot = resolve(process.cwd(), 'crates/server/web/build');
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
		libraryPath: library,
		outsidePath,
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
	if (process.env.LEARNVAULT_VISUAL_CAPTURE_DIR) await page.setViewportSize({ width: 1440, height: 900 });
	await page.goto(`${server.baseURL}/#/login`);
	await expect(page.getByRole('heading', { name: 'Entrar no LearnVault' })).toBeVisible();

	await page.getByLabel('Usuário').fill('admin');
	// O botão "Mostrar Senha" compartilha o rótulo parcial; exigir o campo exato evita
	// que o locator resolva tanto o input quanto o alternador de visibilidade.
	await page.getByLabel('Senha', { exact: true }).fill(ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Entrar' }).click();
	await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeAttached();

	await page.getByRole('button', { name: 'Atualizar cursos' }).click();
	await expect(page.getByText('Catálogo atualizado.')).toBeVisible();
	await expect(page.getByRole('navigation', { name: 'Seções' })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Todos os cursos' })).toBeVisible();
	const initialCatalog = await (await page.request.get(`${server.baseURL}/api/catalog`)).json();
	const mediaId = firstMediaId(initialCatalog);
	expect(mediaId).toBeTruthy();
	const positionResponse = await page.request.post(`${server.baseURL}/api/study/position`, {
		headers: { Origin: server.baseURL },
		data: { media_id: mediaId, position_seconds: 5 }
	});
	await expect(positionResponse).toBeOK();
	const recentResponse = await page.request.get(`${server.baseURL}/api/study/recent?limit=6`);
	await expect(recentResponse).toBeOK();
	expect((await recentResponse.json()).items.length).toBeGreaterThan(0);
	await page.reload();
	await expect(page.getByRole('heading', { name: 'Continue estudando' })).toBeVisible();
	await captureVisual(page, 'redesign-biblioteca-compacta-desktop.png');

	// Cada item identifica a aula pelo título limpo e mostra o contexto (curso · módulo)
	// em uma linha secundária; o nome cru do arquivo não pode vazar para o usuário.
	const firstRecentItem = page.locator('.lv-resume li').first();
	const lessonNode = firstRecentItem.locator('.lv-resume-card__lesson');
	const lessonLabel = (await lessonNode.innerText()).trim();
	expect(lessonLabel).not.toMatch(/\.[a-z0-9]{2,4}$/i);
	expect(lessonLabel).not.toMatch(/^\d+[\s._-]/);
	const contextNode = firstRecentItem.locator('.lv-resume-card__course');
	await expect(contextNode).toHaveText(/\S/);
	expect((await contextNode.innerText()).trim()).toBe('library');

	// Em tela estreita a lista não pode gerar rolagem horizontal.
	const originalViewport = page.viewportSize();
	await page.setViewportSize({ width: 390, height: 844 });
	const narrowWidths = await page.evaluate(() => ({
		scrollWidth: document.documentElement.scrollWidth,
		clientWidth: document.documentElement.clientWidth
	}));
	expect(narrowWidths.scrollWidth).toBeLessThanOrEqual(narrowWidths.clientWidth);
	if (originalViewport) await page.setViewportSize(originalViewport);

	const lightColors = await page.locator('.lv-resume-card__meta').first().evaluate((element) => {
		let node: HTMLElement | null = element as HTMLElement;
		const foreground = getComputedStyle(element).color;
		while (node) {
			const background = getComputedStyle(node).backgroundColor;
			if (background !== 'rgba(0, 0, 0, 0)' && background !== 'transparent') return { foreground, background };
			node = node.parentElement;
		}
		return { foreground, background: getComputedStyle(document.body).backgroundColor };
	});
	expect(contrastRatio(lightColors.foreground, lightColors.background)).toBeGreaterThanOrEqual(4.5);
	await page.emulateMedia({ colorScheme: 'dark' });
	await page.reload();
	const darkColors = await page.locator('.lv-resume-card__meta').first().evaluate((element) => {
		let node: HTMLElement | null = element as HTMLElement;
		const foreground = getComputedStyle(element).color;
		while (node) {
			const background = getComputedStyle(node).backgroundColor;
			if (background !== 'rgba(0, 0, 0, 0)' && background !== 'transparent') return { foreground, background };
			node = node.parentElement;
		}
		return { foreground, background: getComputedStyle(document.body).backgroundColor };
	});
	expect(contrastRatio(darkColors.foreground, darkColors.background)).toBeGreaterThanOrEqual(4.5);
	await page.emulateMedia({ colorScheme: 'light' });
	await page.reload();
	await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeVisible();
	const faviconResponse = await page.request.get(`${server.baseURL}/favicon.svg`);
	expect(faviconResponse.status()).toBe(200);

	// A home lista cada curso registrado como card; o card abre a tela de estudo já com o
	// player na última aula assistida e a lista lateral de aulas.
	const courseCard = page.getByRole('link', { name: 'library', exact: true });
	await expect(courseCard).toBeVisible();
	const videoRequest = page.waitForRequest((request) => {
		const url = new URL(request.url());
		return url.pathname === '/api/media' && url.searchParams.get('path') === 'Categoria/Curso/Aula.mp4';
	});
	await courseCard.click();
	await expect(page.locator('.lv-breadcrumb')).toContainText('library');
	await expect(page.getByRole('heading', { name: 'library', exact: true })).toBeVisible();
	await captureVisual(page, 'redesign-curso-desktop.png');
	await page.getByRole('link', { name: 'Continuar estudando' }).click();
	await expect(page.getByRole('heading', { name: 'Curso', exact: true })).toBeVisible();
	await captureVisual(page, 'redesign-player-desktop.png');
	await page.getByRole('button', { name: /^Pomodoro:/ }).click();
	await expect(page.getByRole('dialog', { name: 'Pomodoro' })).toBeVisible();
	await expect(page.getByRole('group', { name: 'Modo do timer' })).toBeVisible();
	await captureVisual(page, 'redesign-pomodoro-integrado-desktop.png');
	await page.getByRole('button', { name: 'Fechar Pomodoro' }).click();
	// A aula inicial vem do histórico recente gravado acima.
	await expect(page.locator('.lv-curriculum__body .lesson-row.active .lesson-title')).toHaveText('Curso');
	const videoSource = (await videoRequest).url();
	const videoURL = new URL(videoSource, server.baseURL);
	expect(videoURL.pathname).toBe('/api/media');
	expect(videoURL.searchParams.get('path')).toBe('Categoria/Curso/Aula.mp4');
	// Toda mídia é resolvida dentro de um curso registrado, nunca por caminho global.
	expect(videoURL.searchParams.get('course')).toBeTruthy();
	const videoResponse = await page.request.get(videoURL.toString());
	await expect(videoResponse).toBeOK();
	await expect(videoResponse.body()).resolves.toEqual(Buffer.from('video-fixture'));

	const materialHref = await page.getByRole('link', { name: 'Material.pdf' }).getAttribute('href');
	expect(materialHref).toBeTruthy();
	const materialURL = new URL(materialHref as string, server.baseURL);
	expect(materialURL.pathname).toBe('/api/media');
	expect(materialURL.searchParams.get('path')).toBe('Categoria/Curso/Material.pdf');
	const materialResponse = await page.request.get(materialURL.toString());
	await expect(materialResponse).toBeOK();
	await expect(materialResponse.body()).resolves.toEqual(Buffer.from('material-fixture'));

	const catalog = await (await page.request.get(`${server.baseURL}/api/catalog`)).json();
	const courseId = catalog.categories?.[0]?.course_id ?? catalog.nodes?.[0]?.course_id;
	expect(courseId).toBeTruthy();
	for (const path of ['../outside.mp4', '..\\outside.mp4', server.outsidePath]) {
		const traversal = await page.request.get(
			`${server.baseURL}/api/media?course=${encodeURIComponent(courseId)}&path=${encodeURIComponent(path)}`
		);
		// Caminhos fora do catálogo são ausência de mídia (404), nunca uma leitura fora da raiz.
		expect(traversal.status()).toBe(404);
		expect(await traversal.body()).not.toEqual(Buffer.from('outside-sentinel'));
	}
	// Sem o curso, a rota não entrega mídia alguma, mesmo para um caminho indexado.
	const unscoped = await page.request.get(
		`${server.baseURL}/api/media?path=${encodeURIComponent('Categoria/Curso/Aula.mp4')}`
	);
	expect(unscoped.status()).toBe(400);

	// O caminho de volta para a biblioteca continua no breadcrumb da tela de estudo.
	await page.locator('.lv-breadcrumb').getByRole('link', { name: 'Biblioteca', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeVisible();
	if (process.env.LEARNVAULT_VISUAL_CAPTURE_DIR) {
		const origin = server.baseURL;
		const completed = await page.request.post(`${origin}/api/study/completed`, {
			headers: { Origin: origin },
			data: { media_id: mediaId, completed: true }
		});
		await expect(completed).toBeOK();
		const now = Math.floor(Date.now() / 1000);
		for (const index of [1, 2]) {
			const endedAt = now - (index - 1) * 1500;
			const session = await page.request.post(`${origin}/api/focus/sessions`, {
				headers: { Origin: origin },
				data: {
					id: `visual-session-${index}`, kind: 'focus', planned_seconds: 1500,
					actual_seconds: 1500, completed: true, started_at: endedAt - 1500, ended_at: endedAt
				}
			});
			await expect(session).toBeOK();
		}
		const oneDayGoal = await page.request.put(`${origin}/api/rewards/weekly-goal`, {
			headers: { Origin: origin }, data: { weekly_goal: 1 }
		});
		await expect(oneDayGoal).toBeOK();
		for (const mission of ['lesson', 'focus-two', 'focus-fifty']) {
			const claim = await page.request.post(`${origin}/api/rewards/missions/${mission}/claim`, {
				headers: { Origin: origin }
			});
			await expect(claim).toBeOK();
		}
		const defaultGoal = await page.request.put(`${origin}/api/rewards/weekly-goal`, {
			headers: { Origin: origin }, data: { weekly_goal: 3 }
		});
		await expect(defaultGoal).toBeOK();
		await page.goto(`${origin}/#/statistics`);
		await expect(page.getByRole('heading', { name: 'Estatísticas' })).toBeVisible();
		await page.emulateMedia({ colorScheme: 'light' });
		await page.reload();
		await expect(page.getByRole('heading', { name: 'Estatísticas' })).toBeVisible();
		await expect(page.getByText('1 de 3 dias', { exact: true })).toBeVisible();
		await captureVisual(page, 'redesign-estatisticas-desktop-claro.png');
		await page.emulateMedia({ colorScheme: 'dark' });
		await page.reload();
		await expect(page.getByRole('heading', { name: 'Estatísticas' })).toBeVisible();
		await captureVisual(page, 'redesign-estatisticas-desktop-escuro.png');
		await page.setViewportSize({ width: 390, height: 844 });
		await captureVisual(page, 'redesign-estatisticas-mobile-escuro.png');
		await page.emulateMedia({ colorScheme: 'light' });
		await page.reload();
		await expect(page.getByRole('heading', { name: 'Estatísticas' })).toBeVisible();
		await captureVisual(page, 'redesign-estatisticas-mobile-claro.png');
	}
	await page.getByRole('link', { name: 'Biblioteca', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeVisible();
	await page.getByRole('button', { name: 'Sair' }).click();
	await expect(page.getByRole('button', { name: 'Entrar' })).toBeVisible();
	const afterLogoutMedia = await page.request.get(new URL(videoSource as string, server.baseURL).toString());
	expect(afterLogoutMedia.status()).toBe(401);
	const afterLogoutCatalog = await page.request.get(`${server.baseURL}/api/catalog`);
	expect(afterLogoutCatalog.status()).toBe(401);
});

test('reports a lesson whose media disappears from disk and restores it when the file returns', async ({ page, server }) => {
	// A fixture já tem Categoria/Curso/Aula.mp4, usado pelo outro cenário. Em vez de apagar
	// esse arquivo, o cenário cria um módulo próprio com duas aulas: a mídia de uma delas
	// sai do disco e volta, para exercitar o ciclo inteiro de disponibilidade.
	const moduleRoot = join(server.libraryPath, 'Categoria', 'Modulo Extra');
	const lessonAPath = join(moduleRoot, 'Aula A');
	const lessonBPath = join(moduleRoot, 'Aula B');
	await mkdir(lessonAPath, { recursive: true });
	await mkdir(lessonBPath, { recursive: true });
	await writeFile(join(lessonAPath, 'A.mp4'), Buffer.from('extra-a-fixture'));
	const lessonBMedia = join(lessonBPath, 'B.mp4');
	const lessonBContent = Buffer.from('extra-b-fixture');
	await writeFile(lessonBMedia, lessonBContent);

	const courseModule = page.locator('details.course-section').filter({ has: page.getByText('Modulo Extra', { exact: true }) });
	const openCourse = async () => {
		await page.getByRole('link', { name: 'library', exact: true }).click();
		await expect(page.locator('.lv-breadcrumb')).toContainText('library');
		// O módulo da aula inicial pode já estar aberto; só clica quando precisa.
		if (!(await courseModule.evaluate((element) => element.hasAttribute('open')))) {
			await courseModule.locator('summary').click();
		}
	};
	const indexFromHome = async () => {
		await page.goto(`${server.baseURL}/#/home`);
		await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeAttached();
		await page.getByRole('button', { name: 'Atualizar cursos' }).click();
		await expect(page.getByText('Catálogo atualizado.')).toBeVisible();
	};

	await page.goto(`${server.baseURL}/#/login`);
	await page.getByLabel('Usuário').fill('admin');
	await page.getByLabel('Senha', { exact: true }).fill(ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Entrar' }).click();
	await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeAttached();
	await page.getByRole('button', { name: 'Atualizar cursos' }).click();
	await expect(page.getByText('Catálogo atualizado.')).toBeVisible();

	// Fase 1: o arquivo está no disco — duas aulas disponíveis, sem selo, linha navegável.
	await openCourse();
	await expect(courseModule.locator('.section-meta')).toHaveText('2 aulas');
	const lessonA = courseModule.getByRole('link', { name: /Aula A/ });
	const lessonB = courseModule.getByRole('link', { name: /Aula B/ });
	await expect(lessonA).toHaveAttribute('href', /#\/course\//);
	await expect(lessonB).toHaveAttribute('href', /#\/course\//);
	await expect(lessonB).not.toContainText('ausente');

	// Fase 2: o arquivo desaparece do disco (disco externo desconectado ou arquivo renomeado).
	await rm(lessonBMedia, { force: true });
	await indexFromHome();
	await openCourse();
	await expect(courseModule.locator('.section-meta')).toHaveText('1 aula · 1 ausente');
	const missingRow = courseModule.locator('.lesson-row', { hasText: 'Aula B' });
	await expect(missingRow).toBeVisible();
	await expect(missingRow).toContainText('(ausente)');
	// Continua listada, mas não navega: sem link, sem foco de teclado.
	await expect(courseModule.getByRole('link', { name: /Aula B/ })).toHaveCount(0);
	expect(await missingRow.evaluate((element) => element.tagName)).toBe('DIV');
	expect(await missingRow.evaluate((element) => { (element as HTMLElement).focus(); return document.activeElement === element; })).toBe(false);
	// A aula disponível ao lado mantém link e foco — a checagem anterior discrimina.
	expect(await lessonA.evaluate((element) => { (element as HTMLElement).focus(); return document.activeElement === element; })).toBe(true);

	// Acesso direto pela URL da aula ausente: o player não pede o arquivo nem renderiza mídia.
	const catalog = await (await page.request.get(`${server.baseURL}/api/catalog`)).json();
	const courseId = catalog.categories?.[0]?.course_id ?? catalog.nodes?.[0]?.course_id;
	expect(courseId).toBeTruthy();
	const mediaRequests: string[] = [];
	page.on('request', (request) => {
		const url = new URL(request.url());
		if (url.pathname === '/api/media') mediaRequests.push(url.search);
	});
	await page.goto(`${server.baseURL}/#/player/${encodeURIComponent(courseId)}/${encodeURIComponent('Categoria/Modulo Extra/Aula B')}`);
	await expect(page.getByText('A mídia desta aula não está no disco.')).toBeVisible();
	expect(await page.locator('video, audio').count()).toBe(0);
	expect(mediaRequests).toEqual([]);

	// Fase 3: o mesmo arquivo volta ao mesmo caminho e a reindexação restaura a aula.
	await writeFile(lessonBMedia, lessonBContent);
	await indexFromHome();
	await openCourse();
	await expect(courseModule.locator('.section-meta')).toHaveText('2 aulas');
	await expect(lessonB).toHaveAttribute('href', /#\/course\//);
	await expect(lessonB).not.toContainText('ausente');
});

test('renders an automatic video thumbnail in the built SPA without leaking library paths', async ({ page, server }) => {
	await page.goto(`${server.baseURL}/#/login`);
	await page.getByLabel('Usuário').fill('admin');
	await page.getByLabel('Senha', { exact: true }).fill(ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Entrar' }).click();
	await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeAttached();

	// Grava um WebM sintético inteiramente no navegador e o entrega ao servidor como arquivo
	// do curso. Nenhum vídeo real, asset externo ou dependência de codec entra no repositório:
	// a mídia é gerada pelo próprio decoder do WebView sob teste.
	const recorded = await page.evaluate(async () => {
		const canvas = document.createElement('canvas');
		canvas.width = 320;
		canvas.height = 180;
		const context = canvas.getContext('2d');
		if (!context) throw new Error('sem contexto 2D');
		if (!window.MediaRecorder || !MediaRecorder.isTypeSupported('video/webm')) throw new Error('MediaRecorder WebM indisponível');
		const stream = canvas.captureStream(12);
		const recorder = new MediaRecorder(stream, { mimeType: 'video/webm' });
		const chunks: Blob[] = [];
		recorder.ondataavailable = (event) => { if (event.data.size) chunks.push(event.data); };
		const stopped = new Promise<void>((resolve) => { recorder.onstop = () => resolve(); });
		recorder.start();
		const start = performance.now();
		const draw = () => {
			const t = (performance.now() - start) / 1000;
			context.fillStyle = '#17324d';
			context.fillRect(0, 0, 320, 180);
			context.fillStyle = '#f4c95d';
			context.fillRect(Math.round((t * 160) % 240), 40, 60, 60);
			if (recorder.state === 'recording') requestAnimationFrame(draw);
		};
		requestAnimationFrame(draw);
		await new Promise((resolve) => setTimeout(resolve, 900));
		recorder.stop();
		await stopped;
		stream.getTracks().forEach((track) => track.stop());
		const buffer = new Uint8Array(await new Blob(chunks, { type: 'video/webm' }).arrayBuffer());
		let binary = '';
		for (const byte of buffer) binary += String.fromCharCode(byte);
		return btoa(binary);
	});
	expect(recorded.length).toBeGreaterThan(0);

	// A lição "Aula zero" ordena antes de "Curso", então o vídeo decodificável é a primeira
	// mídia da árvore e o card do curso passa a ter miniatura automática.
	const lessonDir = join(server.libraryPath, 'Categoria', 'Aula zero');
	await mkdir(lessonDir, { recursive: true });
	await writeFile(join(lessonDir, 'Video.webm'), Buffer.from(recorded, 'base64'));
	await page.getByRole('button', { name: 'Atualizar cursos' }).click();
	await expect(page.getByText('Catálogo atualizado.')).toBeVisible();
	await page.getByRole('button', { name: 'Capas' }).click();

	const thumbnail = page.locator('.lv-cover__image').first();
	await expect(thumbnail).toBeVisible({ timeout: 30_000 });
	// `naturalWidth` só é positivo quando a CSP do servidor permite carregar o `blob:`; um
	// bloqueio de imagem deixaria a capa gerada no lugar e este valor seria zero.
	await expect
		.poll(() => thumbnail.evaluate((element) => (element as HTMLImageElement).naturalWidth), { timeout: 30_000 })
		.toBeGreaterThan(0);
	const image = await thumbnail.evaluate((element) => {
		const node = element as HTMLImageElement;
		return { width: node.naturalWidth, height: node.naturalHeight, src: node.src };
	});
	expect(image.src.startsWith('blob:')).toBe(true);
	expect(image.width).toBeLessThanOrEqual(320);
	expect(image.height).toBeLessThanOrEqual(180);
	// O card nunca expõe o caminho do arquivo no sistema de arquivos.
	expect(image.src).not.toContain(server.libraryPath);
	expect(image.src).not.toContain('Categoria');
	expect(image.src).not.toContain('Video.webm');
});

test('moves the cover actions to the course card and keeps the course page clean', async ({ page, server }) => {
	await page.goto(`${server.baseURL}/#/login`);
	await page.getByLabel('Usuário').fill('admin');
	await page.getByLabel('Senha', { exact: true }).fill(ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Entrar' }).click();
	await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeAttached();
	await page.getByRole('button', { name: 'Atualizar cursos' }).click();
	await expect(page.getByText('Catálogo atualizado.')).toBeVisible();

	const courseCard = page.getByRole('link', { name: 'library', exact: true });
	await expect(courseCard).toBeVisible();

	// O menu abre pelo clique direito e fecha com Escape.
	await courseCard.click({ button: 'right' });
	await expect(page.getByRole('group', { name: 'Capa de library' })).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(page.getByRole('group', { name: 'Capa de library' })).toHaveCount(0);

	// O botão de ações do card abre o mesmo menu.
	await page.getByRole('button', { name: 'Ações de capa de library' }).click();

	// PNG 1x1: conteúdo válido para a validação por magic bytes e para o `<img>` carregar.
	const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64');
	await page.getByRole('button', { name: 'Adicionar capa' }).dispatchEvent('click');
	await page.locator('.course-cover-input').setInputFiles({ name: 'capa.png', mimeType: 'image/png', buffer: png });
	await expect(page.getByText('Capa atualizada.')).toBeVisible();

	const coverImage = page.locator('.lv-course-grid .lv-cover__image');
	await expect(coverImage).toHaveCount(1);
	const coverSource = await coverImage.getAttribute('src');
	expect(coverSource).toBeTruthy();
	const coverUrl = new URL(coverSource as string, server.baseURL);
	expect(coverUrl.pathname).toMatch(/^\/api\/courses\/[^/]+\/cover$/);
	expect(coverUrl.searchParams.get('v')).toMatch(/^\d+$/);
	const coverResponse = await page.request.get(coverUrl.toString());
	await expect(coverResponse).toBeOK();
	expect(coverResponse.headers()['content-type']).toBe('image/png');
	expect(await coverResponse.body()).toEqual(png);

	const catalog = await (await page.request.get(`${server.baseURL}/api/catalog`)).json();
	expect(catalog.nodes[0].cover_version).toBeGreaterThan(0);

	// Com capa manual, o menu oferece trocar e remover.
	await courseCard.click({ button: 'right' });
	await expect(page.getByRole('button', { name: 'Trocar capa' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Remover capa' })).toBeVisible();
	await page.keyboard.press('Escape');

	// A página do curso volta ao layout anterior, sem controles de capa.
	await courseCard.click();
	await expect(page.locator('.lv-breadcrumb')).toContainText('library');
	await expect(page.getByRole('button', { name: /Ações de capa/ })).toHaveCount(0);
	await expect(page.getByText('Trocar capa')).toHaveCount(0);
	await expect(page.getByText('Remover capa')).toHaveCount(0);

	// A remoção também é feita pelo card.
	await page.locator('.lv-breadcrumb').getByRole('link', { name: 'Biblioteca', exact: true }).click();
	await courseCard.click({ button: 'right' });
	await page.getByRole('button', { name: 'Remover capa' }).click();
	await expect(page.getByRole('alert')).toContainText('Remover a capa de “library”?');
	await page.getByRole('button', { name: 'Remover capa' }).click();
	await expect(page.getByText('Capa removida.')).toBeVisible();
	await expect(page.locator('.lv-course-grid .lv-cover__image[src*="/api/courses/"][src*="/cover"]')).toHaveCount(0);

	const afterRemoval = await page.request.get(coverUrl.toString());
	expect(afterRemoval.status()).toBe(404);
	const catalogAfter = await (await page.request.get(`${server.baseURL}/api/catalog`)).json();
	expect(catalogAfter.nodes[0].cover_version).toBeUndefined();
});
