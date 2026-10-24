<script lang="ts">
	import { displayTitleFor, emptyLibraryPreferences, readLibraryCover, readLibraryPreferences, removeLibraryCover, validateDisplayTitle, writeLibraryCover, writeLibraryPreferences, type LibraryPreferences, type LibraryView } from './library-preferences';
	import { onMount } from 'svelte';
	import LooseFiles from './LooseFiles.svelte';
	import CatalogTree from './CatalogTree.svelte';
	import CourseCard from './CourseCard.svelte';
	import Player from './Player.svelte';
	import NotesIndex from './NotesIndex.svelte';
	import StudyAnalytics from './StudyAnalytics.svelte';
	import Pomodoro from './Pomodoro.svelte';
	import { filterCatalog } from './search';
	import { countCatalogNode, formatCourseMeta, preferredCourseCover } from './catalog';
	import { courseCover } from './cover';
	import PasswordField from './PasswordField.svelte';
	import { lessonSections, neighboringLessons, type LessonSection } from './navigation';
	import { destinationAfterLogin, findCourse, findLesson, findLessonLocation, href, locationFromHash, type Route } from './router';
	import { courseIdOf, transport, transportKind, STUDY_NOT_AVAILABLE, type Catalog, type CatalogNode, type CourseStudySummary, type Lesson, type StudyProgress, type User } from './transport';
	import { defaultRoute, resolveSession } from './session';
	import { formatDuration, studySyncSupported } from './player';
	import Brand from './Brand.svelte';
	import { applyTheme, applyThemeColor, applyThemePalette, persistThemeMode, persistThemePalette, readThemeMode, readThemePalette, resolveTheme, type ThemeMode, type ThemePalette } from './theme';

	let themeMode: ThemeMode = readThemeMode(safeLocalStorage());
	let themePalette: ThemePalette = readThemePalette(safeLocalStorage());
	let systemDark = window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;
	let resolvedTheme = resolveTheme(themeMode, systemDark);
	// O desktop não tem tela de login: um endereço de login ou desconhecido cai na tela
	// inicial em vez de abrir um formulário que não consegue autenticar.
	const fallbackRoute = defaultRoute(transportKind);
	const initialLocation = locationFromHash(location.hash, fallbackRoute);
	let route: Route = initialLocation.route;
	let courseId: string | null = initialLocation.courseId;
	let lessonPath: string | null = initialLocation.lessonPath;
	let user: User | null = null;
	let catalog: Catalog | null = null;
	let lesson: Lesson | null = null;
	let catalogQuery = '';
	let filteredNodes: CatalogNode[] = [];
	let sections: LessonSection[] = [];
	let recent: StudyProgress[] = [];
	let studySummary: CourseStudySummary[] = [];
	let studySummaryAvailable = false;
	let libraryPreferences: LibraryPreferences = emptyLibraryPreferences();
	let courseSort: 'recent' | 'title' | 'library' = 'recent';
	let courseFilter: 'all' | 'in-progress' | 'completed' = 'all';
	let inProgressCourseIds = new Set<string>();
	let courseCovers: Record<string, string> = {};
	let titleEditingCourseId: string | null = null;
	let titleDraft = '';
	let titleError = '';
	let coverError = '';
	let libraryNotice = '';
	let resumeItem: StudyProgress | null = null;
	let resumeLocation: ReturnType<typeof findLessonLocation> = null;
	$: course = findCourse(catalog, courseId);
	$: courseMeta = course ? formatCourseMeta(countCatalogNode(course)) : '';
	$: inProgressCourseIds = new Set(recent.filter((item) => !item.completed).map((item) => findLessonLocation(catalog, item.mediaId)?.courseId).filter((id): id is string => Boolean(id)));
	$: {
		let nodes = catalog ? filterCatalog(catalog.nodes, catalogQuery) : [];
		if (courseFilter !== 'all') nodes = nodes.filter((node) => {
			const id = courseIdOf(node);
			const total = countCatalogNode(node).videos;
			const completed = studySummary.find((item) => item.courseId === id)?.completedMedia ?? 0;
			const isComplete = studySummaryAvailable && total > 0 && completed >= total;
			return courseFilter === 'completed' ? isComplete : inProgressCourseIds.has(id) && !isComplete;
		});
		const displayTitles = libraryPreferences.displayTitles;
		if (courseSort === 'title') nodes.sort((left, right) => (displayTitles[courseIdOf(left)] ?? displayTitleFor(left.title)).localeCompare(displayTitles[courseIdOf(right)] ?? displayTitleFor(right.title), 'pt-BR', { sensitivity: 'base' }));
		if (courseSort === 'recent') {
			const latestByCourse = new Map<string, number>();
			for (const item of recent) {
				if (item.updatedAt === null) continue;
				const id = findLessonLocation(catalog, item.mediaId)?.courseId;
				if (id) latestByCourse.set(id, Math.max(latestByCourse.get(id) ?? 0, item.updatedAt));
			}
			nodes.sort((left, right) => (latestByCourse.get(courseIdOf(right)) ?? 0) - (latestByCourse.get(courseIdOf(left)) ?? 0));
		}
		filteredNodes = nodes;
	}
	// Navegação e índice saem da subárvore do curso aberto. Quando uma reindexação remove
	// a aula aberta, `neighboringLessons` devolve os dois lados nulos e os controles somem
	// em vez de apontar para uma aula que não existe mais.
	$: neighbors = course ? neighboringLessons([course], lessonPath) : { previous: null, next: null };
	$: sections = course ? lessonSections([course]) : [];
	$: lessonCount = sections.reduce((total, section) => total + section.lessons.length, 0);
	$: resumeItem = recent.find((item) => !item.completed && findLessonLocation(catalog, item.mediaId)) ?? null;
	$: resumeLocation = resumeItem ? findLessonLocation(catalog, resumeItem.mediaId) : null;
	function loadLibraryPreferences(userId: string) {
		libraryPreferences = readLibraryPreferences(safeLocalStorage(), userId);
	}
	function saveLibraryPreferences(next: LibraryPreferences) {
		libraryPreferences = next;
		if (writeLibraryPreferences(safeLocalStorage(), user?.userId ?? '', next)) {
			libraryNotice = 'Alterações salvas neste dispositivo.';
		} else {
			libraryNotice = 'Alterações aplicadas nesta sessão, mas não foi possível salvá-las neste dispositivo.';
		}
	}
	async function loadLibraryCovers(userId: string) {
		for (const coverUrl of Object.values(courseCovers)) URL.revokeObjectURL(coverUrl);
		courseCovers = {};
		const nodes = catalog?.nodes ?? [];
		const covers: Record<string, string> = {};
		for (const node of nodes) {
			try {
				const blob = await readLibraryCover(userId, courseIdOf(node));
				if (blob) covers[courseIdOf(node)] = URL.createObjectURL(blob);
			} catch {
				// Uma falha no armazenamento local usa a capa do próprio catálogo ou o fallback.
			}
		}
		courseCovers = covers;
	}
	function saveCourseTitle(courseItem: CatalogNode) {
		const title = titleDraft.trim();
		const issue = validateDisplayTitle(titleDraft);
		if (issue) {
			titleError = issue;
			return;
		}
		const displayTitles = { ...libraryPreferences.displayTitles };
		if (title === displayTitleFor(courseItem.title)) delete displayTitles[courseIdOf(courseItem)];
		else displayTitles[courseIdOf(courseItem)] = title;
		saveLibraryPreferences({ ...libraryPreferences, displayTitles });
		titleEditingCourseId = null;
		titleError = '';
	}
	function setLibraryView(view: LibraryView) {
		if (libraryPreferences.view !== view) saveLibraryPreferences({ ...libraryPreferences, view });
	}
	async function updateLibraryCover(courseItem: CatalogNode, event: Event) {
		const input = event.currentTarget;
		if (!(input instanceof HTMLInputElement)) return;
		const file = input.files?.[0];
		if (!file || !user) return;
		coverError = '';
		try {
			await writeLibraryCover(user.userId, courseIdOf(courseItem), file);
			const oldUrl = courseCovers[courseIdOf(courseItem)];
			if (oldUrl) URL.revokeObjectURL(oldUrl);
			courseCovers = { ...courseCovers, [courseIdOf(courseItem)]: URL.createObjectURL(file) };
			libraryNotice = 'Capa salva neste dispositivo; o arquivo original permaneceu intacto.';
		} catch (cause) {
			coverError = cause instanceof Error ? cause.message : 'Não foi possível salvar a capa.';
		} finally {
			input.value = '';
		}
	}
	async function resetLibraryCover(courseItem: CatalogNode) {
		if (!user) return;
		coverError = '';
		try {
			await removeLibraryCover(user.userId, courseIdOf(courseItem));
			const oldUrl = courseCovers[courseIdOf(courseItem)];
			if (oldUrl) URL.revokeObjectURL(oldUrl);
			const covers = { ...courseCovers };
			delete covers[courseIdOf(courseItem)];
			courseCovers = covers;
			libraryNotice = 'Capa personalizada removida; a capa do catálogo ou o padrão Graphite será usado.';
		} catch (cause) {
			coverError = cause instanceof Error ? cause.message : 'Não foi possível remover a capa.';
		}
	}
	let loading = true;
	let authBusy = false;
	let error = '';
	let username = '';
	let password = '';
	let currentPassword = '';
	let accountStatus = '';
	let accountError = '';
	let rescanning = false;
	let rescanMessage = '';
	let importing = false;
	let importFormOpen = false;
	let importPath = '';
	let importError = '';
	let removing = false;
	let removeConfirm = false;
	let removeError = '';
	let mediaError = '';
	let mediaRetry = 0;
	let recentLoading = false;
	let studyEnabled = studySyncSupported;

	// Só o admin importa e indexa: no servidor isso evita que um usuário comum aponte o
	// servidor para pastas arbitrárias; no desktop a identidade local é admin.
	$: canManage = user !== null && user.role === 'admin';

	applyTheme(document.documentElement, resolvedTheme);
	applyThemePalette(document.documentElement, themePalette);
	const scheme = window.matchMedia?.('(prefers-color-scheme: dark)');
	const updateSystemTheme = (event: MediaQueryListEvent) => {
		systemDark = event.matches;
		if (themeMode === 'system') {
			resolvedTheme = resolveTheme(themeMode, systemDark);
			applyTheme(document.documentElement, resolvedTheme);
			applyThemeColor(document.querySelector<HTMLMetaElement>('meta[name="theme-color"]'), themePalette, resolvedTheme);
		}
	};
	scheme?.addEventListener('change', updateSystemTheme);

	onMount(() => {
		const change = () => {
			const next = locationFromHash(location.hash, fallbackRoute);
			route = next.route;
			courseId = next.courseId;
			lessonPath = next.lessonPath;
			lesson = findLesson(catalog, courseId, lessonPath);
			removeConfirm = false;
			removeError = '';
			mediaError = '';
			mediaRetry = 0;
		};
		window.addEventListener('hashchange', change);
		void initialize();
		return () => {
			window.removeEventListener('hashchange', change);
			scheme?.removeEventListener('change', updateSystemTheme);
			for (const coverUrl of Object.values(courseCovers)) URL.revokeObjectURL(coverUrl);
		};
	});

	const shellPrefsKey = 'learnvault.shell.v1';
	function readShellPrefs(): { railCompact: boolean; indexCollapsed: boolean } {
		try {
			const raw = safeLocalStorage()?.getItem(shellPrefsKey);
			const parsed = raw ? (JSON.parse(raw) as Partial<{ railCompact: boolean; indexCollapsed: boolean }>) : null;
			return { railCompact: parsed?.railCompact === true, indexCollapsed: parsed?.indexCollapsed === true };
		} catch {
			return { railCompact: false, indexCollapsed: false };
		}
	}
	const initialShellPrefs = readShellPrefs();
	let railCompact = initialShellPrefs.railCompact;
	let indexCollapsed = initialShellPrefs.indexCollapsed;
	function persistShellPrefs() {
		try {
			safeLocalStorage()?.setItem(shellPrefsKey, JSON.stringify({ railCompact, indexCollapsed }));
		} catch {
			/* a preferência é opcional */
		}
	}
	function toggleRail() {
		railCompact = !railCompact;
		persistShellPrefs();
	}
	function toggleIndex() {
		indexCollapsed = !indexCollapsed;
		persistShellPrefs();
	}

	function safeLocalStorage(): Storage | null {
		try {
			return window.localStorage;
		} catch {
			return null;
		}
	}
	function changeTheme(mode: ThemeMode) {
		themeMode = mode;
		resolvedTheme = resolveTheme(mode, systemDark);
		applyTheme(document.documentElement, resolvedTheme);
		applyThemeColor(document.querySelector<HTMLMetaElement>('meta[name="theme-color"]'), themePalette, resolvedTheme);
		persistThemeMode(safeLocalStorage(), mode);
	}
	function changeThemePalette(palette: ThemePalette) {
		themePalette = palette;
		applyThemePalette(document.documentElement, palette);
		applyThemeColor(document.querySelector<HTMLMetaElement>('meta[name="theme-color"]'), palette, resolvedTheme);
		persistThemePalette(safeLocalStorage(), palette);
	}
	async function initialize() {
		try {
			const session = await resolveSession(() => transport.me(), transportKind);
			user = session.user;
			error = session.error;
			if (user) {
				loadLibraryPreferences(user.userId);
				catalog = await transport.catalog();
				await loadLibraryCovers(user.userId);
				lesson = findLesson(catalog, courseId, lessonPath);
				await loadRecent();
				if (route === 'login') route = 'home';
			}
		} catch {
			error = 'Não foi possível conectar ao serviço.';
		} finally {
			loading = false;
		}
	}

	async function login(event: SubmitEvent) {
		event.preventDefault();
		if (authBusy) return;
		error = '';
		authBusy = true;
		const destination = location.hash;
		try {
			user = await transport.login(username, password);
			loadLibraryPreferences(user.userId);
			password = '';
			catalog = await transport.catalog();
			await loadLibraryCovers(user.userId);
			lesson = findLesson(catalog, courseId, lessonPath);
			await loadRecent();
			location.hash = destinationAfterLogin(destination);
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível entrar.';
		} finally {
			authBusy = false;
		}
	}

	async function credentials(event: SubmitEvent) {
		event.preventDefault();
		if (authBusy) return;
		accountError = '';
		accountStatus = '';
		authBusy = true;
		try {
			user = await transport.updateCredentials(username, password, currentPassword);
			username = '';
			password = '';
			currentPassword = '';
			accountStatus = 'Credenciais atualizadas. Use-as no próximo acesso.';
		} catch (cause) {
			accountError = cause instanceof Error ? cause.message : 'Não foi possível atualizar as credenciais.';
		} finally {
			authBusy = false;
		}
	}

	async function logout() {
		await transport.logout();
		user = null;
		catalog = null;
		lesson = null;
		recent = [];
		username = '';
		password = '';
		currentPassword = '';
		location.hash = href('login');
	}

	function syncCatalog(nextCatalog: Catalog) {
		catalog = nextCatalog;
		if (user) void loadLibraryCovers(user.userId);
		lesson = findLesson(nextCatalog, courseId, lessonPath);
		mediaError = '';
		mediaRetry = 0;
	}

	async function rescan() {
		rescanning = true;
		rescanMessage = 'Lendo os cursos…';
		try {
			await transport.rescan();
			syncCatalog(await transport.catalog());
			await loadRecent();
			rescanMessage = 'Catálogo atualizado.';
		} catch (cause) {
			rescanMessage = cause instanceof Error ? cause.message : 'Falha ao indexar.';
		} finally {
			rescanning = false;
		}
	}

	// O webview só dispara o seletor nativo; nenhum caminho cruza o IPC. Cancelar preserva
	// a lista anterior, e uma falha é recuperável — o botão volta a ficar ativo. Importar
	// já indexa o curso novo, como a antiga seleção de pasta fazia.
	async function importCourse() {
		importing = true;
		importError = '';
		try {
			await transport.importCourse();
			await transport.rescan();
			syncCatalog(await transport.catalog());
			await loadRecent();
		} catch (cause) {
			importError = cause instanceof Error ? cause.message : 'Não foi possível importar o curso.';
		} finally {
			importing = false;
		}
	}

	// No servidor o navegador não enxerga o disco: o admin informa o caminho da pasta e o
	// servidor valida e registra; a indexação roda em seguida, como no desktop.
	async function submitImport(event: SubmitEvent) {
		event.preventDefault();
		importing = true;
		importError = '';
		try {
			await transport.importCoursePath(importPath.trim());
			importPath = '';
			importFormOpen = false;
			await transport.rescan();
			syncCatalog(await transport.catalog());
			await loadRecent();
		} catch (cause) {
			importError = cause instanceof Error ? cause.message : 'Não foi possível importar o curso.';
		} finally {
			importing = false;
		}
	}

	async function removeCourse() {
		if (!course) return;
		removing = true;
		removeError = '';
		try {
			await transport.removeCourse(course.courseId ?? course.path);
			courseId = null;
			lessonPath = null;
			lesson = null;
			removeConfirm = false;
			location.hash = href('home');
			syncCatalog(await transport.catalog());
		} catch (cause) {
			removeError = cause instanceof Error ? cause.message : 'Não foi possível remover o curso.';
		} finally {
			removing = false;
		}
	}

	async function loadRecent() {
		if (!studySyncSupported) {
			studySummary = [];
			studySummaryAvailable = false;
			return;
		}
		recentLoading = true;
		try {
			const result = await transport.recentStudy(6);
			recent = result.items;
		} catch (cause) {
			recent = [];
			if (cause && typeof cause === 'object' && (cause as { code?: string }).code === STUDY_NOT_AVAILABLE) {
				studyEnabled = false;
			}
		} finally {
			recentLoading = false;
		}
		try {
			studySummary = await transport.studySummary();
			studySummaryAvailable = true;
		} catch (cause) {
			studySummary = [];
			studySummaryAvailable = false;
			if (cause && typeof cause === 'object' && (cause as { code?: string }).code === STUDY_NOT_AVAILABLE) studyEnabled = false;
		}
	}

	function mediaFailed() {
		// O backend responde 503 sob rajada de requisições; uma nova tentativa recupera a mídia.
		if (mediaRetry < 2) {
			mediaRetry += 1;
			setTimeout(() => {
				mediaError = '';
			}, 900 * mediaRetry);
			return;
		}
		mediaError = 'Não foi possível carregar esta mídia.';
	}

	// Mantém a aula atual visível dentro da caixa do índice sem arrastar a página: o
	// ajuste é feito somente no contêiner rolável do índice, com o mínimo de rolagem.
	function revealActive(node: HTMLElement, active: boolean) {
		function reveal(): void {
			if (!active) return;
			const box = node.closest('.lesson-index-body');
			if (!(box instanceof HTMLElement)) return;
			const boxRect = box.getBoundingClientRect();
			const nodeRect = node.getBoundingClientRect();
			if (nodeRect.top < boxRect.top) box.scrollTop -= boxRect.top - nodeRect.top + 8;
			else if (nodeRect.bottom > boxRect.bottom) box.scrollTop += nodeRect.bottom - boxRect.bottom + 8;
		}
		reveal();
		return {
			update(next: boolean): void {
				active = next;
				reveal();
			}
		};
	}
