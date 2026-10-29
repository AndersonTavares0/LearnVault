<script lang="ts">
	import { onMount } from 'svelte';
	import LooseFiles from './LooseFiles.svelte';
	import CatalogTree from './CatalogTree.svelte';
	import CourseCard from './CourseCard.svelte';
	import CourseCover from './CourseCover.svelte';
	import Player from './Player.svelte';
	import Statistics from './Statistics.svelte';
	import Pomodoro from './Pomodoro.svelte';
	import MiniPomodoro from './MiniPomodoro.svelte';
	import DesktopTitlebar from './DesktopTitlebar.svelte';
	import { filterCatalog } from './search';
	import { collectMaterials, countCatalogNode, formatCourseMeta, formatLessonCounts, formatLocationContext } from './catalog';
	import { courseCover } from './cover';
	import PasswordField from './PasswordField.svelte';
	import { defaultLessonPath, lessonContainer, neighboringLessons } from './navigation';
	import { destinationAfterLogin, findCourse, findLesson, findLessonLocation, href, locationFromHash, type LessonLocation, type Route } from './router';
	import { transport, transportKind, COVER_ACCEPT, MAX_COVER_BYTES, courseIdOf, STUDY_NOT_AVAILABLE, type Catalog, type CatalogNode, type Lesson, type StudyProgress, type UpdateStatus, type User } from './transport';
	import { updateOutcome, updateMessage, updateReleaseUrl } from './update';
	import { defaultRoute, resolveSession } from './session';
	import { formatDuration, studySyncSupported } from './player';
	import { createDefaultPlayerPreferences, playerPreferencesScope, readPlayerPreferences, writePlayerPreferences, type PlayerPreferences } from './player-preferences';
	import Brand from './Brand.svelte';
	import { applyTheme, persistThemeMode, readThemeMode, resolveTheme, type ThemeMode } from './theme';
	import { LIBRARY_VIEWS, persistLibraryView, readLibraryView, type LibraryView } from './library-view';

	let themeMode: ThemeMode = readThemeMode(safeLocalStorage());
	let railCompact = readRailCompact();
	let libraryView: LibraryView = readLibraryView(safeLocalStorage());
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
	$: course = findCourse(catalog, courseId);
	$: filteredNodes = catalog ? filterCatalog(catalog.nodes, catalogQuery) : [];
	// `#/course/<id>` é a visão geral do curso; a aula só abre quando o caminho está na
	// URL. O endereço legado `#/player/<curso>/<aula>` continua abrindo o player.
	$: isOverview = route === 'course' && lessonPath === null;
	$: isPlayer = (route === 'course' && lessonPath !== null) || (route === 'player' && courseId !== null);
	$: isCourseRoute = isOverview || isPlayer;
	// A aula da URL só vale se ainda existir; um caminho removido por reindexação cai na
	// aula inicial. Sem aula na URL, o histórico recente decide, e só depois a primeira
	// aula disponível da ordem canônica.
	$: recentMediaIds = recent.map((item) => item.mediaId);
	$: requestedLessonPath = lessonPath !== null && findLesson(catalog, courseId, lessonPath) !== null ? lessonPath : null;
	$: activePath = requestedLessonPath ?? (course ? defaultLessonPath([course], recentMediaIds) : null);
	$: lesson = findLesson(catalog, courseId, activePath);
	$: activeContainer = course && activePath ? lessonContainer(course.children, activePath) : null;
	$: locatedRecent = recent.flatMap((item) => {
		const location = findLessonLocation(catalog, item.mediaId);
		return location ? [{ item, location }] : [];
	});
	$: libraryRecent = locatedRecent.slice(0, 4);
	$: courseRecent = course ? locatedRecent.find((entry) => entry.location.courseId === courseIdOf(course)) ?? null : null;
	$: userInitials = user
		? user.username
				.trim()
				.split(/[\s._-]+/)
				.filter(Boolean)
				.slice(0, 2)
				.map((word) => word[0]?.toUpperCase() ?? '')
				.join('') || '?'
		: '';
	// Navegação sai da subárvore do curso aberto. Quando uma reindexação remove a aula
	// aberta, `neighboringLessons` devolve os dois lados nulos e os controles somem em vez
	// de apontar para uma aula que não existe mais.
	$: neighbors = course ? neighboringLessons([course], activePath) : { previous: null, next: null };
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
	let courseMenuOpen = false;
	let courseTab: 'content' | 'materials' = 'content';
	let focusMode = false;

	function courseTabKey(event: KeyboardEvent) {
		if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight' && event.key !== 'Home' && event.key !== 'End') return;
		event.preventDefault();
		const next = event.key === 'Home' ? 'content' : event.key === 'End' ? 'materials' : courseTab === 'content' ? 'materials' : 'content';
		courseTab = next;
		setTimeout(() => document.getElementById(`course-tab-${next}`)?.focus(), 0);
	}
	let mediaError = '';
	let coverBusy = false;
	let coverError = '';
	let coverMessage = '';
	let coverTargetId: string | null = null;
	let coverMenuId: string | null = null;
	let coverInput: HTMLInputElement | null = null;
	let orderBusy = false;
	let orderError = '';
	let orderMessage = '';
	let recent: StudyProgress[] = [];
	let recentLoading = false;
	let studyEnabled = studySyncSupported;
	// Estado mínimo da verificação de atualização: o último resultado, se uma checagem
	// está em curso e se a última foi manual (para só mostrar erro após uma ação do
	// usuário, e nunca durante a abertura da tela).
	let updateStatus: UpdateStatus | null = null;
	let updateChecking = false;
	let updateManual = false;
	let updateRequested = false;

	// Carrega o estado da atualização ao abrir Configurações (checagem não forçada, que o
	// servidor serve do cache quando dentro do intervalo mínimo). Só o transporte HTTP tem
	// o endpoint; o desktop fica sem canal de atualização.
	$: if (transportKind === 'http' && route === 'settings' && user && !updateRequested) {
		updateRequested = true;
		void checkUpdate(false);
	}
	let playerPreferences: PlayerPreferences = createDefaultPlayerPreferences();

	// Só o admin importa e indexa: no servidor isso evita que um usuário comum aponte o
	// servidor para pastas arbitrárias; no desktop a identidade local é admin.
	$: canManage = user !== null && user.role === 'admin';

	applyTheme(document.documentElement, resolvedTheme);
	const scheme = window.matchMedia?.('(prefers-color-scheme: dark)');
	const updateSystemTheme = (event: MediaQueryListEvent) => {
		systemDark = event.matches;
		if (themeMode === 'system') {
			resolvedTheme = resolveTheme(themeMode, systemDark);
			applyTheme(document.documentElement, resolvedTheme);
			const themeColor = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
			if (themeColor) themeColor.content = resolvedTheme === 'dark' ? '#151b1d' : '#eceeea';
		}
	};
	scheme?.addEventListener('change', updateSystemTheme);

	onMount(() => {
		const change = () => {
			const next = locationFromHash(location.hash, fallbackRoute);
			route = next.route;
			courseId = next.courseId;
			lessonPath = next.lessonPath;
			courseMenuOpen = false;
			removeConfirm = false;
			removeError = '';
			coverBusy = false;
			coverError = '';
			coverMessage = '';
			coverTargetId = null;
			coverMenuId = null;
			mediaError = '';
			courseTab = 'content';
			focusMode = false;
		};
		// Um clique fora dos menus (ou em outra parte da página) fecha as ações abertas; o
		// botão e o painel do próprio menu ficam de fora para que a ação escolhida funcione.
		const closeMenus = (event: PointerEvent) => {
			const target = event.target;
			if (!(target instanceof Element) || !target.closest('.course-menu, .course-menu-toggle')) {
				courseMenuOpen = false;
				removeConfirm = false;
			}
			if (!coverMenuId) return;
			if (target instanceof Element && target.closest('.lv-course-card__menu, .lv-course-card__menu-toggle, .course-card-menu, .course-card-menu-toggle')) return;
			coverMenuId = null;
		};
		const closeMenusOnEscape = (event: KeyboardEvent) => {
			if (event.key !== 'Escape') return;
			coverMenuId = null;
			courseMenuOpen = false;
			removeConfirm = false;
			focusMode = false;
		};
		window.addEventListener('hashchange', change);
		window.addEventListener('pointerdown', closeMenus);
		window.addEventListener('keydown', closeMenusOnEscape);
		void initialize();
		return () => {
			window.removeEventListener('hashchange', change);
			window.removeEventListener('pointerdown', closeMenus);
			window.removeEventListener('keydown', closeMenusOnEscape);
			scheme?.removeEventListener('change', updateSystemTheme);
		};
	});

	function safeLocalStorage(): Storage | null {
		try {
			return window.localStorage;
		} catch {
			return null;
		}
	}
	const RAIL_COMPACT_KEY = 'learnvault-rail-compact';
	function readRailCompact(): boolean {
		try {
			return safeLocalStorage()?.getItem(RAIL_COMPACT_KEY) === 'true';
		} catch {
			return false;
		}
	}
	function setRailCompact(next: boolean) {
		railCompact = next;
		try {
			safeLocalStorage()?.setItem(RAIL_COMPACT_KEY, String(next));
		} catch {
			// A preferência é local e não pode impedir a navegação.
		}
	}
	function toggleRailCompact() {
		setRailCompact(!railCompact);
	}
	function loadPlayerPreferences(currentUser: User | null) {
		const scope = playerPreferencesScope(transportKind, currentUser?.userId);
		playerPreferences = readPlayerPreferences(safeLocalStorage(), scope);
	}
	function savePlayerPreferences(next: PlayerPreferences): boolean {
		playerPreferences = next;
		const scope = playerPreferencesScope(transportKind, user?.userId);
		const saved = writePlayerPreferences(safeLocalStorage(), scope, next);
		return saved;
	}
	function autoAdvance() {
		if (!playerPreferences.autoNext || !neighbors.next) return;
		location.hash = href('course', courseId ?? '', neighbors.next.path);
	}
	const LIBRARY_VIEW_LABELS: Record<LibraryView, string> = {
		covers: 'Capas',
		list: 'Lista',
		compact: 'Compacto',
		shelf: 'Estante'
	};
	const LIBRARY_VIEW_DESCRIPTIONS: Record<LibraryView, string> = {
		covers: 'Capas: destaque para a imagem de cada curso.',
		list: 'Lista: cursos em linhas com informações detalhadas.',
		compact: 'Compacto: títulos e contagens em linhas curtas.',
		shelf: 'Estante: cursos organizados como lombadas.'
	};
	function changeLibraryView(view: LibraryView) {
		libraryView = view;
		persistLibraryView(safeLocalStorage(), view);
	}
	function changeTheme(mode: ThemeMode) {
		themeMode = mode;
		resolvedTheme = resolveTheme(mode, systemDark);
		applyTheme(document.documentElement, resolvedTheme);
		const themeColor = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
		if (themeColor) themeColor.content = resolvedTheme === 'dark' ? '#151b1d' : '#eceeea';
		persistThemeMode(safeLocalStorage(), mode);
	}
	async function initialize() {
		try {
			const session = await resolveSession(() => transport.me(), transportKind);
			user = session.user;
			loadPlayerPreferences(session.user);
			error = session.error;
			if (user) {
				catalog = await transport.catalog();
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
			loadPlayerPreferences(user);
			password = '';
			catalog = await transport.catalog();
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
		playerPreferences = createDefaultPlayerPreferences();
		catalog = null;
		recent = [];
		username = '';
		password = '';
		currentPassword = '';
		location.hash = href('login');
	}

	function syncCatalog(nextCatalog: Catalog) {
		catalog = nextCatalog;
		mediaError = '';
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
			removeConfirm = false;
			courseMenuOpen = false;
			location.hash = href('home');
			syncCatalog(await transport.catalog());
		} catch (cause) {
			removeError = cause instanceof Error ? cause.message : 'Não foi possível remover o curso.';
		} finally {
			removing = false;
		}
	}

	async function loadRecent() {
		if (!studySyncSupported) return;
		recentLoading = true;
		try {
			// O histórico é do usuário inteiro: um limite maior permite achar a última aula
			// assistida de qualquer curso ao abrir a tela de estudo. A home mostra só os
			// itens mais recentes.
			const result = await transport.recentStudy(100);
			recent = result.items;
		} catch (cause) {
			recent = [];
			if (cause && typeof cause === 'object' && (cause as { code?: string }).code === STUDY_NOT_AVAILABLE) {
				studyEnabled = false;
			}
		} finally {
			recentLoading = false;
		}
	}

	function mediaFailed() {
		mediaError = 'Não foi possível carregar esta mídia.';
	}

	// A capa é escolhida no card do curso. No desktop o seletor de imagens roda no Rust
	// (nenhum arquivo cruza o IPC); no servidor o arquivo escolhido no navegador é enviado
	// ao endpoint administrativo. Em ambos os casos o shell recarrega o catálogo, porque a
	// versão da capa (`?v=`) é o que invalida a imagem já exibida.
	function toggleCoverMenu(courseItem: CatalogNode, open: boolean) {
		coverMenuId = open && canManage ? courseIdOf(courseItem) : null;
	}

	function requestCourseCover(courseItem: CatalogNode) {
		if (coverBusy) return;
		coverMenuId = null;
		coverTargetId = courseIdOf(courseItem);
		coverError = '';
		coverMessage = '';
		if (transportKind === 'desktop') {
			void submitCover(null);
			return;
		}
		coverInput?.click();
	}

	async function submitCover(file: File | null) {
		const target = coverTargetId;
		if (!target || coverBusy) return;
		coverBusy = true;
		coverError = '';
		coverMessage = '';
		try {
			await transport.importCourseCover(target, file);
			syncCatalog(await transport.catalog());
			coverMessage = 'Capa atualizada.';
		} catch (cause) {
			coverError = cause instanceof Error ? cause.message : 'Não foi possível salvar a capa do curso.';
		} finally {
			coverBusy = false;
			coverTargetId = null;
		}
	}

	function pickCover(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0] ?? null;
		// O valor é limpo para que escolher o mesmo arquivo de novo volte a disparar o evento.
		input.value = '';
		if (file && file.size > MAX_COVER_BYTES) {
			coverMessage = '';
			coverError = 'A imagem é maior que 4 MiB.';
			coverTargetId = null;
			return;
		}
		void submitCover(file);
	}

	async function removeCover(courseItem: CatalogNode) {
		if (coverBusy) return;
		coverMenuId = null;
		coverBusy = true;
		coverError = '';
		coverMessage = '';
		try {
			const removed = await transport.removeCourseCover(courseIdOf(courseItem));
			syncCatalog(await transport.catalog());
			coverMessage = removed ? 'Capa removida.' : 'Nenhuma capa personalizada para remover.';
		} catch (cause) {
			coverError = cause instanceof Error ? cause.message : 'Não foi possível remover a capa do curso.';
		} finally {
			coverBusy = false;
		}
	}

	async function resetOrder() {
		const current = course;
		if (!current || orderBusy) return;
		orderBusy = true;
		orderError = '';
		orderMessage = '';
		try {
			await transport.resetCourseOrder(courseIdOf(current));
			syncCatalog(await transport.catalog());
			orderMessage = 'Ordem automática restaurada.';
		} catch (cause) {
			orderError = cause instanceof Error ? cause.message : 'Não foi possível restaurar a ordem.';
		} finally {
			orderBusy = false;
		}
	}

	// O botão manual força a checagem; a abertura da tela usa a checagem não forçada. Uma
	// falha só vira mensagem após a ação do usuário — abrir Configurações offline não
	// incomoda. `updateManual` guarda qual foi a última tentativa.
	async function checkUpdate(manual: boolean) {
		if (updateChecking) return;
		updateChecking = true;
		updateManual = manual;
		try {
			updateStatus = await transport.checkUpdate(manual);
		} catch (cause) {
			if (manual) {
				updateStatus = {
					current: updateStatus?.current ?? '',
					enabled: true,
					available: false,
					latest: null,
					releasedAt: null,
					url: null,
					checkedAt: null,
					error: cause instanceof Error ? cause.message : 'Não foi possível verificar a atualização.'
				};
			}
		} finally {
			updateChecking = false;
		}
	}

