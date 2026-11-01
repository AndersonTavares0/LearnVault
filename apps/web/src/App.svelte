<script lang="ts">
	import { onMount } from 'svelte';
	import LooseFiles from './LooseFiles.svelte';
	import CatalogTree from './CatalogTree.svelte';
	import CourseCard from './CourseCard.svelte';
	import CourseCover from './CourseCover.svelte';
	import Player from './Player.svelte';
	import Statistics from './Statistics.svelte';
	import AllNotes from './AllNotes.svelte';
	import UserProfile from './UserProfile.svelte';
	import PomodoroSettings from './PomodoroSettings.svelte';
	import FocusMenu from './FocusMenu.svelte';
	import DesktopTitlebar from './DesktopTitlebar.svelte';
	import { filterCatalog } from './search';
	import { collectMaterials, countCatalogNode, countMissingVideos, formatCount, formatCourseMeta, formatLessonCounts, formatLocationContext } from './catalog';
	import { courseCover } from './cover';
	import PasswordField from './PasswordField.svelte';
	import { defaultLessonPath, firstAvailableLessonPath, lessonContainer, neighboringLessons } from './navigation';
	import { destinationAfterLogin, findCourse, findLesson, findLessonLocation, href, locationFromHash, type LessonLocation, type Route } from './router';
	import { transport, transportKind, COVER_ACCEPT, MAX_COVER_BYTES, courseIdOf, STUDY_NOT_AVAILABLE, type Catalog, type CatalogNode, type Lesson, type StudyProgress, type UpdateStatus, type UpdateHistoryEntry, type User } from './transport';
	import { updateOutcome, updateMessage, updateReleaseUrl } from './update';
	import { defaultRoute, resolveSession } from './session';
	import { formatDuration, studySyncSupported } from './player';
	import { createDefaultPlayerPreferences, playerPreferencesScope, readPlayerPreferences, writePlayerPreferences, type PlayerPreferences } from './player-preferences';
	import Brand from './Brand.svelte';
	import { applyTheme, persistThemeMode, readThemeMode, resolveTheme, type ThemeMode } from './theme';
	import { LIBRARY_VIEWS, persistLibraryView, readLibraryView, type LibraryView } from './library-view';

	let themeMode: ThemeMode = readThemeMode(safeLocalStorage());
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
	let positionSeconds: number | null = initialLocation.positionSeconds ?? null;
	let avatarVersion = Date.now();
	let topbarAvatarFailed = false;

	function handleAvatarChanged() {
		topbarAvatarFailed = false;
		avatarVersion = Date.now();
	}
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
	let importedSuccess: {
		courseId: string;
		title: string;
		firstLessonPath: string | null;
		lessonCount: number;
	} | null = null;
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
			if (themeColor) themeColor.content = resolvedTheme === 'dark' ? '#10151b' : '#eef1f4';
		}
	};
	scheme?.addEventListener('change', updateSystemTheme);

	onMount(() => {
		const change = () => {
			const next = locationFromHash(location.hash, fallbackRoute);
			route = next.route;
			courseId = next.courseId;
			lessonPath = next.lessonPath;
			positionSeconds = next.positionSeconds ?? null;
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

	onMount(() => {
		function onShortcut(event: KeyboardEvent) {
			if (event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey) return;
			const target = event.target as HTMLElement | null;
			if (target?.closest('input, textarea, select, [contenteditable="true"]')) return;
			const input = document.getElementById('catalog-search-input');
			if (!input) return;
			event.preventDefault();
			input.focus();
		}
		document.addEventListener('keydown', onShortcut);
		return () => document.removeEventListener('keydown', onShortcut);
	});

	function safeLocalStorage(): Storage | null {
		try {
			return window.localStorage;
		} catch {
			return null;
		}
	}
	const RAIL_COMPACT_KEY = 'learnvault-rail-compact';
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
		covers: 'Grade: capas e progresso visíveis para escolher rapidamente.',
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
		if (themeColor) themeColor.content = resolvedTheme === 'dark' ? '#10151b' : '#eef1f4';
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
		importedSuccess = null;
		const previousCourseIds = new Set((catalog?.nodes ?? []).map(courseIdOf));
		try {
			await transport.importCourse();
			await transport.rescan();
			const nextCatalog = await transport.catalog();
			syncCatalog(nextCatalog);
			await loadRecent();
			const newlyAdded = nextCatalog.nodes.find((c) => !previousCourseIds.has(courseIdOf(c)));
			if (newlyAdded) {
				const counts = countCatalogNode(newlyAdded);
				importedSuccess = {
					courseId: courseIdOf(newlyAdded),
					title: newlyAdded.title,
					firstLessonPath: firstAvailableLessonPath([newlyAdded]),
					lessonCount: counts.lessons
				};
			}
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
		importedSuccess = null;
		const previousCourseIds = new Set((catalog?.nodes ?? []).map(courseIdOf));
		try {
			await transport.importCoursePath(importPath.trim());
			importPath = '';
			importFormOpen = false;
			await transport.rescan();
			const nextCatalog = await transport.catalog();
			syncCatalog(nextCatalog);
			await loadRecent();
			const newlyAdded = nextCatalog.nodes.find((c) => !previousCourseIds.has(courseIdOf(c)));
			if (newlyAdded) {
				const counts = countCatalogNode(newlyAdded);
				importedSuccess = {
					courseId: courseIdOf(newlyAdded),
					title: newlyAdded.title,
					firstLessonPath: firstAvailableLessonPath([newlyAdded]),
					lessonCount: counts.lessons
				};
			}
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

	function mediaFailed(code?: number) {
		if (code === 4) {
			mediaError = 'Codec ou formato de mídia incompatível.';
		} else if (code === 3) {
			mediaError = 'Falha na decodificação da mídia.';
		} else {
			mediaError = 'Não foi possível carregar esta mídia.';
		}
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
	let updateHistoryList: UpdateHistoryEntry[] = [];

	async function loadUpdateHistory() {
		if (transportKind !== 'http') return;
		try {
			updateHistoryList = await transport.updateHistory();
		} catch {
			updateHistoryList = [];
		}
	}

	async function checkUpdate(manual: boolean) {
		if (updateChecking) return;
		updateChecking = true;
		updateManual = manual;
		try {
			updateStatus = await transport.checkUpdate(manual);
			void loadUpdateHistory();
		} catch (cause) {
			if (manual) {
				updateStatus = {
					current: updateStatus?.current ?? '',
					enabled: true,
					available: false,
					latest: null,
					releasedAt: null,
					url: null,
					changelog: null,
					checkedAt: null,
					error: cause instanceof Error ? cause.message : 'Não foi possível verificar a atualização.'
				};
			}
		} finally {
			updateChecking = false;
		}
	}

	let updateInstalling = false;
	let updateInstallError: string | null = null;

	async function installUpdate() {
		if (updateInstalling || !transport.installUpdate) return;
		updateInstalling = true;
		updateInstallError = null;
		try {
			const installed = await transport.installUpdate();
			if (!installed) {
				updateInstallError = 'Nenhuma atualização disponível para instalação no momento.';
			}
		} catch (cause) {
			updateInstallError = cause instanceof Error ? cause.message : 'Não foi possível instalar a atualização.';
		} finally {
			updateInstalling = false;
		}
	}

</script>
<svelte:head><title>LearnVault — sua biblioteca de estudos</title><meta name="description" content="Sua biblioteca pessoal de cursos." /></svelte:head>
{#if transportKind === 'desktop'}<DesktopTitlebar />{/if}
{#if loading}<div class="lv-loading" aria-live="polite">Carregando seus cursos…</div>
{:else if !user}
	<main class="login-page" aria-labelledby="login-heading">
		<section>
			<div class="login-head"><Brand /><h1 id="login-heading">Entrar no LearnVault</h1></div>
			<p class="intro">Acesse sua biblioteca de cursos.</p>
			<form onsubmit={login}>
				<fieldset class="auth-fields" disabled={authBusy}>
					<label class="form-field" for="login-username">Usuário<input class="lvui-input" id="login-username" name="username" autocomplete="username" autocapitalize="none" spellcheck={false} bind:value={username} required /></label>
					<PasswordField id="login-password" label="Senha" autocomplete="current-password" bind:value={password} />
					{#if error}<p class="error" role="alert">{error}</p>{/if}
					<button class="lvui-btn lvui-btn-primary lvui-btn-block" type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Entrando…' : 'Entrar'}</button>
				</fieldset>
			</form>
		</section>
	</main>
{:else}
	<a class="lv-skip" href="#main-content">Ir para o conteúdo</a>
	<div class="lv-shell" class:is-focus={focusMode} class:is-desktop={transportKind === 'desktop'}>
		<header class="lv-topbar flex flex-wrap items-center gap-5 min-h-14 px-4 lg:px-8">
			<a class="lv-topbar__brand" href={href('home')} aria-label="LearnVault, início">
				<Brand />
				<span>LearnVault</span>
			</a>
			<nav class="lv-topbar__nav" aria-label="Seções">
				<a class="lv-topbar__link" class:is-active={route === 'home'} aria-current={route === 'home' ? 'page' : undefined} href={href('home')}>
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5.5h6.5V19H4zM13.5 5.5H20V19h-6.5z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/></svg>
					<span>Biblioteca</span>
				</a>
				<a class="lv-topbar__link" class:is-active={route === 'statistics'} aria-current={route === 'statistics' ? 'page' : undefined} href={href('statistics')}>
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 19.5h16M7 19.5v-6M12 19.5V8M17 19.5V5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>
					<span>Estatísticas</span>
				</a>
				<a class="lv-topbar__link" class:is-active={route === 'notes'} aria-current={route === 'notes' ? 'page' : undefined} href={href('notes')}>
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M19 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2zM7 7h10M7 12h10M7 17h7" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>
					<span>Anotações</span>
				</a>
				<a class="lv-topbar__link" class:is-active={route === 'settings'} aria-current={route === 'settings' ? 'page' : undefined} href={href('settings')}>
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><circle cx="9" cy="7" r="2.2" fill="var(--rail-bg)" stroke="currentColor" stroke-width="1.6"/><circle cx="15" cy="12" r="2.2" fill="var(--rail-bg)" stroke="currentColor" stroke-width="1.6"/><circle cx="9" cy="17" r="2.2" fill="var(--rail-bg)" stroke="currentColor" stroke-width="1.6"/></svg>
					<span>Ajustes</span>
				</a>
			</nav>
			<div class="lv-topbar__utils">
				<FocusMenu />
				<button class="lv-topbar__icon" type="button" aria-label={resolvedTheme === 'dark' ? 'Mudar para tema claro' : 'Mudar para tema escuro'} title={resolvedTheme === 'dark' ? 'Tema claro' : 'Tema escuro'} onclick={() => changeTheme(resolvedTheme === 'dark' ? 'light' : 'dark')}>
					{#if resolvedTheme === 'dark'}
						<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M18.7 5.3l-1.4 1.4M6.7 17.3l-1.4 1.4" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>
					{:else}
						<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5Z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/></svg>
					{/if}
				</button>
				<div class="lv-topbar__user" title={user.username}>
					{#if !topbarAvatarFailed && transport.userAvatarUrl(avatarVersion)}
						<img class="lv-avatar lv-avatar--img" src={transport.userAvatarUrl(avatarVersion)} alt="Avatar de {user.username}" onerror={() => (topbarAvatarFailed = true)} />
					{:else}
						<span class="lv-avatar" aria-hidden="true">{userInitials}</span>
					{/if}
					<span class="lv-topbar__username">{user.username}</span>
				</div>
				{#if transportKind === 'http'}
					<button class="lv-topbar__logout" type="button" onclick={logout} aria-label="Sair" title="Sair">Sair</button>
				{/if}
			</div>
		</header>
		<main id="main-content" class="lv-main">
			{#if error}<p class="lv-alert" role="alert">{error}</p>{/if}
			{#if route === 'home'}
				<div class="lv-page lv-library">
					<header class="lv-page__head">
						<div>
							<h1>Minha biblioteca</h1>
							<p class="lv-lede">Cursos organizados. Arquivos originais intactos.</p>
						</div>
						<div class="lv-page__actions">
							{#if canManage}
								<div class="lv-page__buttons">
									{#if transportKind === 'desktop'}
										<button class={`lvui-btn${studySyncSupported && libraryRecent.length > 0 && !catalogQuery.trim() ? '' : ' lvui-btn-primary'}`} onclick={importCourse} disabled={importing} aria-busy={importing}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4v10m-4-4 4 4 4-4M5 17v3h14v-3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>{importing ? 'Importando…' : 'Importar curso'}</button>
									{:else}
										<button class={`lvui-btn${studySyncSupported && libraryRecent.length > 0 && !catalogQuery.trim() ? '' : ' lvui-btn-primary'}`} onclick={() => (importFormOpen = !importFormOpen)} aria-expanded={importFormOpen}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4v10m-4-4 4 4 4-4M5 17v3h14v-3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>Importar curso</button>
									{/if}
									<button class="lvui-btn" onclick={rescan} disabled={rescanning || !catalog || !catalog.nodes.length}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7v4h-4M4 17v-4h4M5.5 9a7 7 0 0 1 12-2L20 11M4 13l2.5 4a7 7 0 0 0 12-2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>{rescanning ? 'Atualizando…' : 'Atualizar cursos'}</button>
								</div>
							{/if}
						</div>
					</header>

					{#if catalog && catalog.nodes.length}
						<form class="lv-library-search" role="search" onsubmit={(event) => event.preventDefault()}>
							<label class="visually-hidden" for="catalog-search-input">Buscar nos cursos</label>
							<div class="search-field">
								<svg class="search-glyph" viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7" fill="none" stroke="currentColor" stroke-width="2"></circle><path d="m16.6 16.6 4.2 4.2" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"></path></svg>
								<input class="lvui-input" id="catalog-search-input" name="q" type="search" placeholder="Buscar curso ou aula…" autocomplete="off" bind:value={catalogQuery} />
								<kbd class="lv-search-kbd" aria-hidden="true">/</kbd>
							</div>
						</form>
					{/if}

					{#if studySyncSupported && libraryRecent.length && !catalogQuery.trim()}
						<section class="lv-section" aria-labelledby="recent-heading">
							<div class="lv-section__head">
								<div class="lv-section__title flex min-w-0 items-baseline gap-2">
									<h2 id="recent-heading">Continue estudando</h2>
									<span class="lv-count">{libraryRecent.length} {libraryRecent.length === 1 ? 'recente' : 'recentes'}</span>
								</div>
							</div>
							<ul class={`lv-resume${libraryRecent.length === 1 ? ' lv-resume--single' : ''}`}>
								{#each libraryRecent as entry (entry.item.mediaId)}
									{@const courseNode = findCourse(catalog, entry.location.courseId)}
									{#if courseNode}
										<li>
											<a class="lv-resume-card" href={href('course', entry.location.courseId, entry.location.lesson.path, entry.item.completed ? null : entry.item.positionSeconds)}>
												<span class="lv-resume-card__cover">
													<CourseCover course={courseNode} profile={user.userId} mediaUrl={(file, courseId) => transport.mediaUrl(file, courseId)} manualCoverUrl={transport.coverUrl(courseIdOf(courseNode), courseNode.coverVersion ?? null)} />
												</span>
												<span class="lv-resume-card__body">
													<span class="lv-resume-card__course">{entry.location.courseTitle}</span>
													<span class="lv-resume-card__lesson">{entry.location.lesson.title}</span>
													<span class="lv-resume-card__meta">{entry.item.completed ? 'Concluído' : `Parou em ${formatDuration(entry.item.positionSeconds)}`}</span>
												</span>
												<span class="lv-resume-card__action" aria-hidden="true">
													<svg viewBox="0 0 16 16"><path d="m6 3.5 7 4.5-7 4.5z" fill="currentColor"/></svg>
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
					{#if importedSuccess}
						<div class="lv-import-success" role="status" aria-live="polite">
							<div class="lv-import-success__info">
								<svg class="lv-import-success__icon" viewBox="0 0 24 24" aria-hidden="true">
									<path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
									<polyline points="22 4 12 14.01 9 11.01" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
								</svg>
								<div>
									<p class="lv-import-success__title">Curso importado com sucesso: <strong>{importedSuccess.title}</strong></p>
									<p class="lv-import-success__meta">
										{importedSuccess.lessonCount} {importedSuccess.lessonCount === 1 ? 'aula detectada' : 'aulas detectadas'}.
									</p>
								</div>
							</div>
							<div class="lv-import-success__actions">
								{#if importedSuccess.firstLessonPath}
									<a class="lvui-btn lvui-btn-primary lvui-btn-sm" href={href('course', importedSuccess.courseId, importedSuccess.firstLessonPath)}>
										<svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true"><polygon points="6 4 20 12 6 20 6 4"/></svg>
										Começar a estudar
									</a>
								{:else}
									<a class="lvui-btn lvui-btn-primary lvui-btn-sm" href={href('course', importedSuccess.courseId)}>
										Ver curso
									</a>
								{/if}
								<button type="button" class="lvui-btn lvui-btn-ghost lvui-btn-sm" onclick={() => (importedSuccess = null)} aria-label="Fechar aviso">
									Fechar
								</button>
							</div>
						</div>
					{/if}
					{#if importError}<p class="lv-alert" role="alert">{importError}</p>{/if}
					{#if user.role === 'admin' && rescanMessage}<p class="lv-notice" aria-live="polite">{rescanMessage}</p>{/if}
					{#if canManage}<input class="visually-hidden course-cover-input" type="file" accept={COVER_ACCEPT} bind:this={coverInput} onchange={pickCover} tabindex="-1" aria-hidden="true" />{/if}
					{#if coverError}<p class="lv-alert" role="alert">{coverError}</p>{/if}
					{#if coverMessage}<p class="lv-notice" role="status" aria-live="polite">{coverMessage}</p>{/if}

					<section class="lv-section" aria-labelledby="courses-heading">
						<div class="lv-section__head lv-section__head--courses">
							<div class="lv-section__title flex min-w-0 items-baseline gap-2">
								<h2 id="courses-heading">Todos os cursos</h2>
								{#if catalog && catalog.nodes.length}<span class="lv-count" aria-live="polite">{filteredNodes.length} de {catalog.nodes.length}</span>{/if}
							</div>
							<div class="lv-section__tools">
								<div class="lv-view-switch lvui-join" role="group" aria-label="Visualização dos cursos">
									{#each LIBRARY_VIEWS as view}
										<button type="button" class={`lvui-join-item lvui-btn lvui-btn-sm${libraryView === view ? ' lvui-btn-active' : ''}`} aria-pressed={libraryView === view} onclick={() => changeLibraryView(view)}>
											{#if view === 'covers'}<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="7" height="7" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.7"/><rect x="13" y="4" width="7" height="7" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.7"/><rect x="4" y="13" width="7" height="7" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.7"/><rect x="13" y="13" width="7" height="7" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.7"/></svg>
											{:else if view === 'list'}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6h16M4 12h16M4 18h16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>
											{:else if view === 'compact'}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>
											{:else}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 5v14M9 5v14M13 5v14M17 5v14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>{/if}
											<span>{LIBRARY_VIEW_LABELS[view]}</span>
										</button>
									{/each}
								</div>
							</div>
						</div>
						{#if catalog && catalog.nodes.length}
							{#if filteredNodes.length}
								<ul class="lv-course-grid" data-view={libraryView} aria-label="Cursos na visualização {LIBRARY_VIEW_LABELS[libraryView]}">
									{#each filteredNodes as courseItem (courseItem.courseId ?? courseItem.path)}
										<li class:is-menu-open={coverMenuId === courseIdOf(courseItem)}>
											<CourseCard course={courseItem} profile={user.userId} mediaUrl={(file, courseId) => transport.mediaUrl(file, courseId)} manualCoverUrl={transport.coverUrl(courseIdOf(courseItem), courseItem.coverVersion ?? null)} {canManage} inProgress={locatedRecent.some((entry) => entry.location.courseId === courseIdOf(courseItem) && !entry.item.completed)} menuOpen={coverMenuId === courseIdOf(courseItem)} onMenuOpen={(open) => toggleCoverMenu(courseItem, open)} onChangeCover={requestCourseCover} onRemoveCover={removeCover} />
										</li>
									{/each}
								</ul>
							{:else}
								<div class="lv-empty-state">
									<p class="lv-empty__title" role="status">Nenhum resultado encontrado</p>
									<p class="lv-lede">Nenhum curso ou aula corresponde a “{catalogQuery.trim()}”.</p>
									<button class="lv-btn" type="button" onclick={() => (catalogQuery = '')}>Limpar busca</button>
								</div>
							{/if}
						{:else if canManage}
							<div class="lv-empty-state">
								<p class="lv-empty__title">Sua biblioteca começa aqui</p>
								<p class="lv-lede">Use “Importar curso” acima para adicionar uma pasta. Os arquivos permanecem somente leitura.</p>
							</div>
						{:else}
							<div class="lv-empty-state"><p class="lv-empty__title">Nenhum curso disponível</p><p class="lv-lede">Peça ao administrador para importar um curso.</p></div>
						{/if}
					</section>

					{#if catalog && catalog.looseFiles.length}<div class="lv-section"><LooseFiles files={catalog.looseFiles} heading="Arquivos soltos" /></div>{/if}
				</div>
			{:else if route === 'statistics'}
				<Statistics {catalog} {recent} />
			{:else if route === 'notes'}
				<AllNotes {catalog} />
			{:else if route === 'settings'}
				<div class="lv-page lv-settings">
					<header class="lv-page__head">
						<div>
							<h1>Ajustes</h1>
						<p class="lv-lede">Aparência, ritmo de foco e preferências deste dispositivo.</p>
					</div>
				</header>
					<div class="lv-settings__grid">
					<section class="lv-panel lv-settings__panel">
						<div class="lv-panel__head"><h2>Tema</h2></div>
						<fieldset class="theme-options">
							<legend class="visually-hidden">Tema</legend>
							<label><input type="radio" name="theme" value="system" checked={themeMode === 'system'} onchange={() => changeTheme('system')} />Sistema <span>Acompanha a preferência do sistema.</span></label>
							<label><input type="radio" name="theme" value="light" checked={themeMode === 'light'} onchange={() => changeTheme('light')} />Claro <span>Usa sempre o tema claro.</span></label>
							<label><input type="radio" name="theme" value="dark" checked={themeMode === 'dark'} onchange={() => changeTheme('dark')} />Escuro <span>Usa sempre o tema escuro.</span></label>
						</fieldset>
					</section>
					<section class="lv-panel lv-settings__panel" aria-labelledby="profile-heading">
						<div class="lv-panel__head"><h2 id="profile-heading">Perfil</h2><span class="lv-count">Foto e identidade</span></div>
						<div class="lv-settings__body">
							<UserProfile {user} {avatarVersion} onAvatarChange={handleAvatarChanged} />
						</div>
					</section>
					<section class="lv-panel lv-settings__panel" aria-labelledby="timer-heading">
						<div class="lv-panel__head"><h2 id="timer-heading">Pomodoro</h2><span class="lv-count">Tempo de foco e pausas</span></div>
						<div class="lv-settings__body">
							<PomodoroSettings />
						</div>
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
					<section class="lv-panel lv-settings__panel" aria-labelledby="update-heading">
						<div class="lv-panel__head"><h2 id="update-heading">Atualização</h2>{#if updateStatus}<span class="lv-count">Versão {updateStatus.current}</span>{/if}</div>
						<div class="lv-settings__body">
							<button class="lv-btn" type="button" onclick={() => checkUpdate(true)} disabled={updateChecking} aria-busy={updateChecking}>{updateChecking ? 'Verificando…' : 'Verificar atualização'}</button>
							{#if updateStatus}
								{@const outcome = updateOutcome(updateStatus)}
								{@const releaseUrl = updateReleaseUrl(updateStatus)}
								{#if outcome !== 'error' || updateManual}<p class="lv-notice" role="status">{updateMessage(updateStatus)}</p>{/if}
								{#if outcome === 'available' && releaseUrl}<p class="lv-notice"><a href={releaseUrl} target="_blank" rel="noopener noreferrer">Ver a release</a></p>{/if}
								{#if outcome === 'available' && transportKind === 'desktop'}
									<div class="lv-update-actions" style="margin-top: 0.5rem;">
										<button class="lv-btn lv-btn--primary" type="button" disabled={updateInstalling} onclick={installUpdate} aria-busy={updateInstalling}>
											{updateInstalling ? 'Instalando atualização…' : 'Atualizar e reiniciar'}
										</button>
										{#if updateInstallError}<p class="error" role="alert" style="margin-top: 0.25rem;">{updateInstallError}</p>{/if}
									</div>
								{/if}
								{#if outcome === 'available' && updateStatus.changelog}
									<details class="lv-update-changelog">
										<summary class="lv-update-changelog__summary">Novidades desta versão (Changelog)</summary>
										<pre class="lv-update-changelog__content">{updateStatus.changelog}</pre>
									</details>
								{/if}
							{/if}
							{#if updateHistoryList.length > 0}
								<details class="lv-update-history">
									<summary class="lv-update-history__summary">Histórico de atualizações ({updateHistoryList.length})</summary>
									<ul class="lv-update-history__list">
										{#each updateHistoryList as entry}
											<li class="lv-update-history__item">
												<div class="lv-update-history__header">
													<span class="lv-update-history__status lv-update-history__status--{entry.status}">{entry.status === 'success' ? 'Sucesso' : entry.status === 'rollback' ? 'Reversão' : entry.status}</span>
													<span class="lv-update-history__versions">{entry.from} → {entry.to}</span>
													<time class="lv-update-history__time">{entry.timestamp}</time>
												</div>
												<p class="lv-update-history__msg">{entry.message}</p>
											</li>
										{/each}
									</ul>
								</details>
							{/if}
						</div>
					</section>
					{#if transportKind === 'http' && canManage}
						<section class="lv-panel lv-settings__panel" aria-labelledby="backup-heading">
							<div class="lv-panel__head"><h2 id="backup-heading">Backup do Banco de Dados</h2></div>
							<div class="lv-settings__body">
								<p class="lv-notice">Gere e baixe uma cópia transacional consistente do banco SQLite (verificada com integridade estrutural).</p>
								<a class="lv-btn lv-btn--outline" href="/api/admin/backup" download>Baixar backup agora (.sqlite)</a>
							</div>
						</section>
					{/if}
					</div>
				</div>
			{:else if isCourseRoute}
				{#if !course}
					<div class="lv-page lv-page--narrow">
						<nav class="lv-breadcrumb" aria-label="Caminho"><a href={href('home')}>Biblioteca</a><span aria-hidden="true">/</span><span>Curso</span></nav>
						<div class="lv-empty-state"><p class="lv-empty__title">Curso não encontrado</p><p class="lv-lede">Ele pode ter sido removido do catálogo atual.</p><a class="lv-btn" href={href('home')}>Voltar à biblioteca</a></div>
					</div>
				{:else if isOverview}
					{@const counts = countCatalogNode(course)}
					{@const missingVideos = countMissingVideos(course)}
					{@const meta = formatCourseMeta(counts)}
					{@const cover = courseCover(course.title)}
					{@const materials = collectMaterials(course)}
					<div class="lv-page lv-course-overview">
						<nav class="lv-breadcrumb" aria-label="Caminho do curso"><a href={href('home')}>Biblioteca</a><span aria-hidden="true">/</span><span>{course.title}</span></nav>
						<header class="lv-course-hero">
							<div class="lv-course-hero__cover">
								<CourseCover course={course} profile={user.userId} mediaUrl={(file, courseId) => transport.mediaUrl(file, courseId)} manualCoverUrl={transport.coverUrl(courseIdOf(course), course.coverVersion ?? null)} size="hero" />
							</div>
							<div class="lv-course-hero__main">
								<h1>{course.title}</h1>
								{#if meta}<p class="lv-course-hero__meta">{meta}</p>{/if}
							</div>
							{#if courseRecent}
								<p class="lv-course-hero__resume">Última aula: {courseRecent.location.lesson.title} · {courseRecent.item.completed ? 'concluída' : `parou em ${formatDuration(courseRecent.item.positionSeconds)}`}</p>
							{/if}
							<div class="lv-course-hero__actions">
								{#if activePath}
									<a class="lv-btn lv-btn--primary" href={href('course', courseIdOf(course), courseRecent ? courseRecent.location.lesson.path : activePath, courseRecent && !courseRecent.item.completed ? courseRecent.item.positionSeconds : null)}>{courseRecent ? 'Continuar estudando' : 'Começar curso'}</a>
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
										<div><dt>Aulas</dt><dd>{counts.lessons}{#if counts.missingLessons > 0}<span class="lv-meta__missing">{formatCount(counts.missingLessons, 'ausente', 'ausentes')}</span>{/if}</dd></div>
										<div><dt>Vídeos</dt><dd>{formatCount(counts.videos - missingVideos, 'disponível', 'disponíveis')}{#if missingVideos > 0}<span class="lv-meta__missing">{formatCount(missingVideos, 'ausente', 'ausentes')}</span>{/if}</dd></div>
										<div><dt>Materiais</dt><dd>{counts.materials}</dd></div>
										<div><dt>Código</dt><dd class="lv-meta__code">{cover.code}</dd></div>
									</dl>
								</section>
							{#if courseRecent}
								<section class="lv-panel lv-resume-panel" aria-labelledby="course-resume-heading">
									<div class="lv-panel__head"><h2 id="course-resume-heading">Continue de onde parou</h2></div>
									<div class="lv-resume-panel__body">
										<p class="lv-resume-panel__lesson">{courseRecent.location.lesson.title}</p>
										<p class="lv-resume-panel__meta">{formatLocationContext(courseRecent.location.courseTitle, courseRecent.location.moduleTitle)}{#if !courseRecent.item.completed}{' · '}{formatDuration(courseRecent.item.positionSeconds)}{/if}</p>
										<a class="lv-btn lv-btn--primary" href={href('course', courseIdOf(course), courseRecent.location.lesson.path, courseRecent.item.completed ? null : courseRecent.item.positionSeconds)}>{courseRecent.item.completed ? 'Revisar aula' : 'Retomar aula'}</a>
									</div>
								</section>
							{/if}
							</aside>
						</div>
					</div>
				{:else}
					<div class="lv-page lv-player-page">
						<div class="lv-player-top">
							<nav class="lv-breadcrumb" aria-label="Caminho do curso"><a href={href('home')}>Biblioteca</a><span aria-hidden="true">/</span><a href={href('course', courseIdOf(course))} aria-label={`Voltar ao curso ${course.title}`}><span class="lv-breadcrumb__back-label" aria-hidden="true">Voltar ao curso</span>{course.title}</a><span aria-hidden="true">/</span><span>{lesson?.title ?? 'Aula'}</span></nav>
						</div>
						{#if removeError}<p class="lv-alert" role="alert">{removeError}</p>{/if}
						{#if orderError}<p class="lv-alert" role="alert">{orderError}</p>{/if}
						{#if orderMessage}<p class="lv-notice" role="status" aria-live="polite">{orderMessage}</p>{/if}
						<div class="lv-player-layout">
							<div class="lv-player-main">
								{#if lesson}
									<div class="lv-lesson-head">
										<div>
											<h1>{lesson.title}</h1>
										</div>
										<div class="lv-lesson-actions">
											<a class="lv-btn lv-btn--ghost" href={href('course', courseIdOf(course))}>Visão geral</a>
											{#if neighbors.previous}<a class="lv-btn" href={href('course', courseIdOf(course), neighbors.previous.path)}>Anterior</a>{/if}
											{#if neighbors.next}<a class="lv-btn" href={href('course', courseIdOf(course), neighbors.next.path)}>Próxima</a>{/if}
											<button class="lv-btn" type="button" aria-pressed={focusMode} onclick={() => (focusMode = !focusMode)}>{focusMode ? 'Sair do foco' : 'Modo foco'}</button>
										</div>
									</div>
									<Player {lesson} courseId={courseIdOf(course)} lessonTitle={lesson.title} initialPosition={positionSeconds} {mediaError} onMediaError={mediaFailed} {studyEnabled} preferences={playerPreferences}
										onPreferencesChange={savePlayerPreferences} autoNext={playerPreferences.autoNext}
										hasPreviousLesson={neighbors.previous !== null}
										hasNextLesson={neighbors.next !== null}
										onPreviousLesson={() => { if (neighbors.previous) location.hash = href('course', courseIdOf(course), neighbors.previous.path); }}
										onNextLesson={() => { if (neighbors.next) location.hash = href('course', courseIdOf(course), neighbors.next.path); }}
										onAutoNext={autoAdvance}
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
						<Player {lesson} courseId={courseId ?? ''} initialPosition={positionSeconds} {mediaError} onMediaError={mediaFailed} {studyEnabled} preferences={playerPreferences}
							onPreferencesChange={savePlayerPreferences} autoNext={playerPreferences.autoNext}
							hasPreviousLesson={neighbors.previous !== null}
							hasNextLesson={neighbors.next !== null}
							onPreviousLesson={() => { if (neighbors.previous) location.hash = href('player', courseId ?? '', neighbors.previous.path); }}
							onNextLesson={() => { if (neighbors.next) location.hash = href('player', courseId ?? '', neighbors.next.path); }}
							onAutoNext={autoAdvance}
							onAutoNextChange={(enabled) => savePlayerPreferences({ ...playerPreferences, autoNext: enabled })} />
						{#if neighbors.previous || neighbors.next}<nav class="lv-lesson-nav" aria-label="Aulas vizinhas">{#if neighbors.previous}<a href={href('player', courseId ?? '', neighbors.previous.path)}>← {neighbors.previous.title}</a>{/if}{#if neighbors.next}<a href={href('player', courseId ?? '', neighbors.next.path)}>{neighbors.next.title} →</a>{/if}</nav>{/if}
					{/if}
				</div>
			{/if}
		</main>
	</div>
{/if}