</script>
<svelte:head><title>LearnVault — sua biblioteca de estudos</title><meta name="description" content="Sua biblioteca pessoal de cursos." /></svelte:head>
{#if loading}<div class="loading" aria-live="polite">Carregando seus cursos…</div>
{:else if !user}
	<main class="login-page" aria-labelledby="login-heading">
		<aside class="login-institutional" aria-label="LearnVault">
			<div class="brand login-brand"><Brand /> <span>LEARNVAULT</span></div>
			<div class="login-identity">
				<span class="login-kicker">SEU ESPAÇO DE APRENDIZAGEM</span>
				<p class="login-statement">Aprender com<br />continuidade.</p>
				<p class="login-description">Reúna seus cursos e retome seus estudos no seu próprio ritmo.</p>
				<div class="login-motif" aria-hidden="true"><span></span><span></span><span></span></div>
			</div>
		</aside>
		<section class="login-content">
			<div class="brand login-mobile-brand"><Brand /> <span>LEARNVAULT</span></div>
			<div class="login-form-content">
				<h1 id="login-heading">Boas-vindas de volta</h1>
				<p class="intro">Acesse sua biblioteca de cursos.</p>
				<form onsubmit={login}>
					<fieldset class="auth-fields" disabled={authBusy}>
						<label class="form-field" for="login-username">Usuário<input id="login-username" autocomplete="username" autocapitalize="none" spellcheck={false} bind:value={username} required /></label>
						<PasswordField id="login-password" label="Senha" autocomplete="current-password" bind:value={password} />
						{#if error}<p class="error" role="alert">{error}</p>{/if}
						<button type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Entrando…' : 'Entrar'}</button>
					</fieldset>
				</form>
			</div>
		</section>
	</main>
{:else}<div class="shell" class:rail-compact={railCompact}><aside class="rail"><div class="rail-brand"><Brand /><span class="rail-wordmark">LEARNVAULT</span></div><button class="rail-toggle" type="button" onclick={toggleRail} aria-expanded={!railCompact} aria-label={railCompact ? 'Expandir menu' : 'Recolher menu'} data-label={railCompact ? 'Expandir menu' : 'Recolher menu'}><svg class="rail-icon" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={railCompact ? 'M8 5l5 5-5 5' : 'M12 5l-5 5 5 5'} /></svg><span class="rail-label">{railCompact ? 'Expandir' : 'Recolher'}</span></button><nav aria-label="Navegação principal"><a data-label="Aprender" aria-current={route === 'home' || route === 'course' || route === 'player' ? 'page' : undefined} class:active={route === 'home' || route === 'course' || route === 'player'} href={href('home')}><svg class="rail-icon" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3.5 4.5h3.2A3.3 3.3 0 0 1 10 7.8v8.7a3.3 3.3 0 0 0-3.3-3.3H3.5z" /><path d="M16.5 4.5h-3.2A3.3 3.3 0 0 0 10 7.8v8.7a3.3 3.3 0 0 1 3.3-3.3h3.2z" /></svg><span class="rail-label">Aprender</span></a><a data-label="Anotações" aria-current={route === 'notes' ? 'page' : undefined} class:active={route === 'notes'} href={href('notes')}><svg class="rail-icon" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5.5 3.5h9v13l-4.5-2.7-4.5 2.7z" /></svg><span class="rail-label">Anotações</span></a><a data-label="Estatísticas" aria-current={route === 'analytics' ? 'page' : undefined} class:active={route === 'analytics'} href={href('analytics')}><svg class="rail-icon" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true"><path d="M4 16v-5M10 16V4M16 16v-8" /></svg><span class="rail-label">Estatísticas</span></a></nav><div class="rail-foot"><Pomodoro /><details class="account-details"><summary class="account-avatar" aria-label={`Conta de ${user.username}`}>{user.username.slice(0, 1).toLocaleUpperCase()}</summary><div class="account-menu"><strong>{user.username}</strong><a href={href('settings')}>Configurações da conta</a>{#if transportKind === 'http'}<button class="text-button" onclick={logout}>Encerrar sessão</button>{/if}</div></details></div></aside><div class="layout"><main class:home={route === 'home'}>{#if error}<p class="error" role="alert">{error}</p>{/if}
		{#if route === 'home'}
			{#if catalog && catalog.nodes.length === 0}
				<section class="library-welcome" aria-labelledby="library-welcome-title">
					<div class="library-welcome-art" aria-hidden="true">
						<svg viewBox="0 0 96 80" fill="none"><path d="M13 17.5c11-3.8 23-2.4 35 4.3v43c-12.4-6.4-24-7.4-35-3.1v-44.2Zm70 0c-11-3.8-23-2.4-35 4.3v43c12.4-6.4 24-7.4 35-3.1V17.5Z" stroke="currentColor" stroke-width="3" stroke-linejoin="round"/><path d="M48 22v43M23 29c7-1 14 .2 19 3m31-3c-7-1-14 .2-19 3M23 39c7-1 14 .2 19 3m31-3c-7-1-14 .2-19 3" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/><path d="M5 12h12M79 9h12M8 70h15" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>
					</div>
					<h1 id="library-welcome-title">Sua biblioteca começa aqui</h1>
					<p>Adicione um curso para organizar suas aulas e continuar de onde parou.</p>
					{#if canManage}
						{#if transportKind === 'desktop'}<button class="primary library-welcome-action" onclick={importCourse} disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar primeiro curso'}</button>{:else}<button class="primary library-welcome-action" onclick={() => (importFormOpen = !importFormOpen)} aria-expanded={importFormOpen} aria-controls="import-course-form">Importar primeiro curso</button>{/if}
						<p class="library-welcome-safety"><svg viewBox="0 0 20 20" fill="none" aria-hidden="true"><path d="M10 2.5 16 5v4.7c0 3.5-2.5 6.2-6 7.8-3.5-1.6-6-4.3-6-7.8V5l6-2.5Z" stroke="currentColor" stroke-width="1.5"/><path d="m7.2 9.8 1.8 1.8 3.8-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>Seus arquivos permanecem na pasta original. O LearnVault apenas organiza o acesso a eles.</p>
					{:else}
						<p class="library-welcome-safety">Peça ao administrador para adicionar cursos à biblioteca.</p>
					{/if}
				</section>
			{:else}
				<header class="home-heading library-heading" aria-labelledby="home-heading">
					<div><h1 id="home-heading">Minha biblioteca</h1><p>Continue de onde parou ou escolha um curso.</p></div>
					{#if canManage}
						<div class="library-heading-actions">
							{#if catalog && catalog.nodes.length}<button class="library-rescan" onclick={rescan} disabled={rescanning} aria-busy={rescanning}>{rescanning ? 'Atualizando…' : 'Atualizar'}</button>{/if}
							{#if transportKind === 'desktop'}<button class="primary" onclick={importCourse} disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar curso'}</button>{:else}<button class="primary" onclick={() => (importFormOpen = !importFormOpen)} aria-expanded={importFormOpen}>Importar curso</button>{/if}
						</div>
					{/if}
				</header>
			{/if}
			{#if route === 'home' && catalog && canManage && importFormOpen}<form id="import-course-form" class="import-form" onsubmit={submitImport}><label for="import-path">Caminho da pasta do curso no servidor</label><input id="import-path" bind:value={importPath} placeholder="C:\cursos\Meu curso" required /><div class="import-form-actions"><button class="primary" disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar'}</button><button type="button" class="text-button" onclick={() => (importFormOpen = false)}>Cancelar</button></div></form>{/if}
			{#if importError}<p class="error" role="alert">{importError}</p>{/if}
			{#if rescanMessage}<p class="catalog-message" role="status">{rescanMessage}</p>{/if}
			{#if catalog && catalog.nodes.length > 0}
				{#if studyEnabled && resumeItem && resumeLocation}
			{@const resumeCourse = findCourse(catalog, resumeLocation.courseId)}
			{@const resumeTitle = resumeCourse ? (libraryPreferences.displayTitles[courseIdOf(resumeCourse)] ?? displayTitleFor(resumeCourse.title)) : displayTitleFor(resumeLocation.courseTitle)}
			{@const resumeCourseId = resumeCourse ? courseIdOf(resumeCourse) : ''}
			{@const resumeCoverImage = resumeCourse ? preferredCourseCover(resumeCourse) : null}
			{@const resumeCoverSrc = resumeCourse ? (courseCovers[resumeCourseId] ?? (resumeCoverImage ? transport.mediaUrl(resumeCoverImage, resumeCourseId) ?? '' : '')) : ''}
			{@const resumeCompleted = studySummary.find((item) => item.courseId === resumeCourseId)?.completedMedia ?? 0}
			{@const resumeTotal = resumeCourse ? countCatalogNode(resumeCourse).videos : 0}
			{@const resumePercent = studySummaryAvailable && resumeCompleted > 0 && resumeTotal > 0 ? Math.min(100, Math.round((resumeCompleted / resumeTotal) * 100)) : null}
			<section class="library-resume" aria-labelledby="resume-title">
				<h2 id="resume-title">Continue estudando</h2>
				<div class="library-resume-card">
					<div class="library-resume-cover" class:has-image={Boolean(resumeCoverSrc)} aria-hidden="true">
						{#if resumeCoverSrc}<img src={resumeCoverSrc} alt="" onerror={(event) => event.currentTarget.parentElement?.classList.remove('has-image')} />{/if}
						<span>{displayTitleFor(resumeTitle).slice(0, 2).toLocaleUpperCase()}</span>
					</div>
					<div class="library-resume-info">
						<p class="library-course-kicker">{resumeTitle}</p>
						<h3>{resumeLocation.lesson.title}</h3>
						{#if resumePercent !== null}<div class="library-progress" role="img" aria-label={`${resumePercent}% das mídias deste curso concluídas`}><span style:width={`${resumePercent}%`}></span></div>{/if}
						<p class="library-resume-position">Posição salva · {formatDuration(resumeItem.positionSeconds)}{resumeLocation.moduleTitle ? ` · ${resumeLocation.moduleTitle}` : ''}</p>
					</div>
					<a class="library-continue" href={href('player', resumeLocation.courseId, resumeLocation.lesson.path)}>Continuar <span aria-hidden="true">→</span></a>
				</div>
			</section>
		{/if}

				<section class="library-courses" aria-labelledby="library-courses-title">
					<div class="library-section-heading library-course-heading"><h2 id="library-courses-title">Cursos</h2><span>{filteredNodes.length} {filteredNodes.length === 1 ? 'curso' : 'cursos'}</span></div>
					<div class="library-tools" role="group" aria-label="Filtros e visualização da biblioteca">
						<label class="library-search"><span class="visually-hidden">Buscar cursos</span><input type="search" placeholder="Buscar cursos" bind:value={catalogQuery} /></label>
						<div class="library-filter" role="group" aria-label="Filtrar cursos">
							<button class:active={courseFilter === 'all'} aria-pressed={courseFilter === 'all'} onclick={() => (courseFilter = 'all')}>Todos</button>
							<button class:active={courseFilter === 'in-progress'} aria-pressed={courseFilter === 'in-progress'} onclick={() => (courseFilter = 'in-progress')}>Em andamento</button>
							<button class:active={courseFilter === 'completed'} aria-pressed={courseFilter === 'completed'} onclick={() => (courseFilter = 'completed')}>Concluídos</button>
						</div>
						<label class="library-sort-label">Ordenar
					<select aria-label="Ordenar cursos" bind:value={courseSort}>
						<option value="recent">Recentes</option><option value="title">Nome</option><option value="library">Ordem original</option>
					</select>
				</label>
				<div class="library-view" role="group" aria-label="Modo de visualização">
					<button aria-label="Visualização em grade" aria-pressed={libraryPreferences.view === 'grid'} onclick={() => setLibraryView('grid')}>Grade</button>
					<button aria-label="Visualização em lista" aria-pressed={libraryPreferences.view === 'list'} onclick={() => setLibraryView('list')}>Lista</button>
				</div>
			</div>
			{#if libraryNotice}<p class="library-notice" role="status">{libraryNotice}</p>{/if}
			{#if filteredNodes.length}
				<ul class="library-grid" class:list-mode={libraryPreferences.view === 'list'}>
					{#each filteredNodes as courseItem (courseIdOf(courseItem))}
						{@const courseItemId = courseIdOf(courseItem)}
						{@const courseDisplayTitle = libraryPreferences.displayTitles[courseItemId] ?? displayTitleFor(courseItem.title)}
						{@const courseCoverImage = preferredCourseCover(courseItem)}
						{@const courseImageSrc = courseCovers[courseItemId] ?? (courseCoverImage ? transport.mediaUrl(courseCoverImage, courseItemId) ?? '' : '')}
						{@const courseCompleted = studySummary.find((item) => item.courseId === courseItemId)?.completedMedia ?? 0}
						{@const courseMediaTotal = countCatalogNode(courseItem).videos}
						{@const coursePercent = studySummaryAvailable && courseCompleted > 0 && courseMediaTotal > 0 ? Math.min(100, Math.round((courseCompleted / courseMediaTotal) * 100)) : null}
						<li class="library-card-item">
							<CourseCard course={courseItem} displayTitle={courseDisplayTitle} layout={libraryPreferences.view} coverSrc={courseImageSrc} profile={user.userId} mediaUrl={transport.mediaUrl} progressPercent={coursePercent} progressLabel={inProgressCourseIds.has(courseItemId) ? 'Em andamento' : ''} />
							{#if titleEditingCourseId === courseItemId}
								<form class="library-title-form" onsubmit={(event) => { event.preventDefault(); void saveCourseTitle(courseItem); }}>
									<label for={`course-title-${courseItemId}`}>Título do curso</label>
									<div><input id={`course-title-${courseItemId}`} bind:value={titleDraft} maxlength="120" required /><button type="submit">Salvar</button><button type="button" class="quiet-button" onclick={() => (titleEditingCourseId = null)}>Fechar</button></div>
									{#if titleError}<small role="alert">{titleError}</small>{/if}
									<label for={`course-cover-${courseItemId}`}>Capa personalizada</label>
									<input id={`course-cover-${courseItemId}`} type="file" accept="image/*" onchange={(event) => { void updateLibraryCover(courseItem, event); }} />
									{#if coverError}<small role="alert">{coverError}</small>{/if}
									<button type="button" class="quiet-button" onclick={() => { void resetLibraryCover(courseItem); }}>Usar capa disponível do curso</button>
								</form>
							{:else}
								<details class="library-card-actions"><summary aria-label={`Ações de ${courseDisplayTitle}`} title="Ações do curso">•••</summary><div><button onclick={(event) => { const menu = event.currentTarget.closest('details'); if (menu) menu.open = false; titleEditingCourseId = courseItemId; titleDraft = courseDisplayTitle; titleError = ''; coverError = ''; }}>Personalizar</button></div></details>
							{/if}
						</li>
					{/each}
				</ul>
			{:else}
						<div class="library-empty" role="status"><h3>Nenhum curso encontrado</h3><p>Tente outro título de busca ou filtro.</p></div>
			{/if}
		</section>
	{/if}
	{#if catalog && catalog.looseFiles.length}<LooseFiles files={catalog.looseFiles} heading="Arquivos soltos" />{/if}
{:else if route === 'notes'}<NotesIndex {catalog} />
{:else if route === 'analytics'}<StudyAnalytics />
{:else if route === 'settings'}<h1>Preferências</h1><p class="intro">Escolha como o LearnVault aparece neste dispositivo.</p><fieldset class="theme-options"><legend>Tema</legend><label><input type="radio" name="theme" value="system" checked={themeMode === 'system'} onchange={() => changeTheme('system')} />Sistema <span>Acompanha a preferência do sistema.</span></label><label><input type="radio" name="theme" value="light" checked={themeMode === 'light'} onchange={() => changeTheme('light')} />Claro <span>Usa sempre o tema claro.</span></label><label><input type="radio" name="theme" value="dark" checked={themeMode === 'dark'} onchange={() => changeTheme('dark')} />Escuro <span>Usa sempre o tema escuro.</span></label></fieldset><p class="theme-status" aria-live="polite">Tema atual: {resolvedTheme === 'dark' ? 'escuro' : 'claro'}{themeMode === 'system' ? ' (sistema)' : ''}</p><fieldset class="palette-options"><legend>Paleta de cores</legend><label><input type="radio" name="palette" value="blue-slate" checked={themePalette === 'blue-slate'} onchange={() => changeThemePalette('blue-slate')} /><span class="palette-swatch blue-slate" aria-hidden="true"></span><span class="palette-copy"><strong>Blue-slate</strong><small>Azul acinzentado, equilibrado e discreto.</small></span></label><label><input type="radio" name="palette" value="deep-petrol" checked={themePalette === 'deep-petrol'} onchange={() => changeThemePalette('deep-petrol')} /><span class="palette-swatch deep-petrol" aria-hidden="true"></span><span class="palette-copy"><strong>Deep Petrol</strong><small>Petróleo dessaturado com acento teal.</small></span></label><label><input type="radio" name="palette" value="balanced-sage" checked={themePalette === 'balanced-sage'} onchange={() => changeThemePalette('balanced-sage')} /><span class="palette-swatch balanced-sage" aria-hidden="true"></span><span class="palette-copy"><strong>Balanced Sage</strong><small>Neutros suaves com acento sálvia.</small></span></label></fieldset><section class="account-settings" aria-labelledby="account-heading"><h2 id="account-heading">Conta</h2><p><strong>{user.username}</strong> · {user.isAdmin ? 'Administrador' : 'Estudante'}</p><form class="credentials-form" onsubmit={credentials}><fieldset class="auth-fields" disabled={authBusy}><legend>Credenciais de acesso</legend><PasswordField id="settings-current-password" label="Senha atual" autocomplete="current-password" bind:value={currentPassword} /><label class="form-field" for="settings-new-username">Novo usuário<input id="settings-new-username" autocomplete="username" bind:value={username} required /></label><PasswordField id="settings-new-password" label="Nova senha" autocomplete="new-password" bind:value={password} minLength={12} hint="Use ao menos 12 caracteres." />{#if accountError}<p class="error" role="alert">{accountError}</p>{/if}{#if accountStatus}<p class="catalog-message" role="status">{accountStatus}</p>{/if}<button type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Salvando…' : 'Atualizar credenciais'}</button></fieldset></form>{#if transportKind === 'http'}<button class="text-button" onclick={logout}>Encerrar sessão</button>{:else}<p class="catalog-message">Biblioteca local neste dispositivo.</p>{/if}</section>
{:else if route === 'course'}
	<a class="back" href={href('home')}>← Início</a>
	{#if !course}<h1>Curso não encontrado</h1><p class="empty" role="status">Não foi possível encontrar este curso no catálogo atual. Ele pode ter sido removido.</p>
	{:else}
		{@const courseKey = courseIdOf(course)}
		{@const courseCoverInfo = courseCover(course.title)}
		{@const courseCoverImage = preferredCourseCover(course)}
		{@const courseHeroSrc = courseCovers[courseKey] ?? (courseCoverImage ? transport.mediaUrl(courseCoverImage, courseKey) ?? '' : '')}
		{@const courseSummary = studySummary.find((item) => item.courseId === courseKey)}
		{@const courseMediaTotal = countCatalogNode(course).videos}
		{@const coursePercent = studySummaryAvailable && courseSummary && courseSummary.completedMedia > 0 && courseMediaTotal > 0 ? Math.min(100, Math.round((courseSummary.completedMedia / courseMediaTotal) * 100)) : null}
		{@const courseResumeLocation = resumeLocation && resumeLocation.courseId === courseKey ? resumeLocation : null}
		<header class="course-hero">
			<div class="course-hero-cover" class:has-image={Boolean(courseHeroSrc)} style:--cover-from={courseCoverInfo.from} style:--cover-to={courseCoverInfo.to} aria-hidden="true">
				{#if courseHeroSrc}<img src={courseHeroSrc} alt="" />{/if}
				<span>{courseCoverInfo.monogram}</span>
			</div>
			<div class="course-hero-body">
				<p class="course-kicker">Curso</p>
				<h1>{course.title}</h1>
				{#if courseMeta}<p class="course-meta">{courseMeta}</p>{/if}
				{#if coursePercent !== null}<div class="course-hero-progress"><div class="library-progress" role="img" aria-label={`${coursePercent}% das aulas deste curso concluídas`}><span style:width={`${coursePercent}%`}></span></div><small>{coursePercent}% concluído</small></div>{/if}
				{#if courseResumeLocation || canManage}<div class="course-hero-actions">
					{#if courseResumeLocation}<a class="primary" href={href('player', courseKey, courseResumeLocation.lesson.path)}>Continuar estudando <span aria-hidden="true">→</span></a>{/if}
				</div>{/if}
			</div>
		</header>
		{#if canManage}<div class="course-actions">{#if removeConfirm}<span class="remove-question" role="alert">Remover “{course.title}” da biblioteca? Os arquivos não serão apagados.</span><button class="text-button danger" onclick={removeCourse} disabled={removing} aria-busy={removing}>{removing ? 'Removendo…' : 'Remover curso'}</button><button class="text-button" onclick={() => (removeConfirm = false)} disabled={removing}>Cancelar</button>{:else}<button class="text-button" onclick={() => (removeConfirm = true)}>Remover curso</button><button class="text-button" onclick={rescan} disabled={rescanning}>{rescanning ? 'Indexando…' : 'Indexar cursos'}</button>{/if}</div>{/if}
		{#if removeError}<p class="error" role="alert">{removeError}</p>{/if}
		{#if rescanMessage}<p class="catalog-message" aria-live="polite">{rescanMessage}</p>{/if}
		{#if course.looseFiles.length}<LooseFiles files={course.looseFiles} heading="Arquivos soltos" courseId={course.courseId ?? course.path} />{/if}
		{#if course.children.length}<div class="course-body"><CatalogTree nodes={course.children} courseId={course.courseId ?? course.path} expandAll={true} /></div>{:else}<p class="empty" role="status">Este curso ainda não foi indexado. Use “Indexar cursos” para montar o catálogo.</p>{/if}
	{/if}
{:else}
	{#if !lesson}<h1>Aula não encontrada</h1><p class="empty" role="status">Não foi possível encontrar esta aula no catálogo atual.</p>{:else}
		<a class="back" href={course ? href('course', course.courseId ?? course.path) : href('home')}>← {course ? course.title : 'Início'}</a>
		<div class="player-page" class:index-collapsed={indexCollapsed}>
			<div class="player-stage">
				<header class="player-heading"><h1>{lesson.title}</h1></header>
				<Player {lesson} courseId={courseId ?? ''} {mediaError} onMediaError={mediaFailed} {studyEnabled} />
			</div>
		{#if lessonCount > 1}<aside class="lesson-index" aria-labelledby="lesson-index-title"><div class="lesson-index-head"><h2 id="lesson-index-title">Índice de aulas</h2><span>{lessonCount} aulas</span><button class="index-toggle" type="button" onclick={toggleIndex} aria-expanded={!indexCollapsed} aria-label={indexCollapsed ? 'Expandir índice de aulas' : 'Recolher índice de aulas'} data-label={indexCollapsed ? 'Expandir índice' : 'Recolher índice'}><svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={indexCollapsed ? 'M12 5l-5 5 5 5' : 'M8 5l5 5-5 5'} /></svg></button><span class="index-mini" aria-hidden="true">{sections.flatMap((s) => s.lessons).findIndex((l) => l.path === lesson?.path) + 1}</span></div><div class="lesson-index-body">{#each sections as section}{#if section.title}<h3>{section.title}</h3>{/if}<ol>{#each section.lessons as lessonItem}<li><a class:active={lessonItem.path === lesson.path} href={href('player', courseId ?? '', lessonItem.path)} aria-current={lessonItem.path === lesson.path ? 'page' : undefined} use:revealActive={lessonItem.path === lesson.path}>{lessonItem.title}</a></li>{/each}</ol>{/each}</div></aside>{/if}
		</div>
		{#if neighbors.previous || neighbors.next}<nav class="lesson-nav" aria-label="Aulas vizinhas">{#if neighbors.previous}<a href={href('player', courseId ?? '', neighbors.previous.path)}>← {neighbors.previous.title}</a>{/if}{#if neighbors.next}<a href={href('player', courseId ?? '', neighbors.next.path)}>{neighbors.next.title} →</a>{/if}</nav>{/if}
	{/if}
{/if}
</main></div></div>{/if}