</script>
<svelte:head><title>LearnVault — sua biblioteca de estudos</title><meta name="description" content="Sua biblioteca pessoal de cursos." /></svelte:head>
{#if transportKind === 'desktop'}<DesktopTitlebar />{/if}
{#if loading}<div class="lv-loading" aria-live="polite">Carregando seus cursos…</div>
{:else if !user}
	<main class="login-page" aria-labelledby="login-heading">
		<div class="brand"><Brand /> LEARNVAULT</div>
		<section>
			<h1 id="login-heading">Entrar no LearnVault</h1>
			<p class="intro">Acesse sua biblioteca de cursos.</p>
			<form onsubmit={login}>
				<fieldset class="auth-fields" disabled={authBusy}>
					<label class="form-field" for="login-username">Usuário<input id="login-username" name="username" autocomplete="username" autocapitalize="none" spellcheck={false} bind:value={username} required /></label>
					<PasswordField id="login-password" label="Senha" autocomplete="current-password" bind:value={password} />
					{#if error}<p class="error" role="alert">{error}</p>{/if}
					<button type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Entrando…' : 'Entrar'}</button>
				</fieldset>
			</form>
		</section>
	</main>
{:else}
	<a class="lv-skip" href="#main-content">Ir para o conteúdo</a>
	<div class="lv-shell" class:is-focus={focusMode} class:is-compact={railCompact} class:is-desktop={transportKind === 'desktop'}>
		<aside class="lv-rail" aria-label="Navegação principal">
			<a class="lv-rail__brand" href={href('home')} aria-label="LearnVault, início">
				<Brand />
				<span class="lv-rail__wordmark">LearnVault</span>
			</a>
			<nav class="lv-rail__nav" aria-label="Seções">
				<a class="lv-rail__link" class:is-active={route === 'home'} aria-current={route === 'home' ? 'page' : undefined} href={href('home')} title="Biblioteca">
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5.5h6.5V19H4zM13.5 5.5H20V19h-6.5z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/></svg>
					<span>Biblioteca</span>
				</a>
				<a class="lv-rail__link" class:is-active={route === 'statistics'} aria-current={route === 'statistics' ? 'page' : undefined} href={href('statistics')} title="Estatísticas">
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 19.5h16M7 19.5v-6M12 19.5V8M17 19.5V5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>
					<span>Estatísticas</span>
				</a>
				<a class="lv-rail__link" class:is-active={route === 'focus'} aria-current={route === 'focus' ? 'page' : undefined} href={href('focus')} title="Foco">
					<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="13" r="7.5" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M12 9v4.5l3 1.8M9.5 2.5h5M12 2.5v3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>
					<span>Foco</span>
				</a>
				{#if course && isCourseRoute}
					<a class="lv-rail__link" class:is-active={isCourseRoute} aria-current={isCourseRoute ? 'page' : undefined} href={href('course', courseIdOf(course))} title={course.title}>
						<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 4.5h11.5A2.5 2.5 0 0 1 19 7v12.5H7.5A2.5 2.5 0 0 1 5 17z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/><path d="M8 8h7M8 11.5h7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
						<span>Curso</span>
					</a>
				{/if}
				<a class="lv-rail__link" class:is-active={route === 'settings'} aria-current={route === 'settings' ? 'page' : undefined} href={href('settings')} title="Ajustes">
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><circle cx="9" cy="7" r="2.2" fill="var(--rail-bg)" stroke="currentColor" stroke-width="1.6"/><circle cx="15" cy="12" r="2.2" fill="var(--rail-bg)" stroke="currentColor" stroke-width="1.6"/><circle cx="9" cy="17" r="2.2" fill="var(--rail-bg)" stroke="currentColor" stroke-width="1.6"/></svg>
					<span>Ajustes</span>
				</a>
			</nav>
			<div class="lv-rail__foot">
				<button class="lv-rail__compact" type="button" aria-pressed={railCompact} aria-label={railCompact ? 'Expandir barra lateral' : 'Compactar barra lateral'} title={railCompact ? 'Expandir barra lateral' : 'Compactar barra lateral'} onclick={toggleRailCompact}>
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 6l-4 6 4 6M9 6l-4 6 4 6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>
				</button>
				<div class="lv-rail__user" title={user.username}>
					<span class="lv-avatar" aria-hidden="true">{userInitials}</span>
					<span class="lv-rail__username">{user.username}</span>
				</div>
				{#if transportKind === 'http'}
					<button class="lv-rail__logout" type="button" onclick={logout} aria-label="Sair" title="Sair">Sair</button>
				{/if}
			</div>
		</aside>
		<main id="main-content" class="lv-main">
			{#if error}<p class="lv-alert" role="alert">{error}</p>{/if}
			{#if route === 'home'}
				<div class="lv-page">
					<header class="lv-page__head">
						<div>
							<h1>Minha biblioteca</h1>
							<p class="lv-lede">Cursos que você já tem, reunidos em um só lugar. Nada é movido ou alterado.</p>
						</div>
						<div class="lv-page__actions">
							{#if canManage}
								<div class="lv-page__buttons">
									{#if transportKind === 'desktop'}
										<button class="lv-btn lv-btn--primary lvui-btn lvui-btn-primary" onclick={importCourse} disabled={importing} aria-busy={importing}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4v10m-4-4 4 4 4-4M5 17v3h14v-3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>{importing ? 'Importando…' : 'Importar curso'}</button>
									{:else}
										<button class="lv-btn lv-btn--primary lvui-btn lvui-btn-primary" onclick={() => (importFormOpen = !importFormOpen)} aria-expanded={importFormOpen}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4v10m-4-4 4 4 4-4M5 17v3h14v-3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>Importar curso</button>
									{/if}
									<button class="lv-btn lvui-btn" onclick={rescan} disabled={rescanning || !catalog || !catalog.nodes.length}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7v4h-4M4 17v-4h4M5.5 9a7 7 0 0 1 12-2L20 11M4 13l2.5 4a7 7 0 0 0 12-2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>{rescanning ? 'Atualizando…' : 'Atualizar cursos'}</button>
								</div>
							{/if}
						</div>
					</header>

					{#if studySyncSupported && libraryRecent.length}
						<section class="lv-section" aria-labelledby="recent-heading">
							<div class="lv-section__head">
								<h2 id="recent-heading">Continue estudando</h2>
								<span class="lv-count">{libraryRecent.length} {libraryRecent.length === 1 ? 'recente' : 'recentes'}</span>
							</div>
							<ul class="lv-resume">
								{#each libraryRecent as entry (entry.item.mediaId)}
									{@const courseNode = findCourse(catalog, entry.location.courseId)}
									{#if courseNode}
										<li>
											<a class="lv-resume-card" href={href('course', entry.location.courseId, entry.location.lesson.path)}>
												<span class="lv-resume-card__cover">
													<CourseCover course={courseNode} profile={user.userId} mediaUrl={(file, courseId) => transport.mediaUrl(file, courseId)} manualCoverUrl={transport.coverUrl(courseIdOf(courseNode), courseNode.coverVersion ?? null)} />
												</span>
												<span class="lv-resume-card__body">
													<span class="lv-resume-card__course">{entry.location.courseTitle}</span>
													<span class="lv-resume-card__lesson">{entry.location.lesson.title}</span>
													<span class="lv-resume-card__meta">{entry.item.completed ? 'Concluído' : `Parou em ${formatDuration(entry.item.positionSeconds)}`}</span>
												</span>
											</a>
										</li>
									{/if}
								{/each}
							</ul>
						</section>
					{/if}

					{#if canManage && importFormOpen}
						<form class="lv-import" onsubmit={submitImport}>
							<label for="import-path">Caminho da pasta do curso no servidor</label>
							<input id="import-path" name="path" bind:value={importPath} placeholder="Ex.: C:\cursos\Meu curso" autocomplete="off" spellcheck={false} required />
							<div class="lv-import__actions">
								<button class="lv-btn lv-btn--primary" disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar'}</button>
								<button type="button" class="lv-btn lv-btn--ghost" onclick={() => (importFormOpen = false)}>Cancelar</button>
							</div>
						</form>
					{/if}
					{#if importError}<p class="lv-alert" role="alert">{importError}</p>{/if}
					{#if user.role === 'admin' && rescanMessage}<p class="lv-notice" aria-live="polite">{rescanMessage}</p>{/if}
					{#if canManage}<input class="visually-hidden course-cover-input" type="file" accept={COVER_ACCEPT} bind:this={coverInput} onchange={pickCover} tabindex="-1" aria-hidden="true" />{/if}
					{#if coverError}<p class="lv-alert" role="alert">{coverError}</p>{/if}
					{#if coverMessage}<p class="lv-notice" role="status" aria-live="polite">{coverMessage}</p>{/if}

					<section class="lv-section" aria-labelledby="courses-heading">
						<div class="lv-section__head lv-section__head--courses">
							<h2 id="courses-heading">Todos os cursos</h2>
							{#if catalog && catalog.nodes.length}
								<form class="lv-search" role="search" onsubmit={(event) => event.preventDefault()}>
									<label class="visually-hidden" for="catalog-search-input">Buscar nos cursos</label>
									<div class="search-field">
										<svg class="search-glyph" viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7" fill="none" stroke="currentColor" stroke-width="2"></circle><path d="m16.6 16.6 4.2 4.2" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"></path></svg>
										<input class="lvui-input" id="catalog-search-input" name="q" type="search" placeholder="Buscar curso ou aula…" autocomplete="off" bind:value={catalogQuery} />
									</div>
								</form>
							{/if}
							<div class="lv-section__tools">
								{#if catalog && catalog.nodes.length}<span class="lv-count" aria-live="polite">{filteredNodes.length} de {catalog.nodes.length}</span>{/if}
								<div class="lv-view-switch lvui-join" role="group" aria-label="Visualização dos cursos" aria-describedby="view-hint">
									{#each LIBRARY_VIEWS as view}
										<button type="button" class="lvui-join-item" class:is-active={libraryView === view} aria-pressed={libraryView === view} onclick={() => changeLibraryView(view)}>{LIBRARY_VIEW_LABELS[view]}</button>
									{/each}
								</div>
							</div>
						</div>
						<p id="view-hint" class="lv-view-hint">{LIBRARY_VIEW_DESCRIPTIONS[libraryView]}</p>
						{#if catalog && catalog.nodes.length}
							{#if filteredNodes.length}
								<ul class="lv-course-grid" data-view={libraryView} aria-label="Cursos na visualização {LIBRARY_VIEW_LABELS[libraryView]}">
									{#each filteredNodes as courseItem (courseItem.courseId ?? courseItem.path)}
										<li class:is-menu-open={coverMenuId === courseIdOf(courseItem)}>
											<CourseCard course={courseItem} profile={user.userId} mediaUrl={(file, courseId) => transport.mediaUrl(file, courseId)} manualCoverUrl={transport.coverUrl(courseIdOf(courseItem), courseItem.coverVersion ?? null)} {canManage} menuOpen={coverMenuId === courseIdOf(courseItem)} onMenuOpen={(open) => toggleCoverMenu(courseItem, open)} onChangeCover={requestCourseCover} onRemoveCover={removeCover} />
										</li>
									{/each}
								</ul>
							{:else}
								<div class="lv-empty-state">
									<p class="lv-empty__title" role="status">Nenhum curso corresponde a “{catalogQuery.trim()}”.</p>
									<button class="lv-btn" type="button" onclick={() => (catalogQuery = '')}>Limpar busca</button>
								</div>
							{/if}
						{:else if canManage}
							<div class="lv-empty-state">
								<p class="lv-empty__title">Sua biblioteca começa aqui</p>
								<p class="lv-lede">Importe a pasta de um curso. Os arquivos permanecem somente leitura.</p>
								{#if transportKind === 'desktop'}<button class="lv-btn lv-btn--primary" onclick={importCourse} disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar curso'}</button>{:else}<button class="lv-btn lv-btn--primary" onclick={() => (importFormOpen = true)}>Importar curso</button>{/if}
							</div>
						{:else}
							<div class="lv-empty-state"><p class="lv-empty__title">Nenhum curso disponível</p><p class="lv-lede">Peça ao administrador para importar um curso.</p></div>
						{/if}
					</section>

					{#if catalog && catalog.looseFiles.length}<div class="lv-section"><LooseFiles files={catalog.looseFiles} heading="Arquivos soltos" /></div>{/if}
				</div>
			{:else if route === 'focus'}
				<Pomodoro />
			{:else if route === 'statistics'}
				<Statistics {catalog} {recent} />
			{:else if route === 'settings'}
				<div class="lv-page lv-page--narrow lv-settings">
					<header class="lv-page__head">
						<div>
							<p class="lv-kicker">Preferências</p>
							<h1>Ajustes</h1>
							<p class="lv-lede">Escolha como o LearnVault aparece neste dispositivo.</p>
						</div>
					</header>
					<section class="lv-panel lv-settings__panel">
						<div class="lv-panel__head"><h2>Tema</h2><span class="lv-count">Tema atual: {resolvedTheme === 'dark' ? 'escuro' : 'claro'}{themeMode === 'system' ? ' (sistema)' : ''}</span></div>
						<fieldset class="theme-options">
							<legend class="visually-hidden">Tema</legend>
							<label><input type="radio" name="theme" value="system" checked={themeMode === 'system'} onchange={() => changeTheme('system')} />Sistema <span>Acompanha a preferência do sistema.</span></label>
							<label><input type="radio" name="theme" value="light" checked={themeMode === 'light'} onchange={() => changeTheme('light')} />Claro <span>Usa sempre o tema claro.</span></label>
							<label><input type="radio" name="theme" value="dark" checked={themeMode === 'dark'} onchange={() => changeTheme('dark')} />Escuro <span>Usa sempre o tema escuro.</span></label>
						</fieldset>
						<label class="lv-compact-setting">
							<input type="checkbox" checked={railCompact} onchange={(event) => setRailCompact(event.currentTarget.checked)} />
							<span>Barra lateral compacta</span>
							<small>Mostra apenas os ícones da navegação.</small>
						</label>
					</section>
					<section class="lv-panel lv-settings__panel" aria-labelledby="account-heading">
						<div class="lv-panel__head"><h2 id="account-heading">Conta</h2><span class="lv-count">{user.isAdmin ? 'Administrador' : 'Estudante'}</span></div>
						<div class="lv-settings__body">
							<p><strong>{user.username}</strong></p>
							<form class="credentials-form" onsubmit={credentials}>
								<fieldset class="auth-fields" disabled={authBusy}>
									<legend>Credenciais de acesso</legend>
									<PasswordField id="settings-current-password" label="Senha atual" autocomplete="current-password" bind:value={currentPassword} />
									<label class="form-field" for="settings-new-username">Novo usuário<input id="settings-new-username" name="username" autocomplete="username" autocapitalize="none" spellcheck={false} bind:value={username} required /></label>
									<PasswordField id="settings-new-password" label="Nova senha" autocomplete="new-password" bind:value={password} minLength={12} hint="Use ao menos 12 caracteres." />
									{#if accountError}<p class="error" role="alert">{accountError}</p>{/if}
									{#if accountStatus}<p class="lv-notice" role="status">{accountStatus}</p>{/if}
									<button class="lv-btn lv-btn--primary" type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Salvando…' : 'Atualizar credenciais'}</button>
								</fieldset>
							</form>
							{#if transportKind === 'http'}<button class="lv-btn lv-btn--ghost" type="button" onclick={logout}>Encerrar sessão</button>{:else}<p class="lv-notice">Biblioteca local neste dispositivo.</p>{/if}
						</div>
					</section>
					{#if transportKind === 'http'}
						<section class="lv-panel lv-settings__panel" aria-labelledby="update-heading">
							<div class="lv-panel__head"><h2 id="update-heading">Atualização</h2>{#if updateStatus}<span class="lv-count">Versão {updateStatus.current}</span>{/if}</div>
							<div class="lv-settings__body">
								<button class="lv-btn" type="button" onclick={() => checkUpdate(true)} disabled={updateChecking} aria-busy={updateChecking}>{updateChecking ? 'Verificando…' : 'Verificar atualização'}</button>
								{#if updateStatus}
									{@const outcome = updateOutcome(updateStatus)}
									{@const releaseUrl = updateReleaseUrl(updateStatus)}
									{#if outcome !== 'error' || updateManual}<p class="lv-notice" role="status">{updateMessage(updateStatus)}</p>{/if}
									{#if outcome === 'available' && releaseUrl}<p class="lv-notice"><a href={releaseUrl} target="_blank" rel="noopener noreferrer">Ver a release</a></p>{/if}
								{/if}
							</div>
						</section>
					{/if}
				</div>
			{:else if isCourseRoute}
				{#if !course}
					<div class="lv-page lv-page--narrow">
						<nav class="lv-breadcrumb" aria-label="Caminho"><a href={href('home')}>Biblioteca</a><span aria-hidden="true">/</span><span>Curso</span></nav>
						<div class="lv-empty-state"><p class="lv-empty__title">Curso não encontrado</p><p class="lv-lede">Ele pode ter sido removido do catálogo atual.</p><a class="lv-btn" href={href('home')}>Voltar à biblioteca</a></div>
					</div>
				{:else if isOverview}
					{@const counts = countCatalogNode(course)}
					{@const meta = formatCourseMeta(counts)}
					{@const cover = courseCover(course.title)}
					{@const materials = collectMaterials(course)}
					<div class="lv-page">
						<nav class="lv-breadcrumb" aria-label="Caminho do curso"><a href={href('home')}>Biblioteca</a><span aria-hidden="true">/</span><span>{course.title}</span></nav>
						<header class="lv-course-hero">
							<div class="lv-course-hero__cover">
								<CourseCover course={course} profile={user.userId} mediaUrl={(file, courseId) => transport.mediaUrl(file, courseId)} manualCoverUrl={transport.coverUrl(courseIdOf(course), course.coverVersion ?? null)} size="hero" />
							</div>
							<div class="lv-course-hero__main">
								<p class="lv-kicker">Curso · {cover.code}</p>
								<h1>{course.title}</h1>
								{#if meta}<p class="lv-course-hero__meta">{meta}</p>{/if}
								<div class="lv-course-hero__actions">
									{#if activePath}
										<a class="lv-btn lv-btn--primary" href={href('course', courseIdOf(course), activePath)}>{courseRecent ? 'Continuar estudando' : 'Começar curso'}</a>
									{:else}
										<span class="lv-btn" aria-disabled="true">Nenhuma aula disponível</span>
									{/if}
									{#if canManage}
										<div class="course-menu-wrap">
											<button class="course-menu-toggle lv-btn" type="button" aria-expanded={courseMenuOpen} onclick={() => { courseMenuOpen = !courseMenuOpen; removeConfirm = false; }}>Gerenciar curso <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg></button>
											{#if courseMenuOpen}
												<div class="course-menu" role="group" aria-label="Gerenciar {course.title}">
													{#if removeConfirm}
														<p class="course-menu-question" role="alert">Remover “{course.title}” da biblioteca?</p>
														<p class="course-menu-hint">Os arquivos não serão apagados.</p>
														<div class="course-menu-actions">
															<button class="text-button danger" type="button" onclick={removeCourse} disabled={removing} aria-busy={removing}>{removing ? 'Removendo…' : 'Remover curso'}</button>
															<button class="text-button" type="button" onclick={() => (removeConfirm = false)} disabled={removing}>Cancelar</button>
														</div>
													{:else}
														<button class="text-button" type="button" onclick={resetOrder} disabled={orderBusy} aria-busy={orderBusy}>{orderBusy ? 'Restaurando…' : 'Restaurar ordem'}</button>
														<button class="text-button danger" type="button" onclick={() => (removeConfirm = true)}>Remover curso</button>
													{/if}
												</div>
											{/if}
										</div>
									{/if}
								</div>
								{#if courseRecent}
									<p class="lv-course-hero__resume">Última aula: {courseRecent.location.lesson.title} · {courseRecent.item.completed ? 'concluída' : `parou em ${formatDuration(courseRecent.item.positionSeconds)}`}</p>
								{/if}
							</div>
						</header>
						{#if removeError}<p class="lv-alert" role="alert">{removeError}</p>{/if}
						{#if orderError}<p class="lv-alert" role="alert">{orderError}</p>{/if}
						{#if orderMessage}<p class="lv-notice" role="status" aria-live="polite">{orderMessage}</p>{/if}
						<div class="lv-course-layout">
							<section class="lv-panel" aria-labelledby="course-content-heading">
								<div class="lv-panel__head"><h2 id="course-content-heading">Conteúdo do curso</h2><span class="lv-count">{formatLessonCounts(counts)}</span></div>
								<div class="lv-tabs" role="tablist" aria-label="Conteúdo do curso">
									<button class="lv-tab" type="button" role="tab" id="course-tab-content" aria-controls="course-panel-content" aria-selected={courseTab === 'content'} tabindex={courseTab === 'content' ? 0 : -1} class:is-active={courseTab === 'content'} onclick={() => (courseTab = 'content')} onkeydown={courseTabKey}>Conteúdo</button>
									<button class="lv-tab" type="button" role="tab" id="course-tab-materials" aria-controls="course-panel-materials" aria-selected={courseTab === 'materials'} tabindex={courseTab === 'materials' ? 0 : -1} class:is-active={courseTab === 'materials'} onclick={() => (courseTab = 'materials')} onkeydown={courseTabKey}>Materiais {#if materials.length}<span class="lv-badge">{materials.length}</span>{/if}</button>
								</div>
								{#if courseTab === 'content'}
									<div id="course-panel-content" role="tabpanel" aria-labelledby="course-tab-content">
										{#if course.children.length}
											<CatalogTree nodes={course.children} courseId={courseIdOf(course)} activePath={activePath} showFiles={false} />
										{:else}
											<p class="lv-empty">Este curso ainda não foi indexado.</p>
										{/if}
									</div>
								{:else}
									<div id="course-panel-materials" role="tabpanel" aria-labelledby="course-tab-materials">
										{#if materials.length}
											<LooseFiles files={materials} heading="Materiais do curso" courseId={courseIdOf(course)} />
										{:else}
											<p class="lv-empty">Nenhum material de apoio neste curso.</p>
										{/if}
									</div>
								{/if}
							</section>
							<aside class="lv-course-side">
								<section class="lv-panel lv-meta" aria-labelledby="course-meta-heading">
									<div class="lv-panel__head"><h2 id="course-meta-heading">Ficha do curso</h2></div>
									<dl>
										<div><dt>Módulos</dt><dd>{counts.modules}</dd></div>
										<div><dt>Aulas</dt><dd>{counts.lessons}{#if counts.missingLessons > 0}<span class="lv-meta__missing"> · {counts.missingLessons} ausentes</span>{/if}</dd></div>
										<div><dt>Vídeos</dt><dd>{counts.videos}</dd></div>
										<div><dt>Materiais</dt><dd>{counts.materials}</dd></div>
										<div><dt>Código</dt><dd class="lv-meta__code">{cover.code}</dd></div>
									</dl>
								</section>
								<section class="lv-panel lv-resume-panel" aria-labelledby="course-resume-heading">
									<div class="lv-panel__head"><h2 id="course-resume-heading">Continue de onde parou</h2></div>
									<div class="lv-resume-panel__body">
										{#if courseRecent}
											<p class="lv-resume-panel__lesson">{courseRecent.location.lesson.title}</p>
											<p class="lv-resume-panel__meta">{formatLocationContext(courseRecent.location.courseTitle, courseRecent.location.moduleTitle)}{#if !courseRecent.item.completed}{' · '}{formatDuration(courseRecent.item.positionSeconds)}{/if}</p>
											<a class="lv-btn lv-btn--primary" href={href('course', courseIdOf(course), courseRecent.location.lesson.path)}>{courseRecent.item.completed ? 'Revisar aula' : 'Retomar aula'}</a>
										{:else}
											<p class="lv-empty">Você ainda não começou este curso.</p>
											{#if activePath}<a class="lv-btn" href={href('course', courseIdOf(course), activePath)}>Começar curso</a>{/if}
										{/if}
									</div>
								</section>
							</aside>
						</div>
					</div>
				{:else}
					<div class="lv-page lv-player-page">
						<div class="lv-player-top">
							<nav class="lv-breadcrumb" aria-label="Caminho do curso"><a href={href('home')}>Biblioteca</a><span aria-hidden="true">/</span><a href={href('course', courseIdOf(course))}>{course.title}</a><span aria-hidden="true">/</span><span>{lesson?.title ?? 'Aula'}</span></nav>
							<a class="lv-btn lv-btn--ghost" href={href('course', courseIdOf(course))}>Visão geral do curso</a>
						</div>
						{#if removeError}<p class="lv-alert" role="alert">{removeError}</p>{/if}
						{#if orderError}<p class="lv-alert" role="alert">{orderError}</p>{/if}
						{#if orderMessage}<p class="lv-notice" role="status" aria-live="polite">{orderMessage}</p>{/if}
						<div class="lv-player-layout">
							<div class="lv-player-main">
								{#if lesson}
									<div class="lv-lesson-head">
										<div>
											<p class="lv-kicker">{activeContainer?.title ?? course.title}</p>
											<h1>{lesson.title}</h1>
										</div>
										<div class="lv-lesson-actions">
											{#if neighbors.previous}<a class="lv-btn" href={href('course', courseIdOf(course), neighbors.previous.path)}>Anterior</a>{/if}
											{#if neighbors.next}<a class="lv-btn" href={href('course', courseIdOf(course), neighbors.next.path)}>Próxima</a>{/if}
											<button class="lv-btn" type="button" aria-pressed={focusMode} onclick={() => (focusMode = !focusMode)}>{focusMode ? 'Sair do foco' : 'Modo foco'}</button>
										</div>
									</div>
									<Player {lesson} courseId={courseIdOf(course)} lessonTitle={lesson.title} {mediaError} onMediaError={mediaFailed} {studyEnabled} preferences={playerPreferences}
										onPreferencesChange={savePlayerPreferences} autoNext={playerPreferences.autoNext}
										hasNextLesson={neighbors.next !== null} onAutoNext={autoAdvance}
										onAutoNextChange={(enabled) => savePlayerPreferences({ ...playerPreferences, autoNext: enabled })} />
									{#if activeContainer && (activeContainer.files.length || activeContainer.looseFiles.length)}<div class="lv-player-materials"><LooseFiles files={[...activeContainer.files, ...activeContainer.looseFiles]} heading="Materiais do módulo" courseId={courseIdOf(course)} /></div>{/if}
									{#if course.looseFiles.length}<div class="lv-player-materials"><LooseFiles files={course.looseFiles} heading="Arquivos soltos do curso" courseId={courseIdOf(course)} /></div>{/if}
								{:else}
									<div class="lv-empty-state"><p class="lv-empty__title">Este curso não tem nenhuma aula disponível</p><a class="lv-btn" href={href('course', courseIdOf(course))}>Voltar à visão geral</a></div>
								{/if}
							</div>
							<aside class="lv-curriculum" aria-label="Aulas do curso">
								<div class="lv-panel__head"><h2>Aulas do curso</h2><span class="lv-count">{formatLessonCounts(countCatalogNode(course))}</span></div>
								<div class="lv-curriculum__body" data-playlist-body>
									{#if course.children.length}<CatalogTree nodes={course.children} courseId={courseIdOf(course)} activePath={activePath} showFiles={false} />{:else}<p class="lv-empty">Este curso ainda não foi indexado.</p>{/if}
								</div>
							</aside>
						</div>
					</div>
				{/if}
			{:else}
				<div class="lv-page lv-page--narrow">
					{#if !lesson}<div class="lv-empty-state"><p class="lv-empty__title">Aula não encontrada</p><a class="lv-btn" href={href('home')}>Voltar à biblioteca</a></div>{:else}
						<a class="lv-back" href={href('home')}>← Biblioteca</a>
						<h1 class="lv-standalone-title">{lesson.title}</h1>
						<Player {lesson} courseId={courseId ?? ''} {mediaError} onMediaError={mediaFailed} {studyEnabled} preferences={playerPreferences}
							onPreferencesChange={savePlayerPreferences} autoNext={playerPreferences.autoNext}
							hasNextLesson={neighbors.next !== null} onAutoNext={autoAdvance}
							onAutoNextChange={(enabled) => savePlayerPreferences({ ...playerPreferences, autoNext: enabled })} />
						{#if neighbors.previous || neighbors.next}<nav class="lv-lesson-nav" aria-label="Aulas vizinhas">{#if neighbors.previous}<a href={href('player', courseId ?? '', neighbors.previous.path)}>← {neighbors.previous.title}</a>{/if}{#if neighbors.next}<a href={href('player', courseId ?? '', neighbors.next.path)}>{neighbors.next.title} →</a>{/if}</nav>{/if}
					{/if}
				</div>
			{/if}
		</main>
		<MiniPomodoro hiddenDock={route === 'focus' || focusMode} courseTitle={isCourseRoute ? course?.title ?? null : null} lessonTitle={isPlayer ? lesson?.title ?? null : null} syncScope={transportKind === 'desktop' ? 'desktop:local' : user ? `http:${user.userId}` : null} />
	</div>
{/if}
