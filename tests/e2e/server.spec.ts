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

function mediaIds(value: unknown, found: string[] = []): string[] {
	if (Array.isArray(value)) {
		for (const item of value) mediaIds(item, found);
		return found;
	}
	if (!value || typeof value !== 'object') return found;
	const record = value as Record<string, unknown>;
	if (Array.isArray(record.files)) {
		for (const file of record.files) {
			if (file && typeof file === 'object' && typeof (file as Record<string, unknown>).media_id === 'string') {
				const id = (file as Record<string, string>).media_id;
				if (!found.includes(id)) found.push(id);
			}
		}
	}
	mediaIds(record.children, found);
	mediaIds(record.nodes, found);
	mediaIds(record.categories, found);
	return found;
}

function parseRgb(value: string): [number, number, number] {
	const rgb = value.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
	if (rgb) return [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])];
	const lab = value.match(/oklab\(\s*([+-]?(?:\d+\.?\d*|\.\d+))\s+([+-]?(?:\d+\.?\d*|\.\d+))\s+([+-]?(?:\d+\.?\d*|\.\d+))/);
	if (!lab) throw new Error(`cor inválida: ${value}`);
	const L = Number(lab[1]);
	const a = Number(lab[2]);
	const b = Number(lab[3]);
	const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
	const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
	const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
	const encode = (linear: number) => {
		const srgb = linear <= 0.0031308 ? 12.92 * linear : 1.055 * linear ** (1 / 2.4) - 0.055;
		return Math.max(0, Math.min(1, srgb)) * 255;
	};
	return [
		encode(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
		encode(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
		encode(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s)
	];
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

async function startServer(libraryName = 'library', extraLessons: string[] = []): Promise<ServerFixture> {
	const root = await mkdtemp(join(tmpdir(), 'learnvault-e2e-'));
	const library = join(root, libraryName);
	const outsidePath = join(root, 'outside.mp4');
	await writeFile(outsidePath, Buffer.from('outside-sentinel'));
	await mkdir(join(library, 'Categoria', 'Curso'), { recursive: true });
	await writeFile(join(library, 'Categoria', 'Curso', 'Aula.mp4'), Buffer.from('video-fixture'));
	await writeFile(join(library, 'Categoria', 'Curso', 'Aula.vtt'), 'WEBVTT\n');
	await writeFile(join(library, 'Categoria', 'Curso', 'Material.pdf'), Buffer.from('material-fixture'));
	for (const lesson of extraLessons) {
		await mkdir(join(library, 'Categoria', lesson), { recursive: true });
		await writeFile(join(library, 'Categoria', lesson, 'Aula.mp4'), Buffer.from('video-fixture'));
		await writeFile(join(library, 'Categoria', lesson, 'Aula.vtt'), 'WEBVTT\n');
	}

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
	const completed = await page.request.post(`${server.baseURL}/api/study/completed`, {
		headers: { Origin: server.baseURL },
		data: { media_id: mediaId, completed: true }
	});
	await expect(completed).toBeOK();
	await page.reload();
	await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeVisible();
	await expect(page.locator('.lv-course-grid .lv-course-card__state')).toHaveCount(0);
	const reopened = await page.request.post(`${server.baseURL}/api/study/completed`, {
		headers: { Origin: server.baseURL },
		data: { media_id: mediaId, completed: false }
	});
	await expect(reopened).toBeOK();
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
	const desktopResumeActionAlignment = await page.evaluate(() => {
		const card = document.querySelector('.lv-resume-card')?.getBoundingClientRect();
		const action = document.querySelector('.lv-resume-card__action')?.getBoundingClientRect();
		if (!card || !action) return null;
		return Math.abs(card.top + card.height / 2 - action.top - action.height / 2);
	});
	expect(desktopResumeActionAlignment).not.toBeNull();
	expect(desktopResumeActionAlignment!).toBeLessThanOrEqual(4);
	const desktopControlSizes = await page.evaluate(() => {
		const size = (selector: string) => {
			const bounds = document.querySelector(selector)?.getBoundingClientRect();
			return bounds ? { width: bounds.width, height: bounds.height } : null;
		};
		return {
			viewHeights: Array.from(document.querySelectorAll('.lv-view-switch button')).map((element) => element.getBoundingClientRect().height),
			ctaHeights: Array.from(document.querySelectorAll('.lv-library .lv-page__buttons .lvui-btn')).map((element) => element.getBoundingClientRect().height),
			theme: size('.lv-topbar__icon'),
			logout: size('.lv-topbar__logout'),
			resumeCue: size('.lv-resume-card__action')
		};
	});
	expect(desktopControlSizes.viewHeights.every((height) => height >= 44)).toBe(true);
	expect(desktopControlSizes.ctaHeights.every((height) => height >= 44)).toBe(true);
	expect(desktopControlSizes.theme?.width).toBeGreaterThanOrEqual(44);
	expect(desktopControlSizes.theme?.height).toBeGreaterThanOrEqual(44);
	expect(desktopControlSizes.logout?.width).toBeGreaterThanOrEqual(44);
	expect(desktopControlSizes.logout?.height).toBeGreaterThanOrEqual(44);
	expect(desktopControlSizes.resumeCue?.width).toBeGreaterThanOrEqual(44);
	expect(desktopControlSizes.resumeCue?.height).toBeGreaterThanOrEqual(44);
	await captureVisual(page, 'redesign-biblioteca-compacta-desktop.png');
	await expect(page.getByRole('button', { name: 'Importar curso' })).not.toHaveClass(/lvui-btn-primary/);
	const search = page.getByRole('searchbox', { name: 'Buscar nos cursos' });
	await search.fill('nenhum curso deve corresponder');
	await expect(page.locator('.lv-resume')).toHaveCount(0);
	await expect(page.locator('.lv-course-grid .lv-course-card')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Importar curso' })).toHaveClass(/lvui-btn-primary/);
	await search.fill('');
	await expect(page.getByRole('heading', { name: 'Continue estudando' })).toBeVisible();


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
	await page.setViewportSize({ width: 390, height: 844 });
	if (process.env.LEARNVAULT_VISUAL_CAPTURE_DIR) {
		const activeRecent = await page.request.post(`${server.baseURL}/api/study/completed`, {
			headers: { Origin: server.baseURL },
			data: { media_id: mediaId, completed: false }
		});
		await expect(activeRecent).toBeOK();
		await page.reload();
		await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeVisible();
		await expect(page.locator('.lv-course-grid .lv-course-card__state')).toHaveText('Em andamento');
		await expect(page.locator('.lv-resume-card__meta')).toHaveText(/Parou em/);
	}
	const resumeGeometry = await page.evaluate(() => {
		const resume = document.querySelector('.lv-resume');
		const card = resume?.querySelector('.lv-resume-card');
		const search = document.querySelector('.lv-library-search input');
		const glyph = document.querySelector('.lv-library-search .search-glyph');
		const glyphRect = glyph?.getBoundingClientRect();
		const glyphStyle = glyph ? getComputedStyle(glyph) : null;
		return {
			container: resume?.getBoundingClientRect().width ?? null,
			card: card?.getBoundingClientRect().width ?? null,
			cardLeft: card?.getBoundingClientRect().left ?? null,
			searchLeft: search?.getBoundingClientRect().left ?? null,
			glyphWidth: glyphRect?.width ?? 0,
			glyphHeight: glyphRect?.height ?? 0,
			glyphVisibility: glyphStyle?.visibility ?? 'missing',
			glyphOpacity: glyphStyle?.opacity ?? '0',
			glyphColor: glyphStyle?.color ?? 'transparent',
			glyphZIndex: glyphStyle?.zIndex ?? 'auto'
		};
	});
	expect(resumeGeometry.container).not.toBeNull();
	expect(resumeGeometry.card).not.toBeNull();
	expect(resumeGeometry.card!).toBeGreaterThanOrEqual(resumeGeometry.container! - 8);
	expect(resumeGeometry.cardLeft).not.toBeNull();
	expect(resumeGeometry.searchLeft).not.toBeNull();
	expect(Math.abs(resumeGeometry.cardLeft! - resumeGeometry.searchLeft!)).toBeLessThanOrEqual(1);
	expect(resumeGeometry.glyphWidth).toBeGreaterThanOrEqual(16);
	expect(resumeGeometry.glyphHeight).toBeGreaterThanOrEqual(16);
	expect(resumeGeometry.glyphVisibility).toBe('visible');
	expect(resumeGeometry.glyphOpacity).toBe('1');
	expect(resumeGeometry.glyphColor).not.toBe('rgba(0, 0, 0, 0)');
	expect(resumeGeometry.glyphZIndex).toBe('1');
	const resumeAffordance = page.locator('.lv-resume-card__action');
	await expect(resumeAffordance).toHaveCount(1);
	const resumeAffordanceBox = await resumeAffordance.boundingBox();
	expect(resumeAffordanceBox).not.toBeNull();
	expect(resumeAffordanceBox!.width).toBeGreaterThanOrEqual(44);
	expect(resumeAffordanceBox!.height).toBeGreaterThanOrEqual(44);
	const mobileTargetSizes = await page.evaluate(() => {
		const focusTrigger = document.querySelector('.lv-focus-menu__trigger');
		const logout = document.querySelector('.lv-topbar__logout')?.getBoundingClientRect();
		const theme = document.querySelector('.lv-topbar__icon')?.getBoundingClientRect();
		const utils = document.querySelector('.lv-topbar__utils');
		const secondaryCta = document.querySelector('.lv-library .lv-page__buttons .lvui-btn:not(.lvui-btn-primary)');
		const secondaryCtaStyle = secondaryCta ? getComputedStyle(secondaryCta) : null;
		return {
			focusHeight: focusTrigger?.getBoundingClientRect().height ?? 0,
			logoutWidth: logout?.width ?? 0,
			logoutHeight: logout?.height ?? 0,
			themeWidth: theme?.width ?? 0,
			themeHeight: theme?.height ?? 0,
			utilsGap: Number.parseFloat(utils ? getComputedStyle(utils).columnGap : '0'),
			navHeights: Array.from(document.querySelectorAll('.lv-topbar__nav .lv-topbar__link')).map((element) => element.getBoundingClientRect().height),
			viewHeights: Array.from(document.querySelectorAll('.lv-view-switch button')).map((element) => element.getBoundingClientRect().height),
			ctaHeights: Array.from(document.querySelectorAll('.lv-library .lv-page__buttons .lvui-btn')).map((element) => element.getBoundingClientRect().height),
			ctaWidths: Array.from(document.querySelectorAll('.lv-library .lv-page__buttons .lvui-btn')).map((element) => element.getBoundingClientRect().width),
			ctaTextFits: Array.from(document.querySelectorAll('.lv-library .lv-page__buttons .lvui-btn')).map((element) => element.scrollWidth <= element.clientWidth),
			secondaryCtaBorderWidth: Number.parseFloat(secondaryCtaStyle?.borderTopWidth ?? '0')
		};
	});
	expect(mobileTargetSizes.focusHeight).toBeGreaterThanOrEqual(44);
	expect(mobileTargetSizes.logoutWidth).toBeGreaterThanOrEqual(44);
	expect(mobileTargetSizes.logoutHeight).toBeGreaterThanOrEqual(44);
	expect(mobileTargetSizes.navHeights.every((height) => height >= 44)).toBe(true);
	expect(mobileTargetSizes.viewHeights.every((height) => height >= 44)).toBe(true);
	expect(mobileTargetSizes.ctaHeights.length).toBe(2);
	expect(mobileTargetSizes.ctaHeights.every((height) => height >= 44)).toBe(true);
	expect(Math.abs(mobileTargetSizes.ctaWidths[0] - mobileTargetSizes.ctaWidths[1])).toBeLessThanOrEqual(1);
	expect(mobileTargetSizes.ctaTextFits.every(Boolean)).toBe(true);
	expect(mobileTargetSizes.secondaryCtaBorderWidth).toBeGreaterThanOrEqual(1);
	expect(mobileTargetSizes.themeWidth).toBeGreaterThanOrEqual(44);
	expect(mobileTargetSizes.themeHeight).toBeGreaterThanOrEqual(44);
	expect(mobileTargetSizes.utilsGap).toBeGreaterThanOrEqual(8);
	await expect(page.getByRole('button', { name: /Mudar para tema/ })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Importar curso' })).not.toHaveClass(/lvui-btn-primary/);
	await page.setViewportSize({ width: 320, height: 844 });
	const narrowCtaTextFits = await page.locator('.lv-library .lv-page__buttons .lvui-btn').evaluateAll((buttons) => buttons.map((button) => button.scrollWidth <= button.clientWidth));
	expect(narrowCtaTextFits).toHaveLength(2);
	expect(narrowCtaTextFits.every(Boolean)).toBe(true);
	await page.setViewportSize({ width: 390, height: 844 });
	const coverColumnCount = () => page.locator('.lv-course-grid').evaluate((grid) => getComputedStyle(grid).gridTemplateColumns.trim().split(/\s+/).length);
	expect(await coverColumnCount()).toBe(2);
	const cardAlignment = await page.evaluate(() => {
		const switcher = document.querySelector('.lv-view-switch');
		const grid = document.querySelector('.lv-course-grid');
		const body = grid?.querySelector('.lv-course-card__body');
		const title = body?.querySelector('.lv-course-card__title');
		if (!switcher || !grid || !body || !title) return null;
		const range = document.createRange();
		range.selectNodeContents(title);
		return {
			titleInset: range.getBoundingClientRect().left - body.getBoundingClientRect().left,
			viewToGridGap: grid.getBoundingClientRect().top - switcher.getBoundingClientRect().bottom
		};
	});
	expect(cardAlignment).not.toBeNull();
	expect(cardAlignment!.titleInset).toBeLessThanOrEqual(16);
	expect(cardAlignment!.viewToGridGap).toBeGreaterThanOrEqual(12);
	await page.getByRole('button', { name: 'Lista' }).click();
	expect(await coverColumnCount()).toBe(1);
	await page.getByRole('button', { name: 'Capas' }).click();
	expect(await coverColumnCount()).toBe(2);
	const readLibraryStates = () => page.evaluate(() => {
		const active = document.querySelector('.lv-view-switch button[aria-pressed="true"]');
		const inactive = document.querySelector('.lv-view-switch button[aria-pressed="false"]');
		const action = document.querySelector('.lv-resume-card__action');
		if (!active || !inactive || !action) return null;
		const surfaceFor = (element: Element) => {
			for (let current: Element | null = element; current; current = current.parentElement) {
				const color = getComputedStyle(current).backgroundColor;
				if (color !== 'rgba(0, 0, 0, 0)' && color !== 'transparent') return color;
			}
			return getComputedStyle(document.body).backgroundColor;
		};
		const activeStyle = getComputedStyle(active);
		const inactiveStyle = getComputedStyle(inactive);
		const actionStyle = getComputedStyle(action);
		return {
			activeBorder: activeStyle.borderTopColor,
			activeColor: activeStyle.color,
			activeBackground: activeStyle.backgroundColor,
			inactiveColor: inactiveStyle.color,
			inactiveSurface: surfaceFor(inactive),
			actionColor: actionStyle.color,
			actionBackground: actionStyle.backgroundColor
		};
	});
	await page.locator('.lv-view-switch button').evaluateAll(async (buttons) => {
		await Promise.all(buttons.flatMap((button) => button.getAnimations().map((animation) => animation.finished.catch(() => undefined))));
	});
	const lightLibraryStates = await readLibraryStates();
	expect(lightLibraryStates).not.toBeNull();
	expect(contrastRatio(lightLibraryStates!.activeBorder, lightLibraryStates!.inactiveSurface)).toBeGreaterThanOrEqual(3);
	expect(contrastRatio(lightLibraryStates!.activeColor, lightLibraryStates!.activeBackground)).toBeGreaterThanOrEqual(4.5);
	expect(contrastRatio(lightLibraryStates!.inactiveColor, lightLibraryStates!.inactiveSurface)).toBeGreaterThanOrEqual(4.5);
	expect(contrastRatio(lightLibraryStates!.activeColor, lightLibraryStates!.activeBackground)).toBeGreaterThan(contrastRatio(lightLibraryStates!.inactiveColor, lightLibraryStates!.inactiveSurface));
	expect(contrastRatio(lightLibraryStates!.actionColor, lightLibraryStates!.actionBackground)).toBeGreaterThanOrEqual(3);
	const recentCountGap = await page.evaluate(() => {
		const heading = document.querySelector('#recent-heading');
		const count = heading?.parentElement?.querySelector('.lv-count');
		if (!heading || !count) return null;
		return count.getBoundingClientRect().left - heading.getBoundingClientRect().right;
	});
	expect(recentCountGap).not.toBeNull();
	expect(recentCountGap!).toBeLessThanOrEqual(12);
	const titleCountRows = await page.evaluate(() => {
		const recentHeading = document.querySelector('#recent-heading');
		const catalogHeading = document.querySelector('#courses-heading');
		return {
			recentHeader: recentHeading?.parentElement?.getBoundingClientRect().height ?? null,
			recentTitle: recentHeading?.getBoundingClientRect().height ?? null,
			catalogHeader: catalogHeading?.parentElement?.getBoundingClientRect().height ?? null,
			catalogTitle: catalogHeading?.getBoundingClientRect().height ?? null
		};
	});
	expect(titleCountRows.recentHeader).not.toBeNull();
	expect(titleCountRows.recentTitle).not.toBeNull();
	expect(titleCountRows.catalogHeader).not.toBeNull();
	expect(titleCountRows.catalogTitle).not.toBeNull();
	expect(Math.abs(titleCountRows.recentHeader! - titleCountRows.recentTitle!)).toBeLessThan(3);
	expect(Math.abs(titleCountRows.catalogHeader! - titleCountRows.catalogTitle!)).toBeLessThan(3);
	await captureVisual(page, 'mobile-library-refinement-390-light.png');
	await page.emulateMedia({ colorScheme: 'dark' });
	await page.reload();
	await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeVisible();
	const darkLibraryStates = await readLibraryStates();
	expect(darkLibraryStates).not.toBeNull();
	expect(contrastRatio(darkLibraryStates!.activeBorder, darkLibraryStates!.inactiveSurface)).toBeGreaterThanOrEqual(3);
	expect(contrastRatio(darkLibraryStates!.activeColor, darkLibraryStates!.activeBackground)).toBeGreaterThanOrEqual(4.5);
	expect(contrastRatio(darkLibraryStates!.inactiveColor, darkLibraryStates!.inactiveSurface)).toBeGreaterThanOrEqual(4.5);
	expect(contrastRatio(darkLibraryStates!.activeColor, darkLibraryStates!.activeBackground)).toBeGreaterThan(contrastRatio(darkLibraryStates!.inactiveColor, darkLibraryStates!.inactiveSurface));
	expect(contrastRatio(darkLibraryStates!.actionColor, darkLibraryStates!.actionBackground)).toBeGreaterThanOrEqual(3);
	await captureVisual(page, 'mobile-library-refinement-390-dark.png');

	const mobileSearch = page.getByRole('searchbox', { name: 'Buscar nos cursos' });
	await mobileSearch.fill('não existe curso com esse termo');
	await expect(page.getByText('Nenhum resultado encontrado', { exact: true })).toBeVisible();
	await expect(page.locator('.lv-resume')).toHaveCount(0);
	await page.getByRole('button', { name: 'Limpar busca' }).click();
	await expect(page.getByRole('heading', { name: 'Continue estudando' })).toBeVisible();
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

test.describe('touch course cover actions', () => {
	test.use({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });

	test('keeps cover controls visible and usable without hover', async ({ page, server }) => {
		await page.goto(`${server.baseURL}/#/login`);
		await page.getByLabel('Usuário').fill('admin');
		await page.getByLabel('Senha', { exact: true }).fill(ADMIN_PASSWORD);
		await page.getByRole('button', { name: 'Entrar' }).click();
		await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeAttached();

		expect(await page.evaluate(() => matchMedia('(hover: none)').matches)).toBe(true);
		const coverActions = page.getByRole('button', { name: 'Ações de capa de library' });
		await expect(coverActions).toBeVisible();
		await expect(coverActions).toHaveCSS('opacity', '1');
		await expect(coverActions).toHaveCSS('width', '44px');
		await captureVisual(page, 'mobile-library-cover-actions-390-light.png');
		await coverActions.tap();
		await expect(page.getByRole('group', { name: 'Capa de library' })).toBeVisible();
		await captureVisual(page, 'mobile-library-cover-actions-open-390-light.png');
	});
});

test('keeps long course titles readable in the mobile covers grid', async ({ page }) => {
	const courseTitle = 'Aprendizagem Digital e Design Instrucional para Cursos Profissionais Avançados';
	const server = await startServer(courseTitle);
	try {

		await page.goto(`${server.baseURL}/#/login`);
		await page.getByLabel('Usuário').fill('admin');
		await page.getByLabel('Senha', { exact: true }).fill(ADMIN_PASSWORD);
		await page.getByRole('button', { name: 'Entrar' }).click();
		await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeAttached();
		await page.getByRole('button', { name: 'Atualizar cursos' }).click();
		await expect(page.getByText('Catálogo atualizado.')).toBeVisible();
		await page.setViewportSize({ width: 390, height: 844 });
		await page.getByRole('button', { name: 'Capas' }).click();

		const longTitle = page.locator('.lv-course-grid .lv-course-card__title').filter({ hasText: courseTitle });
		await expect(longTitle).toBeVisible();
		const titleLayout = await longTitle.evaluate((element) => {
			const rect = element.getBoundingClientRect();
			const style = getComputedStyle(element);
			return {
				fontSize: style.fontSize,
				lineHeight: Number.parseFloat(style.lineHeight),
				height: rect.height,
				cardScrollWidth: element.closest('.lv-course-card')?.scrollWidth ?? 0,
				cardClientWidth: element.closest('.lv-course-card')?.clientWidth ?? 0
			};
		});
		expect(titleLayout.fontSize).toBe('15px');
		expect(titleLayout.height / titleLayout.lineHeight).toBeGreaterThan(1);
		expect(titleLayout.height / titleLayout.lineHeight).toBeLessThanOrEqual(2);
		expect(titleLayout.cardClientWidth).toBeGreaterThan(0);
		expect(titleLayout.cardScrollWidth).toBeLessThanOrEqual(titleLayout.cardClientWidth);
		if (process.env.LEARNVAULT_VISUAL_CAPTURE_DIR) await captureVisual(page, 'mobile-library-long-title-390-light.png');
		await longTitle.click();
		await expect(page.getByRole('heading', { name: courseTitle, exact: true })).toBeVisible();
	} finally {
		await page.context().close();
		await server.stop();
	}
});

test('keeps every recent item reachable and aligned in the mobile carousel', async ({ page }) => {
	const server = await startServer('library', ['Segundo curso']);
	try {
		await page.goto(`${server.baseURL}/#/login`);
		await page.getByLabel('Usuário').fill('admin');
		await page.getByLabel('Senha', { exact: true }).fill(ADMIN_PASSWORD);
		await page.getByRole('button', { name: 'Entrar' }).click();
		await expect(page.getByRole('heading', { name: 'Minha biblioteca' })).toBeAttached();
		await page.getByRole('button', { name: 'Atualizar cursos' }).click();
		await expect(page.getByText('Catálogo atualizado.')).toBeVisible();

		const catalog = await (await page.request.get(`${server.baseURL}/api/catalog`)).json();
		const ids = mediaIds(catalog);
		expect(ids.length).toBeGreaterThanOrEqual(2);
		for (const [index, mediaId] of ids.entries()) {
			const response = await page.request.post(`${server.baseURL}/api/study/position`, {
				headers: { Origin: server.baseURL },
				data: { media_id: mediaId, position_seconds: 5 + index }
			});
			await expect(response).toBeOK();
		}
		const recent = await (await page.request.get(`${server.baseURL}/api/study/recent?limit=6`)).json();
		// A Biblioteca mostra no máximo quatro retomadas.
		const expectedCards = Math.min(recent.items.length, 4);
		expect(expectedCards).toBeGreaterThanOrEqual(2);

		await page.setViewportSize({ width: 390, height: 844 });
		await page.reload();
		const strip = page.locator('.lv-library .lv-resume');
		const items = page.locator('.lv-library .lv-resume > li');
		await expect(items).toHaveCount(expectedCards);

		// A faixa rola horizontalmente: nenhum cartão fica inalcançável.
		const stripState = await strip.evaluate((element) => ({
			scrollable: element.scrollWidth > element.clientWidth,
			snapType: getComputedStyle(element).scrollSnapType
		}));
		expect(stripState.scrollable).toBe(true);

		// Gesto real de rolagem horizontal: a faixa precisa responder ao input,
		// não apenas à rolagem programática, e repousar alinhada a um cartão.
		const startOffsets = () => strip.evaluate((element) => {
			const container = element.getBoundingClientRect();
			return Array.from(element.children).map((child) =>
				child.getBoundingClientRect().left - container.left
			);
		});
		const referenceOffset = (await startOffsets())[0];
		const stripBox = await strip.boundingBox();
		expect(stripBox).not.toBeNull();
		await page.mouse.move(stripBox!.x + stripBox!.width / 2, stripBox!.y + stripBox!.height / 2);
		await page.mouse.wheel(40, 0);
		await expect.poll(async () => strip.evaluate((element) => element.scrollLeft)).toBeGreaterThan(0);
		await expect.poll(async () => {
			const offsets = await startOffsets();
			return Math.min(...offsets.map((offset) => Math.abs(offset - referenceOffset)));
		}).toBeLessThanOrEqual(1);

		// Cada retomada é alcançável e nenhum cartão é cortado pela faixa.
		const reach = await strip.evaluate((element) => {
			const padLeft = Number.parseFloat(getComputedStyle(element).paddingLeft);
			return Array.from(element.children).map((child, index, all) => {
				child.scrollIntoView({ inline: 'start', block: 'nearest' });
				const container = element.getBoundingClientRect();
				const bounds = child.getBoundingClientRect();
				return {
					isLast: index === all.length - 1,
					snapAlign: getComputedStyle(child).scrollSnapAlign,
					outsideLeft: container.left - bounds.left,
					outsideRight: bounds.right - container.right,
					startOffset: Math.abs(bounds.left - (container.left + padLeft))
				};
			});
		});
		expect(reach).toHaveLength(expectedCards);
		for (const card of reach) {
			expect(card.snapAlign).toBe('start');
			// Nenhum cartão é cortado pelas bordas da faixa ao ser alcançado.
			expect(card.outsideLeft).toBeLessThanOrEqual(1);
			expect(card.outsideRight).toBeLessThanOrEqual(1);
		}
		if (process.env.LEARNVAULT_VISUAL_CAPTURE_DIR) await captureVisual(page, 'mobile-library-resume-carousel-390-light.png');
	} finally {
		await page.context().close();
		await server.stop();
	}
});
