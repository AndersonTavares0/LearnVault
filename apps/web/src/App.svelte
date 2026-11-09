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
	import AdminUsers from './AdminUsers.svelte';
	import { filterCatalog } from './search';
	import { collectMaterials, countCatalogNode, countMissingVideos, formatCategoryName, formatCount, formatCourseMeta, formatLessonCounts, formatLocationContext, getCourseProgressStatus, type CourseProgressStatus } from './catalog';
	import { courseCover } from './cover';
	import PasswordField from './PasswordField.svelte';
	import { defaultLessonPath, firstAvailableLessonPath, lessonContainer, neighboringLessons } from './navigation';
	import { destinationAfterLogin, findCourse, findLesson, findLessonLocation, href, locationFromHash, type LessonLocation, type Route } from './router';
	import { transport, transportKind, COVER_ACCEPT, MAX_COVER_BYTES, courseIdOf, STUDY_NOT_AVAILABLE, type Catalog, type CatalogNode, type Lesson, type StudyNote, type StudyProgress, type UpdateStatus, type UpdateHistoryEntry, type User } from './transport';
	import { updateOutcome, updateMessage, updateReleaseUrl } from './update';
	import { defaultRoute, resolveSession } from './session';
	import { formatDuration, studySyncSupported } from './player';
	import { createDefaultPlayerPreferences, playerPreferencesScope, readPlayerPreferences, writePlayerPreferences, type PlayerPreferences } from './player-preferences';
	import Brand from './Brand.svelte';
	import { getLocale, LOCALES, LOCALE_LABELS, readLocale, setLocale, type Locale } from './i18n.svelte';
	import { applyTheme, persistThemeMode, readThemeMode, resolveTheme, type ThemeMode } from './theme';
	import { LIBRARY_VIEWS, persistLibraryView, readLibraryView, type LibraryView } from './library-view';

	let themeMode: ThemeMode = readThemeMode(safeLocalStorage());
	let locale: Locale = setLocale(readLocale(safeLocalStorage(), typeof navigator !== 'undefined' ? navigator.language : null), safeLocalStorage(), typeof document !== 'undefined' ? document.documentElement : null);
	$: tr = (en: string, ptBr: string) => (locale === 'pt-BR' ? ptBr : en);
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
	let avatarVersion = 0;
	let topbarAvatarFailed = false;

	function handleAvatarChanged() {
		topbarAvatarFailed = false;
		avatarVersion = Date.now();
		try {
			if (user?.userId) localStorage.setItem('learnvault_avatar_' + user.userId, String(avatarVersion));
		} catch {
			// localStorage unavailable
		}
	}
	let user: User | null = null;
	let catalog: Catalog | null = null;
	let lesson: Lesson | null = null;
	let catalogQuery = '';
	let progressFilter: CourseProgressStatus = 'all';
	let sourceFolderFilter = 'all';
	let favoritesOnly = false;
	let favoriteCourseIds: Set<string> = new Set();
	let favoriteMediaIds: Set<string> = new Set();
	let filteredNodes: CatalogNode[] = [];
	let allNotesForSearch: StudyNote[] = [];
	let notesLoaded = false;
	$: course = findCourse(catalog, courseId);
	$: completedMediaIds = new Set(recent.filter((item) => item.completed).map((item) => item.mediaId));
	$: inProgressMediaIds = new Set(recent.filter((item) => !item.completed && item.positionSeconds > 0).map((item) => item.mediaId));
	$: courseStatusMap = new Map(
		(catalog?.nodes ?? []).map((node) => [
			courseIdOf(node),
			getCourseProgressStatus(node, completedMediaIds, inProgressMediaIds)
		])
	);
	$: progressCounts = {
		all: catalog?.nodes.length ?? 0,
		in_progress: (catalog?.nodes ?? []).filter((n) => courseStatusMap.get(courseIdOf(n)) === 'in_progress').length,
		not_started: (catalog?.nodes ?? []).filter((n) => courseStatusMap.get(courseIdOf(n)) === 'not_started').length,
		completed: (catalog?.nodes ?? []).filter((n) => courseStatusMap.get(courseIdOf(n)) === 'completed').length,
		favorites: (catalog?.nodes ?? []).filter((n) => favoriteCourseIds.has(courseIdOf(n))).length
	};
	$: availableSourceFolders = catalog
		? Array.from(new Set(catalog.nodes.map((n) => n.sourceFolder).filter((f): f is string => Boolean(f)))).sort((a, b) => a.localeCompare(b))
		: [];
	$: if (sourceFolderFilter !== 'all' && !availableSourceFolders.includes(sourceFolderFilter)) {
		sourceFolderFilter = 'all';
	}
	$: filteredNodes = (catalog ? filterCatalog(catalog.nodes, catalogQuery) : []).filter((node) => {
		if (favoritesOnly && !favoriteCourseIds.has(courseIdOf(node))) return false;
		if (progressFilter !== 'all' && courseStatusMap.get(courseIdOf(node)) !== progressFilter) return false;
		if (sourceFolderFilter !== 'all' && node.sourceFolder !== sourceFolderFilter) return false;
		return true;
	});
	$: matchingNotes = catalogQuery.trim()
		? allNotesForSearch.filter((note) => {
				const q = catalogQuery.toLowerCase().trim();
				return note.content.toLowerCase().includes(q) || (note.relativePath && note.relativePath.toLowerCase().includes(q));
		  })
		: [];
	$: if (catalogQuery.trim() && !notesLoaded && user) {
		void loadNotesForSearch();
	}
	$: if (route !== 'home' && notesLoaded) {
		notesLoaded = false;
	}
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
	$: currentLessonMediaId = lesson?.media.find((f) => f.mediaId)?.mediaId ?? null;
	$: activeContainer = course && activePath ? lessonContainer(course.children, activePath) : null;
	$: locatedRecent = recent.flatMap((item) => {
		const location = findLessonLocation(catalog, item.mediaId);
		return location ? [{ item, location }] : [];
	});
	$: libraryRecent = (() => {
		const seen = new Set<string>();
		const list: typeof locatedRecent = [];
		for (const entry of locatedRecent) {
			if (!seen.has(entry.location.courseId)) {
				seen.add(entry.location.courseId);
				list.push(entry);
				if (list.length >= 4) break;
			}
		}
		return list;
	})();
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
	function libraryViewLabel(view: LibraryView): string {
		switch (view) {
			case 'covers':
				return tr('Covers', 'Capas');
			case 'list':
				return tr('List', 'Lista');
			case 'compact':
				return tr('Compact', 'Compacto');
			case 'shelf':
				return tr('Shelf', 'Estante');
		}
	}
	function changeLibraryView(view: LibraryView) {
		libraryView = view;
		persistLibraryView(safeLocalStorage(), view);
	}
	function changeLocale(next: Locale) {
		locale = setLocale(next, safeLocalStorage(), typeof document !== 'undefined' ? document.documentElement : null);
	}
	function toggleLocale() {
		changeLocale(locale === 'en' ? 'pt-BR' : 'en');
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
			allNotesForSearch = [];
			notesLoaded = false;
			loadPlayerPreferences(session.user);
			error = session.error;
			if (user) {
				try {
					const stored = localStorage.getItem('learnvault_avatar_' + user.userId);
					if (stored) avatarVersion = Number(stored);
				} catch {
					avatarVersion = 0;
				}
				catalog = await transport.catalog();
				await Promise.all([loadRecent(), loadFavorites()]);
				if (route === 'login') route = 'home';
			}
		} catch {
			error = tr('Could not connect to the service.', 'Não foi possível conectar ao serviço.');
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
			try {
				const stored = localStorage.getItem('learnvault_avatar_' + user.userId);
				avatarVersion = stored ? Number(stored) : 0;
			} catch {
				avatarVersion = 0;
			}
			allNotesForSearch = [];
			notesLoaded = false;
			loadPlayerPreferences(user);
			password = '';
			catalog = await transport.catalog();
			await Promise.all([loadRecent(), loadFavorites()]);
			location.hash = destinationAfterLogin(destination);
		} catch (cause) {
			error = cause instanceof Error ? cause.message : tr('Could not sign in.', 'Não foi possível entrar.');
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
			accountStatus = tr('Credentials updated. Use them on your next sign-in.', 'Credenciais atualizadas. Use-as no próximo acesso.');
		} catch (cause) {
			accountError = cause instanceof Error ? cause.message : tr('Could not update credentials.', 'Não foi possível atualizar as credenciais.');
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
		favoriteCourseIds = new Set();
		favoriteMediaIds = new Set();
		favoritesOnly = false;
		allNotesForSearch = [];
		notesLoaded = false;
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
		rescanMessage = tr('Reading courses…', 'Lendo os cursos…');
		try {
			await transport.rescan();
			syncCatalog(await transport.catalog());
			await loadRecent();
			rescanMessage = tr('Catalog updated.', 'Catálogo atualizado.');
		} catch (cause) {
			rescanMessage = cause instanceof Error ? cause.message : tr('Failed to index.', 'Falha ao indexar.');
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
			importError = cause instanceof Error ? cause.message : tr('Could not import course.', 'Não foi possível importar o curso.');
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
			importError = cause instanceof Error ? cause.message : tr('Could not import course.', 'Não foi possível importar o curso.');
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
			removeError = cause instanceof Error ? cause.message : tr('Could not remove course.', 'Não foi possível remover o curso.');
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

	async function loadNotesForSearch() {
		try {
			allNotesForSearch = await transport.listNotes();
			notesLoaded = true;
		} catch {
			allNotesForSearch = [];
		}
	}

	async function loadFavorites() {
		if (!studySyncSupported || typeof transport.listFavorites !== 'function') return;
		try {
			const favs = await transport.listFavorites();
			favoriteCourseIds = new Set(favs.courseIds);
			favoriteMediaIds = new Set(favs.mediaIds);
		} catch {
			favoriteCourseIds = new Set();
			favoriteMediaIds = new Set();
		}
	}

	async function handleToggleCourseFavorite(courseItem: CatalogNode) {
		if (!studySyncSupported || typeof transport.setCourseFavorite !== 'function') return;
		const id = courseIdOf(courseItem);
		const nextFav = !favoriteCourseIds.has(id);
		try {
			const saved = await transport.setCourseFavorite(id, nextFav);
			const nextSet = new Set(favoriteCourseIds);
			if (saved) nextSet.add(id);
			else nextSet.delete(id);
			favoriteCourseIds = nextSet;
		} catch {
			// Ignora falha transitória
		}
	}

	async function handleToggleLessonMediaFavorite(mediaId: string) {
		if (!studySyncSupported || !mediaId || typeof transport.setLessonFavorite !== 'function') return;
		const nextFav = !favoriteMediaIds.has(mediaId);
		try {
			const saved = await transport.setLessonFavorite(mediaId, nextFav);
			const nextSet = new Set(favoriteMediaIds);
			if (saved) nextSet.add(mediaId);
			else nextSet.delete(mediaId);
			favoriteMediaIds = nextSet;
		} catch {
			// Ignora falha transitória
		}
	}

	async function handleToggleLessonFavorite(lessonNode: CatalogNode) {
		const media = [...lessonNode.files, ...lessonNode.looseFiles].find((f) => f.kind === 'media' && f.mediaId);
		if (!media?.mediaId) return;
		await handleToggleLessonMediaFavorite(media.mediaId);
	}

	async function handleToggleLessonCompleted(lessonNode: CatalogNode) {
		if (!studySyncSupported) return;
		const media = [...lessonNode.files, ...lessonNode.looseFiles].find((f) => f.kind === 'media' && f.mediaId);
		if (!media || !media.mediaId) return;
		const id = media.mediaId;
		const currentCompleted = completedMediaIds.has(id);
		try {
			const updated = await transport.setCompleted(id, !currentCompleted);
			const existingIndex = recent.findIndex((item) => item.mediaId === id);
			if (existingIndex >= 0) {
				const nextRecent = [...recent];
				nextRecent[existingIndex] = updated;
				recent = nextRecent;
			} else {
				recent = [updated, ...recent];
			}
		} catch {
			// Ignora falha transitória
		}
	}

	function mediaFailed(code?: number) {
		if (code === 4) {
			mediaError = tr('Incompatible media codec or format.', 'Codec ou formato de mídia incompatível.');
		} else if (code === 3) {
			mediaError = tr('Media decoding failed.', 'Falha na decodificação da mídia.');
		} else {
			mediaError = tr('Could not load this media.', 'Não foi possível carregar esta mídia.');
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
			coverMessage = tr('Cover updated.', 'Capa atualizada.');
		} catch (cause) {
			coverError = cause instanceof Error ? cause.message : tr('Could not save course cover.', 'Não foi possível salvar a capa do curso.');
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
			coverError = tr('Image is larger than 4 MiB.', 'A imagem é maior que 4 MiB.');
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
			coverMessage = removed
				? tr('Cover removed.', 'Capa removida.')
				: tr('No custom cover to remove.', 'Nenhuma capa personalizada para remover.');
		} catch (cause) {
			coverError = cause instanceof Error ? cause.message : tr('Could not remove course cover.', 'Não foi possível remover a capa do curso.');
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
			orderMessage = tr('Automatic order restored.', 'Ordem automática restaurada.');
		} catch (cause) {
			orderError = cause instanceof Error ? cause.message : tr('Could not restore order.', 'Não foi possível restaurar a ordem.');
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
					error: cause instanceof Error ? cause.message : tr('Could not check for updates.', 'Não foi possível verificar a atualização.')
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
				updateInstallError = tr('No update available to install right now.', 'Nenhuma atualização disponível para instalação no momento.');
			}
		} catch (cause) {
			updateInstallError = cause instanceof Error ? cause.message : tr('Could not install the update.', 'Não foi possível instalar a atualização.');
		} finally {
			updateInstalling = false;
		}
	}
</script>
<svelte:head><title>{tr('LearnVault — your study library', 'LearnVault — sua biblioteca de estudos')}</title><meta name="description" content={tr('Your personal course library.', 'Sua biblioteca pessoal de cursos.')} /></svelte:head>
{#if transportKind === 'desktop'}<DesktopTitlebar />{/if}
{#if loading}<div class="lv-loading" aria-live="polite">{tr('Loading your courses…', 'Carregando seus cursos…')}</div>
{:else if !user}
	<main class="login-page" aria-labelledby="login-heading">
		<section>
			<div class="login-head">
				<Brand />
				<h1 id="login-heading">{tr('Sign in to LearnVault', 'Entrar no LearnVault')}</h1>
				<button
					class="lvui-btn lvui-btn-ghost lvui-btn-sm login-head__locale"
					type="button"
					aria-label={tr('Switch language to Português (Brasil)', 'Mudar idioma para English')}
					title={tr('Português (Brasil)', 'English')}
					onclick={toggleLocale}
				>
					{locale === 'en' ? 'EN' : 'PT'}
				</button>
			</div>
			<p class="intro">{tr('Access your course library.', 'Acesse sua biblioteca de cursos.')}</p>
			<form onsubmit={login}>
				<fieldset class="auth-fields" disabled={authBusy}>
					<label class="form-field" for="login-username">{tr('Username', 'Usuário')}<input class="lvui-input" id="login-username" name="username" autocomplete="username" autocapitalize="none" spellcheck={false} bind:value={username} required /></label>
					<PasswordField id="login-password" label={tr('Password', 'Senha')} autocomplete="current-password" bind:value={password} />
					{#if error}<p class="error" role="alert">{error}</p>{/if}
					<button class="lvui-btn lvui-btn-primary lvui-btn-block" type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? tr('Signing in…', 'Entrando…') : tr('Sign in', 'Entrar')}</button>
				</fieldset>
			</form>
		</section>
	</main>
{:else}
	<a class="lv-skip" href="#main-content">{tr('Skip to content', 'Ir para o conteúdo')}</a>
	<div class="lv-shell" class:is-focus={focusMode} class:is-desktop={transportKind === 'desktop'}>
		<header class="lv-topbar flex flex-wrap items-center gap-5 min-h-14 px-4 lg:px-8">
			<a class="lv-topbar__brand" href={href('home')} aria-label={tr('LearnVault, home', 'LearnVault, início')}>
				<Brand />
				<span>LearnVault</span>
			</a>
			<nav class="lv-topbar__nav" aria-label={tr('Sections', 'Seções')}>
				<a class="lv-topbar__link" class:is-active={route === 'home'} aria-current={route === 'home' ? 'page' : undefined} href={href('home')}>
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5.5h6.5V19H4zM13.5 5.5H20V19h-6.5z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/></svg>
					<span>{tr('Library', 'Biblioteca')}</span>
				</a>
				<a class="lv-topbar__link" class:is-active={route === 'statistics'} aria-current={route === 'statistics' ? 'page' : undefined} href={href('statistics')}>
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 19.5h16M7 19.5v-6M12 19.5V8M17 19.5V5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>
					<span>{tr('Statistics', 'Estatísticas')}</span>
				</a>
				<a class="lv-topbar__link" class:is-active={route === 'notes'} aria-current={route === 'notes' ? 'page' : undefined} href={href('notes')}>
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M19 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2zM7 7h10M7 12h10M7 17h7" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>
					<span>{tr('Notes', 'Anotações')}</span>
				</a>
				<a class="lv-topbar__link" class:is-active={route === 'settings'} aria-current={route === 'settings' ? 'page' : undefined} href={href('settings')}>
					<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><circle cx="9" cy="7" r="2.2" fill="var(--rail-bg)" stroke="currentColor" stroke-width="1.6"/><circle cx="15" cy="12" r="2.2" fill="var(--rail-bg)" stroke="currentColor" stroke-width="1.6"/><circle cx="9" cy="17" r="2.2" fill="var(--rail-bg)" stroke="currentColor" stroke-width="1.6"/></svg>
					<span>{tr('Settings', 'Ajustes')}</span>
				</a>
			</nav>
			<div class="lv-topbar__utils">
				<FocusMenu />
				<button
					class="lv-topbar__icon"
					type="button"
					aria-label={tr('Switch language to Português (Brasil)', 'Mudar idioma para English')}
					title={tr('Language: English (click for Português)', 'Idioma: Português (clique para English)')}
					onclick={toggleLocale}
				>
					<span class="lv-topbar__locale">{locale === 'en' ? 'EN' : 'PT'}</span>
				</button>
				<button class="lv-topbar__icon" type="button" aria-label={resolvedTheme === 'dark' ? tr('Switch to light theme', 'Mudar para tema claro') : tr('Switch to dark theme', 'Mudar para tema escuro')} title={resolvedTheme === 'dark' ? tr('Light theme', 'Tema claro') : tr('Dark theme', 'Tema escuro')} onclick={() => changeTheme(resolvedTheme === 'dark' ? 'light' : 'dark')}>
					{#if resolvedTheme === 'dark'}
						<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M18.7 5.3l-1.4 1.4M6.7 17.3l-1.4 1.4" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>
					{:else}
						<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5Z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"/></svg>
					{/if}
				</button>
				<div class="lv-topbar__user" title={user.username}>
					{#if !topbarAvatarFailed && Boolean(avatarVersion) && transport.userAvatarUrl(avatarVersion)}
						<img class="lv-avatar lv-avatar--img" src={transport.userAvatarUrl(avatarVersion)} alt={tr(`Avatar of ${user.username}`, `Avatar de ${user.username}`)} onerror={() => (topbarAvatarFailed = true)} />
					{:else}
						<span class="lv-avatar" aria-hidden="true">{userInitials}</span>
					{/if}
					<span class="lv-topbar__username">{user.username}</span>
				</div>
				{#if transportKind === 'http'}
					<button class="lv-topbar__logout" type="button" onclick={logout} aria-label={tr('Sign out', 'Sair')} title={tr('Sign out', 'Sair')}>{tr('Sign out', 'Sair')}</button>
				{/if}
			</div>
		</header>
		<main id="main-content" class="lv-main">
			{#if error}<p class="lv-alert" role="alert">{error}</p>{/if}
			{#if route === 'home'}
				<div class="lv-page lv-library">
					<header class="lv-page__head">
						<div>
							<h1>{tr('My library', 'Minha biblioteca')}</h1>
							<p class="lv-lede">{tr('Organized courses. Original files untouched.', 'Cursos organizados. Arquivos originais intactos.')}</p>
						</div>
						<div class="lv-page__actions">
							{#if canManage}
								<div class="lv-page__buttons">
									{#if transportKind === 'desktop'}
										<button class={`lvui-btn${studySyncSupported && libraryRecent.length > 0 && !catalogQuery.trim() ? '' : ' lvui-btn-primary'}`} onclick={importCourse} disabled={importing} aria-busy={importing}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4v10m-4-4 4 4 4-4M5 17v3h14v-3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>{importing ? tr('Importing…', 'Importando…') : tr('Import course', 'Importar curso')}</button>
									{:else}
										<button class={`lvui-btn${studySyncSupported && libraryRecent.length > 0 && !catalogQuery.trim() ? '' : ' lvui-btn-primary'}`} onclick={() => (importFormOpen = !importFormOpen)} aria-expanded={importFormOpen}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4v10m-4-4 4 4 4-4M5 17v3h14v-3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>{tr('Import course', 'Importar curso')}</button>
									{/if}
									<button class="lvui-btn" onclick={rescan} disabled={rescanning || !catalog || !catalog.nodes.length}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7v4h-4M4 17v-4h4M5.5 9a7 7 0 0 1 12-2L20 11M4 13l2.5 4a7 7 0 0 0 12-2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>{rescanning ? tr('Refreshing…', 'Atualizando…') : tr('Refresh courses', 'Atualizar cursos')}</button>
								</div>
								{#if rescanMessage}<p class="lv-page__status" aria-live="polite">{rescanMessage}</p>{/if}
							{/if}
						</div>
					</header>

					{#if catalog && catalog.nodes.length}
						<form class="lv-library-search" role="search" onsubmit={(event) => event.preventDefault()}>
							<label class="visually-hidden" for="catalog-search-input">{tr('Search courses', 'Buscar nos cursos')}</label>
							<div class="search-field">
								<svg class="search-glyph" viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7" fill="none" stroke="currentColor" stroke-width="2"></circle><path d="m16.6 16.6 4.2 4.2" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"></path></svg>
								<input class="lvui-input" id="catalog-search-input" name="q" type="search" placeholder={tr('Search course or lesson…', 'Buscar curso ou aula…')} autocomplete="off" bind:value={catalogQuery} />
								<kbd class="lv-search-kbd" aria-hidden="true">/</kbd>
							</div>
						</form>
						<div class="lv-library-filters">
							<div class="lv-filter-group">
								<span class="lv-filter-group__label">{tr('Status', 'Status')}</span>
								<div class="lv-progress-filters" role="group" aria-label={tr('Filter courses by progress', 'Filtrar cursos por progresso')}>
									<button type="button" class="lv-filter-pill" class:is-active={progressFilter === 'all'} aria-pressed={progressFilter === 'all'} onclick={() => (progressFilter = 'all')}>
										{tr('All', 'Todos')} <span class="lv-filter-badge">{progressCounts.all}</span>
									</button>
									<button type="button" class="lv-filter-pill" class:is-active={progressFilter === 'in_progress'} aria-pressed={progressFilter === 'in_progress'} onclick={() => (progressFilter = 'in_progress')}>
										{tr('In progress', 'Em andamento')} <span class="lv-filter-badge">{progressCounts.in_progress}</span>
									</button>
									<button type="button" class="lv-filter-pill" class:is-active={progressFilter === 'not_started'} aria-pressed={progressFilter === 'not_started'} onclick={() => (progressFilter = 'not_started')}>
										{tr('Not started', 'Não iniciados')} <span class="lv-filter-badge">{progressCounts.not_started}</span>
									</button>
									<button type="button" class="lv-filter-pill" class:is-active={progressFilter === 'completed'} aria-pressed={progressFilter === 'completed'} onclick={() => (progressFilter = 'completed')}>
										{tr('Completed', 'Concluídos')} <span class="lv-filter-badge">{progressCounts.completed}</span>
									</button>
									{#if studySyncSupported}
										<button type="button" class="lv-filter-pill" class:is-active={favoritesOnly} aria-pressed={favoritesOnly} onclick={() => (favoritesOnly = !favoritesOnly)}>
											{tr('★ Favorites', '★ Favoritos')} <span class="lv-filter-badge">{progressCounts.favorites}</span>
										</button>
									{/if}
								</div>
							</div>
							{#if availableSourceFolders.length > 1}
								<div class="lv-filter-group lv-filter-group--sources">
									<span class="lv-filter-group__label">{tr('Category', 'Categoria')}</span>
									<div class="lv-progress-filters lv-source-filters" role="group" aria-label={tr('Filter courses by source folder', 'Filtrar cursos por pasta de origem')}>
										<button type="button" class="lv-filter-pill" class:is-active={sourceFolderFilter === 'all'} aria-pressed={sourceFolderFilter === 'all'} onclick={() => (sourceFolderFilter = 'all')}>
											{tr(`All folders (${catalog?.nodes.length ?? 0})`, `Todas as pastas (${catalog?.nodes.length ?? 0})`)}
										</button>
										{#each availableSourceFolders as folder}
											{@const count = (catalog?.nodes ?? []).filter((n) => n.sourceFolder === folder).length}
											<button type="button" class="lv-filter-pill" class:is-active={sourceFolderFilter === folder} aria-pressed={sourceFolderFilter === folder} onclick={() => (sourceFolderFilter = folder)}>
												{formatCategoryName(folder)} <span class="lv-filter-badge">{count}</span>
											</button>
										{/each}
									</div>
								</div>
							{/if}
						</div>
					{/if}

					{#if studySyncSupported && libraryRecent.length && !catalogQuery.trim()}
						<section class="lv-section" aria-labelledby="recent-heading">
							<div class="lv-section__head">
								<div class="lv-section__title flex min-w-0 items-baseline gap-2">
									<h2 id="recent-heading">{tr('Continue studying', 'Continue estudando')}</h2>
									<span class="lv-count">{libraryRecent.length} {libraryRecent.length === 1 ? tr('recent', 'recente') : tr('recent', 'recentes')}</span>
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
													<span class="lv-resume-card__meta">{entry.item.completed ? tr('Completed', 'Concluído') : tr(`Stopped at ${formatDuration(entry.item.positionSeconds)}`, `Parou em ${formatDuration(entry.item.positionSeconds)}`)}</span>
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
							<label for="import-path">{tr('Course folder path on server', 'Caminho da pasta do curso no servidor')}</label>
							<input id="import-path" name="path" bind:value={importPath} placeholder={tr('E.g., /courses/My Course', 'Ex.: C:\\cursos\\Meu curso')} autocomplete="off" spellcheck={false} required />
							<div class="lv-import__actions">
								<button class="lv-btn lv-btn--primary" disabled={importing} aria-busy={importing}>{importing ? tr('Importing…', 'Importando…') : tr('Import', 'Importar')}</button>
								<button type="button" class="lv-btn lv-btn--ghost" onclick={() => (importFormOpen = false)}>{tr('Cancel', 'Cancelar')}</button>
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
									<p class="lv-import-success__title">{tr('Course imported successfully:', 'Curso importado com sucesso:')} <strong>{importedSuccess.title}</strong></p>
									<p class="lv-import-success__meta">
										{importedSuccess.lessonCount} {importedSuccess.lessonCount === 1 ? tr('lesson detected', 'aula detectada') : tr('lessons detected', 'aulas detectadas')}.
									</p>
								</div>
							</div>
							<div class="lv-import-success__actions">
								{#if importedSuccess.firstLessonPath}
									<a class="lvui-btn lvui-btn-primary lvui-btn-sm" href={href('course', importedSuccess.courseId, importedSuccess.firstLessonPath)}>
										<svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true"><polygon points="6 4 20 12 6 20 6 4"/></svg>
										{tr('Start studying', 'Começar a estudar')}
									</a>
								{:else}
									<a class="lvui-btn lvui-btn-primary lvui-btn-sm" href={href('course', importedSuccess.courseId)}>
										{tr('View course', 'Ver curso')}
									</a>
								{/if}
								<button type="button" class="lvui-btn lvui-btn-ghost lvui-btn-sm" onclick={() => (importedSuccess = null)} aria-label={tr('Close notice', 'Fechar aviso')}>
									{tr('Close', 'Fechar')}
								</button>
							</div>
						</div>
					{/if}
					{#if importError}<p class="lv-alert" role="alert">{importError}</p>{/if}
					{#if canManage}<input class="visually-hidden course-cover-input" type="file" accept={COVER_ACCEPT} bind:this={coverInput} onchange={pickCover} tabindex="-1" aria-hidden="true" />{/if}
					{#if coverError}<p class="lv-alert" role="alert">{coverError}</p>{/if}
					{#if coverMessage}<p class="lv-notice" role="status" aria-live="polite">{coverMessage}</p>{/if}

					<section class="lv-section" aria-labelledby="courses-heading">
						<div class="lv-section__head lv-section__head--courses">
							<div class="lv-section__title flex min-w-0 items-baseline gap-2">
								<h2 id="courses-heading">{tr('All courses', 'Todos os cursos')}</h2>
								{#if catalog && catalog.nodes.length}<span class="lv-count" aria-live="polite">{tr(`${filteredNodes.length} of ${catalog.nodes.length}`, `${filteredNodes.length} de ${catalog.nodes.length}`)}</span>{/if}
							</div>
							<div class="lv-section__tools">
								<div class="lv-view-switch lvui-join" role="group" aria-label={tr('Course view', 'Visualização dos cursos')}>
									{#each LIBRARY_VIEWS as view}
										<button type="button" class={`lvui-join-item lvui-btn lv-view-tab${libraryView === view ? ' lvui-btn-active' : ''}`} aria-pressed={libraryView === view} onclick={() => changeLibraryView(view)}>
											{#if view === 'covers'}<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="4" width="7" height="7" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.7"/><rect x="13" y="4" width="7" height="7" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.7"/><rect x="4" y="13" width="7" height="7" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.7"/><rect x="13" y="13" width="7" height="7" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.7"/></svg>
											{:else if view === 'list'}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6h16M4 12h16M4 18h16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>
											{:else if view === 'compact'}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>
											{:else}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 5v14M9 5v14M13 5v14M17 5v14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>{/if}
											<span>{libraryViewLabel(view)}</span>
										</button>
									{/each}
								</div>
							</div>
						</div>
						{#if catalog && catalog.nodes.length}
							{#if filteredNodes.length}
								<ul class="lv-course-grid" data-view={libraryView} aria-label={tr(`Courses in ${libraryViewLabel(libraryView)} view`, `Cursos na visualização ${libraryViewLabel(libraryView)}`)}>
									{#each filteredNodes as courseItem (courseItem.courseId ?? courseItem.path)}
										<li class:is-menu-open={coverMenuId === courseIdOf(courseItem)}>
											<CourseCard course={courseItem} profile={user.userId} mediaUrl={(file, courseId) => transport.mediaUrl(file, courseId)} manualCoverUrl={transport.coverUrl(courseIdOf(courseItem), courseItem.coverVersion ?? null)} {canManage} inProgress={locatedRecent.some((entry) => entry.location.courseId === courseIdOf(courseItem) && !entry.item.completed)} favorite={favoriteCourseIds.has(courseIdOf(courseItem))} onToggleFavorite={studySyncSupported ? handleToggleCourseFavorite : undefined} menuOpen={coverMenuId === courseIdOf(courseItem)} onMenuOpen={(open) => toggleCoverMenu(courseItem, open)} onChangeCover={requestCourseCover} onRemoveCover={removeCover} />
										</li>
									{/each}
								</ul>
							{:else}
								<div class="lv-empty-state">
									<p class="lv-empty__title" role="status">{tr('No results found', 'Nenhum resultado encontrado')}</p>
									{#if !favoritesOnly && progressFilter === 'all' && sourceFolderFilter === 'all' && catalogQuery.trim()}
										<p class="lv-lede">{tr(`No course or lesson matches “${catalogQuery.trim()}”.`, `Nenhum curso ou aula corresponde a “${catalogQuery.trim()}”.`)}</p>
										<button class="lv-btn" type="button" onclick={() => (catalogQuery = '')}>{tr('Clear search', 'Limpar busca')}</button>
									{:else}
										<p class="lv-lede">{tr(`No course matches the active filters${catalogQuery.trim() ? ` or search “${catalogQuery.trim()}”` : ''}.`, `Nenhum curso corresponde aos filtros ativos${catalogQuery.trim() ? ` ou à busca “${catalogQuery.trim()}”` : ''}.`)}</p>
										<button class="lv-btn" type="button" onclick={() => { catalogQuery = ''; progressFilter = 'all'; sourceFolderFilter = 'all'; favoritesOnly = false; }}>{tr('Clear filters and search', 'Limpar filtros e busca')}</button>
									{/if}
								</div>
							{/if}
						{:else if canManage}
							<div class="lv-empty-state">
								<p class="lv-empty__title">{tr('Your library starts here', 'Sua biblioteca começa aqui')}</p>
								<p class="lv-lede">{tr('Use “Import course” above to add a folder. Files remain read-only.', 'Use “Importar curso” acima para adicionar uma pasta. Os arquivos permanecem somente leitura.')}</p>
							</div>
						{:else}
							<div class="lv-empty-state"><p class="lv-empty__title">{tr('No courses available', 'Nenhum curso disponível')}</p><p class="lv-lede">{tr('Ask an administrator to import a course.', 'Peça ao administrador para importar um curso.')}</p></div>
						{/if}

						{#if catalogQuery.trim() && matchingNotes.length > 0}
							<div class="lv-matching-notes" role="region" aria-label={tr('Matching notes', 'Anotações correspondentes')}>
								<div class="lv-panel__head">
									<h2>{tr('Notes found', 'Anotações encontradas')}</h2>
									<span class="lv-count">{matchingNotes.length} {matchingNotes.length === 1 ? tr('note', 'anotação') : tr('notes', 'anotações')}</span>
								</div>
								<div class="lv-matching-notes__grid">
									{#each matchingNotes as note (note.noteId)}
										{@const location = findLessonLocation(catalog, note.mediaId)}
										{@const targetHref = location?.courseId && location?.lesson.path ? href('course', location.courseId, location.lesson.path, note.positionSeconds) : null}
										{#if targetHref}
											<a class="lv-note-match-card" href={targetHref}>
												<div class="lv-note-match-card__header">
													<span class="lv-note-match-course">{location?.courseTitle ?? tr('Course', 'Curso')}</span>
													<span class="lv-note-match-time">{formatDuration(note.positionSeconds)}</span>
												</div>
												<p class="lv-note-match-lesson">{location?.lesson.title ?? note.relativePath ?? tr('Lesson', 'Aula')}</p>
												<p class="lv-note-match-content">{note.content}</p>
											</a>
										{:else}
											<div class="lv-note-match-card lv-note-match-card--detached">
												<div class="lv-note-match-card__header">
													<span class="lv-note-match-course">{location?.courseTitle ?? tr('Course', 'Curso')}</span>
													<span class="lv-note-match-time">{formatDuration(note.positionSeconds)}</span>
												</div>
												<p class="lv-note-match-lesson">{location?.lesson.title ?? note.relativePath ?? tr('Lesson', 'Aula')}</p>
												<p class="lv-note-match-content">{note.content}</p>
											</div>
										{/if}
									{/each}
								</div>
							</div>
						{/if}
					</section>

					{#if catalog && catalog.looseFiles.length}<div class="lv-section"><LooseFiles files={catalog.looseFiles} heading={tr('Loose files', 'Arquivos soltos')} /></div>{/if}
				</div>
			{:else if route === 'statistics'}
				<Statistics {catalog} {recent} />
			{:else if route === 'notes'}
				<AllNotes {catalog} />
			{:else if route === 'settings'}
				<div class="lv-page lv-settings">
					<header class="lv-page__head">
						<div>
							<h1>{tr('Settings', 'Ajustes')}</h1>
						<p class="lv-lede">{tr('Appearance, language, focus pace, and preferences on this device.', 'Aparência, idioma, ritmo de foco e preferências deste dispositivo.')}</p>
					</div>
				</header>
					<div class="lv-settings__grid">
					<section class="lv-panel lv-settings__panel">
						<div class="lv-panel__head"><h2>{tr('Theme & Language', 'Tema e Idioma')}</h2></div>
						<fieldset class="theme-options">
							<legend class="visually-hidden">{tr('Theme', 'Tema')}</legend>
							<label><input type="radio" name="theme" value="system" checked={themeMode === 'system'} onchange={() => changeTheme('system')} />{tr('System', 'Sistema')} <span>{tr('Follows system preference.', 'Acompanha a preferência do sistema.')}</span></label>
							<label><input type="radio" name="theme" value="light" checked={themeMode === 'light'} onchange={() => changeTheme('light')} />{tr('Light', 'Claro')} <span>{tr('Always uses light theme.', 'Usa sempre o tema claro.')}</span></label>
							<label><input type="radio" name="theme" value="dark" checked={themeMode === 'dark'} onchange={() => changeTheme('dark')} />{tr('Dark', 'Escuro')} <span>{tr('Always uses dark theme.', 'Usa sempre o tema escuro.')}</span></label>
						</fieldset>
						<fieldset class="theme-options" style="margin-top: 0.75rem; padding-top: 0.75rem; border-top: 1px solid var(--border);">
							<legend class="visually-hidden">{tr('Language', 'Idioma')}</legend>
							{#each LOCALES as loc (loc)}
								<label>
									<input type="radio" name="locale" value={loc} checked={locale === loc} onchange={() => changeLocale(loc)} />
									{LOCALE_LABELS[loc]}
									<span>{loc === 'en' ? tr('Default interface language.', 'Idioma padrão da interface.') : tr('Interface in Brazilian Portuguese.', 'Interface em português do Brasil.')}</span>
								</label>
							{/each}
						</fieldset>
					</section>
					<section class="lv-panel lv-settings__panel" aria-labelledby="profile-heading">
						<div class="lv-panel__head"><h2 id="profile-heading">{tr('Profile', 'Perfil')}</h2><span class="lv-count">{tr('Photo and identity', 'Foto e identidade')}</span></div>
						<div class="lv-settings__body">
							<UserProfile {user} {avatarVersion} onAvatarChange={handleAvatarChanged} />
						</div>
					</section>
					<section class="lv-panel lv-settings__panel" aria-labelledby="timer-heading">
						<div class="lv-panel__head"><h2 id="timer-heading">Pomodoro</h2><span class="lv-count">{tr('Focus and break timers', 'Tempo de foco e pausas')}</span></div>
						<div class="lv-settings__body">
							<PomodoroSettings />
						</div>
					</section>
					<section class="lv-panel lv-settings__panel" aria-labelledby="account-heading">
						<div class="lv-panel__head"><h2 id="account-heading">{tr('Account', 'Conta')}</h2><span class="lv-count">{user.isAdmin ? tr('Administrator', 'Administrador') : tr('Student', 'Estudante')}</span></div>
						<div class="lv-settings__body">
							<p><strong>{user.username}</strong></p>
							<form class="credentials-form" onsubmit={credentials}>
								<fieldset class="auth-fields" disabled={authBusy}>
									<legend>{tr('Access credentials', 'Credenciais de acesso')}</legend>
									<PasswordField id="settings-current-password" label={tr('Current password', 'Senha atual')} autocomplete="current-password" bind:value={currentPassword} />
									<label class="form-field" for="settings-new-username">{tr('New username', 'Novo usuário')}<input id="settings-new-username" name="username" autocomplete="username" autocapitalize="none" spellcheck={false} bind:value={username} required /></label>
									<PasswordField id="settings-new-password" label={tr('New password', 'Nova senha')} autocomplete="new-password" bind:value={password} minLength={12} hint={tr('Use at least 12 characters.', 'Use ao menos 12 caracteres.')} />
									{#if accountError}<p class="error" role="alert">{accountError}</p>{/if}
									{#if accountStatus}<p class="lv-notice" role="status">{accountStatus}</p>{/if}
									<button class="lv-btn lv-btn--primary" type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? tr('Saving…', 'Salvando…') : tr('Update credentials', 'Atualizar credenciais')}</button>
								</fieldset>
							</form>
							{#if transportKind === 'http'}<button class="lv-btn lv-btn--ghost" type="button" onclick={logout}>{tr('Sign out', 'Encerrar sessão')}</button>{:else}<p class="lv-notice">{tr('Local library on this device.', 'Biblioteca local neste dispositivo.')}</p>{/if}
						</div>
					</section>
					<section class="lv-panel lv-settings__panel" aria-labelledby="update-heading">
						<div class="lv-panel__head"><h2 id="update-heading">{tr('Update', 'Atualização')}</h2>{#if updateStatus}<span class="lv-count">{tr('Version', 'Versão')} {updateStatus.current}</span>{/if}</div>
						<div class="lv-settings__body">
							<button class="lv-btn" type="button" onclick={() => checkUpdate(true)} disabled={updateChecking} aria-busy={updateChecking}>{updateChecking ? tr('Checking…', 'Verificando…') : tr('Check for updates', 'Verificar atualização')}</button>
							{#if updateStatus}
								{@const outcome = updateOutcome(updateStatus)}
								{@const releaseUrl = updateReleaseUrl(updateStatus)}
								{#if outcome !== 'error' || updateManual}<p class="lv-notice" role="status">{updateMessage(updateStatus)}</p>{/if}
								{#if outcome === 'available' && releaseUrl}<p class="lv-notice"><a href={releaseUrl} target="_blank" rel="noopener noreferrer">{tr('View release', 'Ver a release')}</a></p>{/if}
								{#if outcome === 'available' && transportKind === 'desktop'}
									<div class="lv-update-actions" style="margin-top: 0.5rem;">
										<button class="lv-btn lv-btn--primary" type="button" disabled={updateInstalling} onclick={installUpdate} aria-busy={updateInstalling}>
											{updateInstalling ? tr('Installing update…', 'Instalando atualização…') : tr('Update and restart', 'Atualizar e reiniciar')}
										</button>
										{#if updateInstallError}<p class="error" role="alert" style="margin-top: 0.25rem;">{updateInstallError}</p>{/if}
									</div>
								{/if}
								{#if outcome === 'available' && updateStatus.changelog}
									<details class="lv-update-changelog">
										<summary class="lv-update-changelog__summary">{tr('What’s new in this version (Changelog)', 'Novidades desta versão (Changelog)')}</summary>
										<pre class="lv-update-changelog__content">{updateStatus.changelog}</pre>
									</details>
								{/if}
							{/if}
							{#if updateHistoryList.length > 0}
								<details class="lv-update-history">
									<summary class="lv-update-history__summary">{tr(`Update history (${updateHistoryList.length})`, `Histórico de atualizações (${updateHistoryList.length})`)}</summary>
									<ul class="lv-update-history__list">
										{#each updateHistoryList as entry}
											<li class="lv-update-history__item">
												<div class="lv-update-history__header">
													<span class="lv-update-history__status lv-update-history__status--{entry.status}">{entry.status === 'success' ? tr('Success', 'Sucesso') : entry.status === 'rollback' ? tr('Rollback', 'Reversão') : entry.status}</span>
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
						<AdminUsers {transport} courses={catalog?.nodes ?? []} />
						<section class="lv-panel lv-settings__panel" aria-labelledby="backup-heading">
							<div class="lv-panel__head"><h2 id="backup-heading">{tr('Database Backup', 'Backup do Banco de Dados')}</h2></div>
							<div class="lv-settings__body">
								<p class="lv-notice">{tr('Generate and download a consistent transactional copy of the SQLite database (verified with structural integrity).', 'Gere e baixe uma cópia transacional consistente do banco SQLite (verificada com integridade estrutural).')}</p>
								<a class="lv-btn lv-btn--outline" href="/api/admin/backup" download>{tr('Download backup now (.sqlite)', 'Baixar backup agora (.sqlite)')}</a>
							</div>
						</section>
					{/if}
					</div>
				</div>
			{:else if isCourseRoute}
				{#if !course}
					<div class="lv-page lv-page--narrow">
						<nav class="lv-breadcrumb" aria-label={tr('Breadcrumb', 'Caminho')}><a href={href('home')}>{tr('Library', 'Biblioteca')}</a><span aria-hidden="true">/</span><span>{tr('Course', 'Curso')}</span></nav>
						<div class="lv-empty-state"><p class="lv-empty__title">{tr('Course not found', 'Curso não encontrado')}</p><p class="lv-lede">{tr('It may have been removed from the current catalog.', 'Ele pode ter sido removido do catálogo atual.')}</p><a class="lv-btn" href={href('home')}>{tr('Back to library', 'Voltar à biblioteca')}</a></div>
					</div>
				{:else if isOverview}
					{@const counts = countCatalogNode(course)}
					{@const missingVideos = countMissingVideos(course)}
					{@const meta = formatCourseMeta(counts)}
					{@const cover = courseCover(course.title)}
					{@const materials = collectMaterials(course)}
					<div class="lv-page lv-course-overview">
						<nav class="lv-breadcrumb" aria-label={tr('Course path', 'Caminho do curso')}><a href={href('home')}>{tr('Library', 'Biblioteca')}</a><span aria-hidden="true">/</span><span>{course.title}</span></nav>
						<header class="lv-course-hero">
							<div class="lv-course-hero__cover">
								<CourseCover course={course} profile={user.userId} mediaUrl={(file, courseId) => transport.mediaUrl(file, courseId)} manualCoverUrl={transport.coverUrl(courseIdOf(course), course.coverVersion ?? null)} size="hero" />
							</div>
							<div class="lv-course-hero__main">
								<h1>{course.title}</h1>
								{#if meta}<p class="lv-course-hero__meta">{meta}</p>{/if}
							</div>
							{#if courseRecent}
								<p class="lv-course-hero__resume">{tr('Last lesson:', 'Última aula:')} {courseRecent.location.lesson.title} · {courseRecent.item.completed ? tr('completed', 'concluída') : tr(`stopped at ${formatDuration(courseRecent.item.positionSeconds)}`, `parou em ${formatDuration(courseRecent.item.positionSeconds)}`)}</p>
							{/if}
							<div class="lv-course-hero__actions">
								{#if activePath}
									<a class="lv-btn lv-btn--primary" href={href('course', courseIdOf(course), courseRecent ? courseRecent.location.lesson.path : activePath, courseRecent && !courseRecent.item.completed ? courseRecent.item.positionSeconds : null)}>{courseRecent ? tr('Continue studying', 'Continuar estudando') : tr('Start course', 'Começar curso')}</a>
								{:else}
									<span class="lv-btn" aria-disabled="true">{tr('No lessons available', 'Nenhuma aula disponível')}</span>
								{/if}
								{#if studySyncSupported}
									{@const isCourseFav = favoriteCourseIds.has(courseIdOf(course))}
									<button class="lv-btn" type="button" aria-pressed={isCourseFav} onclick={() => course && handleToggleCourseFavorite(course)}>
										{isCourseFav ? tr('★ Favorited', '★ Favoritado') : tr('☆ Favorite course', '☆ Favoritar curso')}
									</button>
								{/if}
								{#if canManage}
									<div class="course-menu-wrap">
										<button class="course-menu-toggle lv-btn" type="button" aria-expanded={courseMenuOpen} onclick={() => { courseMenuOpen = !courseMenuOpen; removeConfirm = false; }}>{tr('Manage course', 'Gerenciar curso')} <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg></button>
										{#if courseMenuOpen}
											<div class="course-menu" role="group" aria-label={tr(`Manage ${course.title}`, `Gerenciar ${course.title}`)}>
												{#if removeConfirm}
													<p class="course-menu-question" role="alert">{tr(`Remove “${course.title}” from the library?`, `Remover “${course.title}” da biblioteca?`)}</p>
													<p class="course-menu-hint">{tr('Original files will not be deleted.', 'Os arquivos não serão apagados.')}</p>
													<div class="course-menu-actions">
														<button class="text-button danger" type="button" onclick={removeCourse} disabled={removing} aria-busy={removing}>{removing ? tr('Removing…', 'Removendo…') : tr('Remove course', 'Remover curso')}</button>
														<button class="text-button" type="button" onclick={() => (removeConfirm = false)} disabled={removing}>{tr('Cancel', 'Cancelar')}</button>
													</div>
												{:else}
													<button class="text-button" type="button" onclick={resetOrder} disabled={orderBusy} aria-busy={orderBusy}>{orderBusy ? tr('Restoring…', 'Restaurando…') : tr('Restore order', 'Restaurar ordem')}</button>
													<button class="text-button danger" type="button" onclick={() => (removeConfirm = true)}>{tr('Remove course', 'Remover curso')}</button>
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
								<div class="lv-panel__head"><h2 id="course-content-heading">{tr('Course content', 'Conteúdo do curso')}</h2><span class="lv-count">{formatLessonCounts(counts)}</span></div>
								<div class="lv-tabs" role="tablist" aria-label={tr('Course content', 'Conteúdo do curso')}>
									<button class="lv-tab" type="button" role="tab" id="course-tab-content" aria-controls="course-panel-content" aria-selected={courseTab === 'content'} tabindex={courseTab === 'content' ? 0 : -1} class:is-active={courseTab === 'content'} onclick={() => (courseTab = 'content')} onkeydown={courseTabKey}>{tr('Content', 'Conteúdo')}</button>
									<button class="lv-tab" type="button" role="tab" id="course-tab-materials" aria-controls="course-panel-materials" aria-selected={courseTab === 'materials'} tabindex={courseTab === 'materials' ? 0 : -1} class:is-active={courseTab === 'materials'} onclick={() => (courseTab = 'materials')} onkeydown={courseTabKey}>{tr('Materials', 'Materiais')} {#if materials.length}<span class="lv-tab__count">{materials.length}</span>{/if}</button>
								</div>
								{#if courseTab === 'content'}
									<div id="course-panel-content" role="tabpanel" aria-labelledby="course-tab-content">
										{#if course.children.length}
											<CatalogTree nodes={course.children} courseId={courseIdOf(course)} activePath={activePath} showFiles={false} {completedMediaIds} onToggleLessonCompleted={handleToggleLessonCompleted} {favoriteMediaIds} onToggleLessonFavorite={studySyncSupported ? handleToggleLessonFavorite : undefined} />
										{:else}
											<p class="lv-empty">{tr('This course has not been indexed yet.', 'Este curso ainda não foi indexado.')}</p>
										{/if}
									</div>
								{:else}
									<div id="course-panel-materials" role="tabpanel" aria-labelledby="course-tab-materials">
										{#if materials.length}
											<LooseFiles files={materials} heading={tr('Course materials', 'Materiais do curso')} courseId={courseIdOf(course)} />
										{:else}
											<p class="lv-empty">{tr('No support materials in this course.', 'Nenhum material de apoio neste curso.')}</p>
										{/if}
									</div>
								{/if}
							</section>
							<aside class="lv-course-side">
								<section class="lv-panel lv-meta" aria-labelledby="course-meta-heading">
									<div class="lv-panel__head"><h2 id="course-meta-heading">{tr('Course details', 'Ficha do curso')}</h2></div>
									<dl>
										<div><dt>{tr('Modules', 'Módulos')}</dt><dd>{counts.modules}</dd></div>
										<div><dt>{tr('Lessons', 'Aulas')}</dt><dd>{counts.lessons}{#if counts.missingLessons > 0}<span class="lv-meta__missing">{formatCount(counts.missingLessons, tr('missing', 'ausente'), tr('missing', 'ausentes'))}</span>{/if}</dd></div>
										<div><dt>{tr('Videos', 'Vídeos')}</dt><dd>{formatCount(counts.videos - missingVideos, tr('available', 'disponível'), tr('available', 'disponíveis'))}{#if missingVideos > 0}<span class="lv-meta__missing">{formatCount(missingVideos, tr('missing', 'ausente'), tr('missing', 'ausentes'))}</span>{/if}</dd></div>
										<div><dt>{tr('Materials', 'Materiais')}</dt><dd>{counts.materials}</dd></div>
										<div><dt>{tr('Code', 'Código')}</dt><dd class="lv-meta__code">{cover.code}</dd></div>
									</dl>
								</section>
							{#if courseRecent}
								<section class="lv-panel lv-resume-panel" aria-labelledby="course-resume-heading">
									<div class="lv-panel__head"><h2 id="course-resume-heading">{tr('Pick up where you left off', 'Continue de onde parou')}</h2></div>
									<div class="lv-resume-panel__body">
										<p class="lv-resume-panel__lesson">{courseRecent.location.lesson.title}</p>
										<p class="lv-resume-panel__meta">{formatLocationContext(courseRecent.location.courseTitle, courseRecent.location.moduleTitle)}{#if !courseRecent.item.completed}{' · '}{formatDuration(courseRecent.item.positionSeconds)}{/if}</p>
										<a class="lv-btn lv-btn--primary" href={href('course', courseIdOf(course), courseRecent.location.lesson.path, courseRecent.item.completed ? null : courseRecent.item.positionSeconds)}>{courseRecent.item.completed ? tr('Review lesson', 'Revisar aula') : tr('Resume lesson', 'Retomar aula')}</a>
									</div>
								</section>
							{/if}
							</aside>
						</div>
					</div>
				{:else}
					<div class="lv-page lv-player-page">
						<div class="lv-player-top">
							<nav class="lv-breadcrumb" aria-label={tr('Course path', 'Caminho do curso')}><a href={href('home')}>{tr('Library', 'Biblioteca')}</a><span aria-hidden="true">/</span><a href={href('course', courseIdOf(course))} aria-label={tr(`Back to course ${course.title}`, `Voltar ao curso ${course.title}`)}><span class="lv-breadcrumb__back-label" aria-hidden="true">{tr('Back to course', 'Voltar ao curso')}</span>{course.title}</a><span aria-hidden="true">/</span><span>{lesson?.title ?? tr('Lesson', 'Aula')}</span></nav>
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
											<a class="lv-btn lv-btn--ghost" href={href('course', courseIdOf(course))}>{tr('Overview', 'Visão geral')}</a>
											{#if studySyncSupported && currentLessonMediaId}
												{@const isLessonFav = favoriteMediaIds.has(currentLessonMediaId)}
												<button class="lv-btn" type="button" aria-pressed={isLessonFav} onclick={() => currentLessonMediaId && handleToggleLessonMediaFavorite(currentLessonMediaId)}>
													{isLessonFav ? tr('★ Favorite lesson', '★ Aula favorita') : tr('☆ Favorite lesson', '☆ Favoritar aula')}
												</button>
											{/if}
											{#if neighbors.previous}<a class="lv-btn" href={href('course', courseIdOf(course), neighbors.previous.path)}>{tr('Previous', 'Anterior')}</a>{/if}
											{#if neighbors.next}<a class="lv-btn" href={href('course', courseIdOf(course), neighbors.next.path)}>{tr('Next', 'Próxima')}</a>{/if}
											<button class="lv-btn" type="button" aria-pressed={focusMode} onclick={() => (focusMode = !focusMode)}>{focusMode ? tr('Exit focus', 'Sair do foco') : tr('Focus mode', 'Modo foco')}</button>
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
									{#if activeContainer && (activeContainer.files.length || activeContainer.looseFiles.length)}<div class="lv-player-materials"><LooseFiles files={[...activeContainer.files, ...activeContainer.looseFiles]} heading={tr('Module materials', 'Materiais do módulo')} courseId={courseIdOf(course)} /></div>{/if}
									{#if course.looseFiles.length}<div class="lv-player-materials"><LooseFiles files={course.looseFiles} heading={tr('Course loose files', 'Arquivos soltos do curso')} courseId={courseIdOf(course)} /></div>{/if}
								{:else}
									<div class="lv-empty-state"><p class="lv-empty__title">{tr('This course has no available lessons', 'Este curso não tem nenhuma aula disponível')}</p><a class="lv-btn" href={href('course', courseIdOf(course))}>{tr('Back to overview', 'Voltar à visão geral')}</a></div>
								{/if}
							</div>
							<aside class="lv-curriculum" aria-label={tr('Course lessons', 'Aulas do curso')}>
								<div class="lv-panel__head"><h2>{tr('Course lessons', 'Aulas do curso')}</h2><span class="lv-count">{formatLessonCounts(countCatalogNode(course))}</span></div>
								<div class="lv-curriculum__body" data-playlist-body>
									{#if course.children.length}<CatalogTree nodes={course.children} courseId={courseIdOf(course)} activePath={activePath} showFiles={false} {completedMediaIds} onToggleLessonCompleted={handleToggleLessonCompleted} {favoriteMediaIds} onToggleLessonFavorite={studySyncSupported ? handleToggleLessonFavorite : undefined} />{:else}<p class="lv-empty">{tr('This course has not been indexed yet.', 'Este curso ainda não foi indexado.')}</p>{/if}
								</div>
							</aside>
						</div>
					</div>
				{/if}
			{:else}
				<div class="lv-page lv-page--narrow">
					{#if !lesson}<div class="lv-empty-state"><p class="lv-empty__title">{tr('Lesson not found', 'Aula não encontrada')}</p><a class="lv-btn" href={href('home')}>{tr('Back to library', 'Voltar à biblioteca')}</a></div>{:else}
						<a class="lv-back" href={href('home')}>{tr('← Library', '← Biblioteca')}</a>
						<h1 class="lv-standalone-title">{lesson.title}</h1>
						<Player {lesson} courseId={courseId ?? ''} initialPosition={positionSeconds} {mediaError} onMediaError={mediaFailed} {studyEnabled} preferences={playerPreferences}
							onPreferencesChange={savePlayerPreferences} autoNext={playerPreferences.autoNext}
							hasPreviousLesson={neighbors.previous !== null}
							hasNextLesson={neighbors.next !== null}
							onPreviousLesson={() => { if (neighbors.previous) location.hash = href('player', courseId ?? '', neighbors.previous.path); }}
							onNextLesson={() => { if (neighbors.next) location.hash = href('player', courseId ?? '', neighbors.next.path); }}
							onAutoNext={autoAdvance}
							onAutoNextChange={(enabled) => savePlayerPreferences({ ...playerPreferences, autoNext: enabled })} />
						{#if neighbors.previous || neighbors.next}<nav class="lv-lesson-nav" aria-label={tr('Neighboring lessons', 'Aulas vizinhas')}>{#if neighbors.previous}<a href={href('player', courseId ?? '', neighbors.previous.path)}>← {neighbors.previous.title}</a>{/if}{#if neighbors.next}<a href={href('player', courseId ?? '', neighbors.next.path)}>{neighbors.next.title} →</a>{/if}</nav>{/if}
					{/if}
				</div>
			{/if}
		</main>
	</div>
{/if}
