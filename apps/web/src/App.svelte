<script lang="ts">
	import { onMount } from 'svelte';
	import LooseFiles from './LooseFiles.svelte';
	import CatalogTree from './CatalogTree.svelte';
	import CourseCard from './CourseCard.svelte';
	import Player from './Player.svelte';
	import Pomodoro from './Pomodoro.svelte';
	import ImportPreview from './ImportPreview.svelte';
	import ImportSteps from './ImportSteps.svelte';
	import { filterCatalog } from './search';
	import { countCatalogNode, formatLessonCounts, formatLocationContext } from './catalog';
	import PasswordField from './PasswordField.svelte';
	import CredentialsForm from './CredentialsForm.svelte';
	import NotesIndex from './NotesIndex.svelte';
	import EmptyState from './EmptyState.svelte';
	import { defaultLessonPath, lessonContainer, neighboringLessons } from './navigation';
	import { destinationAfterLogin, findCourse, findLesson, findLessonLocation, href, locationFromHash, type Route } from './router';
	import { transport, transportKind, COVER_ACCEPT, MAX_COVER_BYTES, courseIdOf, STUDY_NOT_AVAILABLE, type Catalog, type CatalogNode, type ImportPreview as ImportPreviewData, type Lesson, type StudyAnalytics, type StudyProgress, type UpdateStatus, type User } from './transport';
	import { updateOutcome, updateMessage, updateReleaseUrl } from './update';
	import { defaultRoute, resolveSession } from './session';
	import { formatDuration, studySyncSupported } from './player';
	import { courseCover } from './cover';
	import { createDefaultPlayerPreferences, playerPreferencesScope, readPlayerPreferences, writePlayerPreferences, type PlayerPreferences } from './player-preferences';
	import { applyTheme, persistThemeMode, readThemeMode, resolveTheme, type ThemeMode } from './theme';
	import PaletteSettings from './PaletteSettings.svelte';
	import { applyPalette, readPalettePreferences, persistPalettePreferences, resolvePalette, type PalettePreferences } from './palettes';
	import { graphiteScreens } from './graphite/layout';

	let accountDetails: HTMLDetailsElement | null = null;
	let themeMode: ThemeMode = readThemeMode(safeLocalStorage());
	let systemDark = window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;
	let resolvedTheme = resolveTheme(themeMode, systemDark);
	let palettePreferences = readPalettePreferences(safeLocalStorage());
	let themePersistenceMessage = '';
	let settingsSection: 'theme' | 'account' | 'updates' = 'theme';
	// O desktop não tem tela de login: um endereço de login ou desconhecido cai na tela
	// inicial em vez de abrir um formulário que não consegue autenticar.
	const fallbackRoute = defaultRoute(transportKind);
	const initialLocation = locationFromHash(location.hash, fallbackRoute);
	let route: Route = initialLocation.route;
	let courseId: string | null = initialLocation.courseId;
	let activeBoard = '01';
	$: activeBoard = route === 'home' ? graphiteScreens.library.board : route === 'course' && lessonPath === null ? graphiteScreens.course.board : route === 'settings' ? graphiteScreens.settings.board : route === 'analytics' ? graphiteScreens.analytics.board : route === 'notes' ? graphiteScreens.notes.board : graphiteScreens.player.board;
	let lessonPath: string | null = initialLocation.lessonPath;
	let user: User | null = null;
	let accountUser: User = { userId: '', username: '', role: '', isAdmin: false, mustChangeCredentials: false };
	$: accountUser = user ?? accountUser;
	let catalog: Catalog | null = null;
	$: course = findCourse(catalog, courseId);
	let lesson: Lesson | null = null;
	let catalogQuery = '';
	let courseFilter: 'all' | 'in-progress' = 'all';
	let courseSort: 'recent' | 'library' = studySyncSupported ? 'recent' : 'library';
	let inProgressCourseIds = new Set<string>();
	let notesSearchQuery = '';
	let filteredNodes: CatalogNode[] = [];
	$: {
		const nodes = catalog
			? filterCatalog(catalog.nodes, catalogQuery).filter((node) => courseFilter === 'all' || inProgressCourseIds.has(courseIdOf(node)))
			: [];
		if (courseSort === 'recent') {
			const latestByCourse = new Map<string, number>();
			for (const item of recent) {
				if (item.updatedAt === null) continue;
				const location = findLessonLocation(catalog, item.mediaId);
				if (!location) continue;
				latestByCourse.set(location.courseId, Math.max(latestByCourse.get(location.courseId) ?? 0, item.updatedAt));
			}
			nodes.sort((left, right) => (latestByCourse.get(courseIdOf(right)) ?? 0) - (latestByCourse.get(courseIdOf(left)) ?? 0));
		}
		filteredNodes = nodes;
	}
	// A tela de estudo é a do curso unificado; o endereço legado `#/player/<curso>/<aula>`
	// abre a mesma tela, e `#/player/<caminho>` continua sendo o player de arquivo solto.
	$: watchRoute = (route === 'course' && lessonPath !== null) || (route === 'player' && courseId !== null);
	// A aula da URL só vale se ainda existir; um caminho removido por reindexação cai na
	// aula inicial. Sem aula na URL, o histórico recente decide, e só depois a primeira
	// aula disponível da ordem canônica.
	$: recentMediaIds = recent.map((item) => item.mediaId);
	$: requestedLessonPath = lessonPath !== null && findLesson(catalog, courseId, lessonPath) !== null ? lessonPath : null;
	$: activePath = requestedLessonPath ?? (course ? defaultLessonPath([course], recentMediaIds) : null);
	$: lesson = findLesson(catalog, courseId, activePath);
	$: activeContainer = course && activePath ? lessonContainer(course.children, activePath) : null;
	// Navegação sai da subárvore do curso aberto. Quando uma reindexação remove a aula
	// aberta, `neighboringLessons` devolve os dois lados nulos e os controles somem em vez
	// de apontar para uma aula que não existe mais.
	$: neighbors = course ? neighboringLessons([course], activePath) : { previous: null, next: null };
	let loading = true;
	let authBusy = false;
	let error = '';
	let username = '';
	let password = '';
	let rescanning = false;
	let rescanMessage = '';
	let importing = false;
	let importFormOpen = false;
	let importPhase: 'choose' | 'discover' | 'review' | 'import' | 'rescan' | 'rescan-error' = 'choose';
	let importPath = '';
	let importPreviewPath = '';
	let importError = '';
	let importPreview: ImportPreviewData | null = null;
	let previewingImport = false;
	let removing = false;
	let removeConfirm = false;
	let removeError = '';
	let courseMenuOpen = false;
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
	let organizingCatalog = false;
	let recent: StudyProgress[] = [];
	let recentLoading = false;
	let studyEnabled = studySyncSupported;
	$: inProgressCourseIds = new Set(
		recent
			.filter((item) => !item.completed)
			.map((item) => findLessonLocation(catalog, item.mediaId)?.courseId)
			.filter((id): id is string => Boolean(id))
	);
	function recentInProgressItems(): StudyProgress[] {
		const seenCourses = new Set<string>();
		const items: StudyProgress[] = [];
		for (const item of recent) {
			if (item.completed) continue;
			const location = findLessonLocation(catalog, item.mediaId);
			if (!location || seenCourses.has(location.courseId)) continue;
			seenCourses.add(location.courseId);
			items.push(item);
			if (items.length === 3) break;
		}
		return items;
	}
	// Estado mínimo da verificação de atualização: o último resultado, se uma checagem
	// está em curso e se a última foi manual (para só mostrar erro após uma ação do
	// usuário, e nunca durante a abertura da tela).
	let updateStatus: UpdateStatus | null = null;
	let updateChecking = false;
	let updateManual = false;
	let updateRequested = false;
	let studyAnalytics: StudyAnalytics | null = null;
	let studyAnalyticsError = '';
	let studyAnalyticsRequested = false;
	let studyAnalyticsUserId: string | null = null;
	let studyAnalyticsRequest = 0;
	$: if ((user?.userId ?? null) !== studyAnalyticsUserId) {
		studyAnalyticsUserId = user?.userId ?? null;
		studyAnalytics = null;
		studyAnalyticsError = '';
		studyAnalyticsRequested = false;
		studyAnalyticsRequest += 1;
	}
	$: if (route === 'analytics' && user && !studyAnalyticsRequested) {
		studyAnalyticsRequested = true;
		void loadStudyAnalytics();
	}

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
	applyPalette(document.documentElement, resolvePalette(palettePreferences), resolvedTheme);
	const scheme = window.matchMedia?.('(prefers-color-scheme: dark)');
	const updateSystemTheme = (event: MediaQueryListEvent) => {
		systemDark = event.matches;
		if (themeMode === 'system') {
			resolvedTheme = resolveTheme(themeMode, systemDark);
			applyTheme(document.documentElement, resolvedTheme);
			applyPalette(document.documentElement, resolvePalette(palettePreferences), resolvedTheme);
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
			organizingCatalog = false;
			removeConfirm = false;
			removeError = '';
			coverBusy = false;
			coverError = '';
			coverMessage = '';
			coverTargetId = null;
			coverMenuId = null;
			mediaError = '';
		};
		// Um clique fora dos menus (ou em outra parte da página) fecha as ações abertas; o
		// botão e o painel do próprio menu ficam de fora para que a ação escolhida funcione.
		const closeMenus = (event: PointerEvent) => {
			const target = event.target;
			if (!(target instanceof Element) || !target.closest('.course-menu, .course-menu-toggle')) {
				courseMenuOpen = false;
				removeConfirm = false;
			}
			if (accountDetails?.open && !(target instanceof Element && target.closest('.account-details'))) {
				accountDetails.open = false;
			}
			if (!coverMenuId) return;
			if (target instanceof Element && target.closest('.course-card-menu, .course-card-menu-toggle')) return;
			coverMenuId = null;
		};
		const closeMenusOnEscape = (event: KeyboardEvent) => {
			if (event.key !== 'Escape') return;
			coverMenuId = null;
			courseMenuOpen = false;
			removeConfirm = false;
			if (accountDetails?.open) {
				accountDetails.open = false;
				accountDetails.querySelector('summary')?.focus();
			}
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
		location.hash = href('player', courseId ?? '', neighbors.next.path);
	}
	function changeTheme(mode: ThemeMode) {
		themeMode = mode;
		resolvedTheme = resolveTheme(mode, systemDark);
		applyTheme(document.documentElement, resolvedTheme);
		applyPalette(document.documentElement, resolvePalette(palettePreferences), resolvedTheme);
		themePersistenceMessage = persistThemeMode(safeLocalStorage(), mode) ? '' : 'Tema aplicado nesta sessão; não foi possível salvar neste navegador.';
	}
	function changePalette(next: PalettePreferences): boolean {
		palettePreferences = next;
		applyPalette(document.documentElement, resolvePalette(next), resolvedTheme);
		return persistPalettePreferences(safeLocalStorage(), next);
	}
	function showSettingsSection(section: 'theme' | 'account' | 'updates', id: string) {
		settingsSection = section;
		const heading = document.getElementById(id);
		if (heading) {
			heading.scrollIntoView({ block: 'start' });
			heading.focus({ preventScroll: true });
		}
	}

	// A seção ativa acompanha a rolagem: o observador marca a seção no terço superior e é
	// desconectado quando o elemento sai do DOM.
	function trackSection(node: HTMLElement, id: 'theme' | 'account' | 'updates'): { destroy: () => void } {
		const observer = new IntersectionObserver((entries) => {
			for (const entry of entries) if (entry.isIntersecting) settingsSection = id;
		}, { rootMargin: '-30% 0px -60% 0px' });
		observer.observe(node);
		return { destroy: () => observer.disconnect() };
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

	async function logout() {
		await transport.logout();
		user = null;
		playerPreferences = createDefaultPlayerPreferences();
		catalog = null;
		recent = [];
		username = '';
		password = '';
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

	// A prévia percorre a pasta em modo somente leitura; só a confirmação registra o curso.
	async function submitImport(event: SubmitEvent) {
		event.preventDefault();
		if (previewingImport || importing) return;
		previewingImport = true;
		importError = '';
		importPhase = 'discover';
		try {
			const previewPath = importPath.trim();
			importPreview = await transport.previewImport(previewPath);
			importPreviewPath = previewPath;
			importFormOpen = false;
			importPhase = 'review';
		} catch (cause) {
			importError = cause instanceof Error ? cause.message : 'Não foi possível analisar a pasta.';
			importPhase = 'choose';
		} finally {
			previewingImport = false;
		}
	}

	function cancelImportPreview() {
		if (importing) return;
		importPreview = null;
		importPreviewPath = '';
		importFormOpen = true;
		importPhase = 'choose';
		importError = '';
	}

	function toggleImportForm() {
		if (importing || previewingImport) return;
		if (importPreview) {
			cancelImportPreview();
			return;
		}
		importFormOpen = !importFormOpen;
		importPhase = importFormOpen ? 'choose' : importPhase;
		if (!importFormOpen) importError = '';
	}

	// Registra o curso uma única vez; o catálogo só muda depois do rescan bem-sucedido.
	async function confirmImport() {
		if (!importPreview || importing) return;
		importing = true;
		importError = '';
		importPhase = 'import';
		try {
			await transport.importCoursePath(importPreviewPath);
			await finishImportRescan();
		} catch (cause) {
			importError = cause instanceof Error ? cause.message : 'Não foi possível importar o curso.';
			importPhase = 'review';
		} finally {
			importing = false;
		}
	}

	// Repete apenas rescan/catalog/recent: o curso já está registrado e nunca deve ser
	// importado de novo como se fosse nova tentativa.
	async function finishImportRescan(): Promise<void> {
		importPhase = 'rescan';
		try {
			await transport.rescan();
			syncCatalog(await transport.catalog());
			await loadRecent();
			importPath = '';
			importPreview = null;
			importPreviewPath = '';
			importFormOpen = false;
			importPhase = 'choose';
			importError = '';
		} catch (cause) {
			importError = cause instanceof Error ? cause.message : 'Não foi possível atualizar o catálogo.';
			importPhase = 'rescan-error';
		}
	}

	async function loadStudyAnalytics() {
		const requestedUserId = user?.userId ?? null;
		const request = ++studyAnalyticsRequest;
		try {
			const analytics = await transport.studyAnalytics();
			if (request !== studyAnalyticsRequest || requestedUserId !== (user?.userId ?? null)) return;
			studyAnalytics = analytics;
			studyAnalyticsError = '';
		} catch (cause) {
			if (request !== studyAnalyticsRequest || requestedUserId !== (user?.userId ?? null)) return;
			studyAnalytics = null;
			studyAnalyticsError = cause instanceof Error ? cause.message : 'Não foi possível carregar as estatísticas.';
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
			courseSort = 'library';
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
	async function saveCourseOrder(parentPath: string, childPaths: string[]) {
		const current = course;
		if (!current || !canManage || orderBusy) return;
		orderBusy = true;
		orderError = '';
		orderMessage = '';
		try {
			await transport.setCourseOrder(courseIdOf(current), parentPath, childPaths);
			syncCatalog(await transport.catalog());
			orderMessage = 'Ordem salva.';
		} catch (cause) {
			orderError = cause instanceof Error ? cause.message : 'Não foi possível salvar a ordem.';
		} finally {
			orderBusy = false;
		}
	}

	async function resetParentOrder(parentPath: string) {
		const current = course;
		if (!current || !canManage || orderBusy || !window.confirm('Restaurar a ordem automática desta lista?')) return;
		orderBusy = true;
		orderError = '';
		orderMessage = '';
		try {
			const removed = await transport.resetCourseOrder(courseIdOf(current), parentPath);
			syncCatalog(await transport.catalog());
			orderMessage = removed ? 'Ordem automática restaurada.' : 'Esta lista já usa a ordem automática.';
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
{#if loading}<div class="loading" aria-live="polite">Carregando seus cursos…</div>
{:else if !user}
	<main class="board login-board" aria-labelledby="login-heading">
		<section class="screen active">
			<div class="login">
				<div class="login-identity">
					<a class="brand" href={href('home')}>Learn<i>Vault</i></a>
					<div>
						<h1>Seu próximo capítulo começa de onde você parou.</h1>
						<p>Entre no seu ambiente de aprendizagem local.</p>
					</div>
					<span class="small">Seus cursos ficam no seu dispositivo ou servidor.</span>
				</div>
				<form class="login-card" onsubmit={login}>
					<h2 id="login-heading">Entrar</h2>
					<p class="small">Use as credenciais da sua instalação.</p>
					<fieldset class="auth-fields" disabled={authBusy}>
						<label class="form-field" for="login-username">Usuário<input class="field" id="login-username" name="username" autocomplete="username" autocapitalize="none" spellcheck={false} placeholder="Seu usuário" bind:value={username} required /></label>
						<PasswordField id="login-password" label="Senha" autocomplete="current-password" placeholder="Sua senha" bind:value={password} />
						{#if error}<p class="error" role="alert">{error}</p>{/if}
						<button class="btn primary" type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Entrando…' : 'Entrar no LearnVault'}</button>
					</fieldset>
				</form>
			</div>
		</section>
	</main>
{:else}
	<div class="shell graphite-app" data-board={activeBoard}>
		<a class="skip-link" href="#main-content">Pular para conteúdo</a>
		<header class="top">
			<a class="brand" href={href('home')}>Learn<i>Vault</i></a>
			<nav class="globalnav" aria-label="Navegação principal">
				<a class:on={route === 'home' || watchRoute} href={href('home')}>Aprender</a>
				<a class:on={route === 'notes'} href={href('notes')}>Anotações</a>
				<a class:on={route === 'settings'} href={href('settings')}>Configurações</a>
				<a class:on={route === 'analytics'} href={href('analytics')}>Estatísticas</a>
			</nav>
			<form class="search-form" role="search" onsubmit={(event) => event.preventDefault()}>
				<label class="visually-hidden" for="global-search">{route === 'notes' ? 'Buscar anotações' : 'Buscar cursos'}</label>
				<div class="search-field">
					<svg class="search-glyph" viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7" fill="none" stroke="currentColor" stroke-width="2"></circle><path d="m16.6 16.6 4.2 4.2" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"></path></svg>
				<input id="global-search" class="search" type="search" placeholder={route === 'notes' ? 'Buscar anotações' : 'Buscar cursos'} value={route === 'notes' ? notesSearchQuery : catalogQuery} oninput={(event) => route === 'notes' ? notesSearchQuery = event.currentTarget.value : catalogQuery = event.currentTarget.value} onkeydown={(event) => { if (event.key === 'Enter' && route !== 'notes') { event.preventDefault(); courseFilter = 'all'; location.hash = href('home'); } }} />
				</div>
			</form>
			{#if studySyncSupported}<Pomodoro />{/if}
			{#if transportKind === 'http'}
				<details class="account-details" bind:this={accountDetails}>
					<summary class="user account-button" title="Abrir conta" aria-label="Abrir conta">{user.username.slice(0, 1).toUpperCase()}</summary>
					<div class="account-menu" role="group" aria-label="Conta de {user.username}">
						<span class="account-name">{user.username}</span>
						<a href={href('settings')} aria-label="Abrir configurações">Configurações</a>
						<button class="text-button danger" type="button" onclick={logout}>Encerrar sessão</button>
					</div>
				</details>
			{:else}
				<div class="user" aria-label={user.username}>{user.username.slice(0, 1).toUpperCase()}</div>
			{/if}
		</header>
		<nav class="mobile-nav" aria-label="Navegação principal móvel">
			<a class:on={route === 'home' || watchRoute} href={href('home')}>Aprender</a>
			<a class:on={route === 'notes'} href={href('notes')}>Anotações</a>
			<a class:on={route === 'settings'} href={href('settings')}>Configurações</a>
			<a class:on={route === 'analytics'} href={href('analytics')}>Estatísticas</a>
		</nav>
		<div class="layout" class:watch-layout={watchRoute}>
			<aside class="sidebar" aria-hidden="true"></aside>
			<main id="main-content" tabindex="-1" class:home={route === 'home'} class:import-flow={route === 'home' && (importFormOpen || importPreview !== null)} class:course={route === 'course' && !watchRoute} class:settings={route === 'settings'} class:watch={watchRoute}>{#if error}<p class="error" role="alert">{error}</p>{/if}
{#if route === 'home'}
	{#if importPreview}
		<ImportPreview preview={importPreview} onCancel={cancelImportPreview} onConfirm={confirmImport} busy={importing} step={importPhase === 'rescan' || importPhase === 'rescan-error' ? 5 : importing ? 4 : 3} error={importError} onRetryRescan={importPhase === 'rescan-error' ? () => void finishImportRescan() : null} />
	{:else if importFormOpen}
		<section class="import-form-screen" aria-labelledby="import-heading">
			<header class="page-head">
				<div>
					<h1 id="import-heading">Importar curso</h1>
					<p class="sub">Informe a pasta no servidor para analisar o conteúdo antes de importar.</p>
				</div>
			</header>
			<ImportSteps currentStep={previewingImport ? 2 : 1} />
			<div class="import-layout">
				<section class="import-card" aria-labelledby="import-scope-heading">
					<form class="import-form" onsubmit={submitImport} aria-busy={previewingImport}>
						<label class="form-field" for="import-path">Caminho da pasta do curso no servidor
							<input id="import-path" name="import-path" class="field" bind:value={importPath} placeholder="C:\cursos\Meu curso" required disabled={previewingImport} autocomplete="off" spellcheck={false} aria-describedby={importError ? 'import-path-help import-path-error' : 'import-path-help'} aria-invalid={importError ? 'true' : undefined} />
							<p class="small" id="import-path-help">Use um caminho acessível pelo servidor; ele pode ser diferente do caminho deste navegador.</p>
						</label>
						{#if importError}
							<div class="import-error" id="import-path-error" role="alert">
								<strong>Não foi possível analisar esta pasta</strong>
								<p>{importError}</p>
								<p class="small">Verifique o caminho e as permissões de leitura, depois tente novamente.</p>
							</div>
						{/if}
						<div class="flow-actions">
							<button class="primary" type="submit" disabled={previewingImport || importing} aria-busy={previewingImport}>{previewingImport ? 'Analisando…' : 'Analisar pasta'}</button>
							<button type="button" onclick={() => (importFormOpen = false)} disabled={previewingImport || importing}>Cancelar</button>
						</div>
					</form>
				</section>
				<aside class="import-help" aria-labelledby="import-safety-heading">
					<h3 id="import-safety-heading">Importação segura</h3>
					<p>Somente leitura — a análise não grava no catálogo nem altera a pasta.</p>
					<p>Confirmação explícita — o curso só é registrado após revisar a prévia.</p>
					<p>Arquivos preservados — importar não move, renomeia ou apaga nada.</p>
				</aside>
			</div>
		</section>
	{:else}
		<header class="page-head library-head">
			<div>
				<h1 class="title">Meus cursos</h1>
				<p class="sub">Retome uma aula diretamente ou escolha outro curso.</p>
			</div>
			{#if catalog && catalog.nodes.length}
				<div class="tools" role="group" aria-label="Ordenação e filtros do catálogo">
					<select class="btn library-sort" aria-label="Ordenar cursos" bind:value={courseSort}>
						<option value="recent" disabled={!studyEnabled}>Recentes</option>
						<option value="library">Ordem da biblioteca</option>
					</select>
					<div class="course-filters" id="course-filters" role="group" aria-label="Filtrar cursos">
						<button id="course-filter-all" class="btn" class:active={courseFilter === 'all'} aria-pressed={courseFilter === 'all'} onclick={() => (courseFilter = 'all')}>Tudo</button>
						<button class="btn" class:active={courseFilter === 'in-progress'} aria-pressed={courseFilter === 'in-progress'} onclick={() => (courseFilter = 'in-progress')}>Em andamento</button>
					</div>
					{#if canManage}
						<div class="library-actions" aria-label="Ações da biblioteca">
							{#if transportKind === 'desktop'}
								<button class="btn primary" type="button" onclick={importCourse} disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar curso'}</button>
							{:else}
								<button class="btn primary" type="button" onclick={toggleImportForm} aria-expanded={importFormOpen} disabled={importing || previewingImport} aria-busy={importing || previewingImport}>Importar curso</button>
							{/if}
							<details class="library-more">
								<summary class="btn">Mais ações</summary>
								<div class="home-actions">
									<button class="secondary" type="button" onclick={rescan} disabled={rescanning || !catalog || !catalog.nodes.length}>{rescanning ? 'Indexando…' : 'Indexar cursos'}</button>
								</div>
							</details>
						</div>
					{/if}
				</div>
			{/if}
		</header>
		{#if catalog && catalog.nodes.length === 0}
			{#if canManage}
				<div class="welcome-block">
					<div class="welcome-main">
						<h2>Adicione seu primeiro curso</h2>
						<p>Escolha uma pasta que já existe. Seus arquivos continuam no lugar.</p>
						{#if transportKind === 'desktop'}
							<button class="btn primary" type="button" onclick={importCourse} disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar curso'}</button>
						{:else}
							<button class="btn primary" type="button" onclick={toggleImportForm} disabled={importing || previewingImport} aria-busy={importing || previewingImport}>Importar curso</button>
						{/if}
						{#if importError}<p class="error" role="alert">{importError}</p>{/if}
					</div>
					<aside class="welcome-help">
						<h3>Seus arquivos ficam onde estão</h3>
						<ul>
							<li>O LearnVault lê a pasta no lugar onde ela está.</li>
							<li>Nada é movido, renomeado ou apagado.</li>
							{#if transportKind === 'http'}<li>Você revisa a estrutura antes de importar.</li>{:else}<li>O desktop usa o seletor de pasta nativo do sistema.</li>{/if}
						</ul>
					</aside>
				</div>
			{:else}
				<EmptyState title="Sua biblioteca ainda está vazia" description="Nenhum curso foi importado nesta instalação. Peça ao administrador para importar um curso pela área dele." />
			{/if}
		{:else}
			{#if studySyncSupported && inProgressCourseIds.size}
				<section class="recent-study" aria-labelledby="recent-heading">
					<div class="row section-row"><h2 id="recent-heading" class="section-head">Continuar estudando</h2><span class="small">{inProgressCourseIds.size} {inProgressCourseIds.size === 1 ? 'curso em andamento' : 'cursos em andamento'} · atividade recente</span></div>
					<ul class="resume-list">
						{#each recentInProgressItems() as item, index}
							{@const location = findLessonLocation(catalog, item.mediaId)}
							{#if location}
								<li class="resume-item">
									<div class="cover" class:alt={index === 1} class:violet={index > 1} aria-hidden="true">{courseCover(location.courseTitle).monogram}</div>
									<div class="resume-copy">
										<h3>{location.courseTitle}</h3>
										<p>{location.lesson.title} · {formatLocationContext(location.courseTitle, location.moduleTitle)} · Parou em {formatDuration(item.positionSeconds)}</p>
									</div>
									<a class="btn primary" href={href('player', location.courseId, location.lesson.path)}>Retomar aula</a>
								</li>
							{/if}
						{/each}
					</ul>
				</section>
			{/if}
			{#if user.role === 'admin' && rescanMessage}<p class="catalog-message" aria-live="polite">{rescanMessage}</p>{/if}
			{#if canManage}<input class="visually-hidden course-cover-input" type="file" accept={COVER_ACCEPT} bind:this={coverInput} onchange={pickCover} tabindex="-1" aria-hidden="true" />{/if}
			{#if coverError}<p class="error" role="alert">{coverError}</p>{/if}
			{#if coverMessage}<p class="catalog-message" role="status" aria-live="polite">{coverMessage}</p>{/if}
			{#if catalog && catalog.nodes.length}
				{#if filteredNodes.length}
					<ul class="course-grid" aria-label="Todos os cursos">
						{#each filteredNodes as courseItem (courseItem.courseId ?? courseItem.path)}
							<li><CourseCard course={courseItem} profile={user.userId} mediaUrl={(file, courseId) => transport.mediaUrl(file, courseId)} manualCoverUrl={transport.coverUrl(courseIdOf(courseItem), courseItem.coverVersion ?? null)} {canManage} menuOpen={coverMenuId === courseIdOf(courseItem)} onMenuOpen={(open) => toggleCoverMenu(courseItem, open)} onChangeCover={requestCourseCover} onRemoveCover={removeCover} /></li>
						{/each}
					</ul>
				{:else if catalogQuery.trim() !== ''}
					<EmptyState title="Nenhum curso corresponde à busca" description={`Nada encontrado para “${catalogQuery.trim()}”. Ajuste o termo ou limpe a busca.`} action={{ label: 'Limpar busca', onClick: () => { catalogQuery = ''; courseFilter = 'all'; } }} />
				{:else if courseFilter === 'in-progress'}
					<EmptyState title="Nenhum curso em andamento" description="Você ainda não abriu nenhuma aula nesta instalação." action={{ label: 'Mostrar todos os cursos', onClick: () => (courseFilter = 'all') }} />
				{/if}
			{/if}
			{#if catalog && catalog.looseFiles.length}<LooseFiles files={catalog.looseFiles} heading="Arquivos soltos" />{/if}
		{/if}
	{/if}
{:else if route === 'course' && !watchRoute}
	{#if !course}<section class="wrap"><h1>Curso não encontrado</h1><p class="empty" role="status">Não foi possível encontrar este curso no catálogo atual.</p></section>
	{:else}
		{@const counts = countCatalogNode(course)}
		{@const resume = recent.find((item) => findLessonLocation(catalog, item.mediaId)?.courseId === courseIdOf(course)) ?? null}
		{@const resumeLocation = resume ? findLessonLocation(catalog, resume.mediaId) : null}
		<header class="course-hero">
			<div class="wrap">
				<div><div class="eyebrow">Seu curso</div><h1>{course.title}</h1><p>Conteúdo local indexado no LearnVault. Os arquivos continuam na pasta de origem.</p><div class="course-stats"><span>{counts.modules} módulos</span><span>{counts.lessons} aulas</span><span>{counts.materials} materiais</span></div></div>
				<div class="cover" aria-hidden="true"><span>{course.title.slice(0, 2).toUpperCase()}</span></div>
			</div>
		</header>
		<div class="overview-grid">
			<section class="outline" aria-labelledby="course-outline-heading">
				<div class="outline-head"><strong id="course-outline-heading">Conteúdo do curso</strong><span class="small">{formatLessonCounts(counts)}</span></div>
				{#if canManage}<div class="catalog-order-toolbar">
					<button class="text-button" type="button" onclick={() => { organizingCatalog = !organizingCatalog; orderError = ''; orderMessage = ''; }} aria-pressed={organizingCatalog} disabled={orderBusy}>{organizingCatalog ? 'Concluir organização' : 'Organizar aulas'}</button>
					{#if organizingCatalog}<span class="small">Use as setas para mover módulos e aulas. Os arquivos permanecem na pasta original.</span>{/if}
					{#if orderError}<p class="error" role="alert">{orderError}</p>{/if}
					{#if orderMessage}<p class="catalog-message" role="status" aria-live="polite">{orderMessage}</p>{/if}
				</div>{/if}
				{#if course.children.length}<CatalogTree nodes={course.children} courseId={courseIdOf(course)} showFiles={true} organizing={organizingCatalog} orderSaving={orderBusy} onSaveOrder={saveCourseOrder} onResetOrder={resetParentOrder} />{:else}<p class="empty" role="status">Este curso ainda não contém itens indexados.</p>{/if}
				{#if course.looseFiles.length}<LooseFiles files={course.looseFiles} heading="Materiais do curso" courseId={courseIdOf(course)} />{/if}
			</section>
			<aside class="resume-box" aria-labelledby="resume-heading">
				<h2 id="resume-heading">Continuar estudando</h2>
				{#if resume && resumeLocation}
					<p>{resumeLocation.lesson.title}</p><p class="small">{resume.completed ? 'Aula concluída' : `Última posição: ${formatDuration(resume.positionSeconds)}`}</p>
					<a class="btn primary" href={href('player', resumeLocation.courseId, resumeLocation.lesson.path)}>Retomar aula</a>
				{:else if course.children.length}
					<p class="small">Ainda não há atividade registrada neste curso.</p>
					{@const firstLesson = defaultLessonPath([course], [])}
					{#if firstLesson}<a class="btn primary" href={href('player', courseIdOf(course), firstLesson)}>Começar curso</a>{/if}
				{:else}<p class="small">Nenhuma aula disponível para iniciar.</p>{/if}
			</aside>
		</div>
	{/if}
{:else if route === 'settings'}
	<div class="wrap settings-wrap">
		<div class="eyebrow">Seu ambiente</div>
		<h1 class="title">Configurações</h1>
		<p class="sub">Aparência, conta e comportamento do LearnVault.</p>
		<div class="settings-layout">
			<nav class="setting-nav" aria-label="Seções de configurações">
				<button class:sel={settingsSection === 'theme'} type="button" aria-current={settingsSection === 'theme' ? 'page' : undefined} onclick={() => showSettingsSection('theme', 'theme-heading')}>Aparência</button>
				<button class:sel={settingsSection === 'account'} type="button" aria-current={settingsSection === 'account' ? 'page' : undefined} onclick={() => showSettingsSection('account', 'account-heading')}>Conta e segurança</button>
				{#if transportKind === 'http'}<button class:sel={settingsSection === 'updates'} type="button" aria-current={settingsSection === 'updates' ? 'page' : undefined} onclick={() => showSettingsSection('updates', 'update-heading')}>Atualizações</button>{/if}
			</nav>
			<div class="setting-content">
				<section class="setting-card" aria-labelledby="theme-heading" use:trackSection={'theme'}>
					<h3 id="theme-heading" tabindex="-1">Tema</h3>
					<p>Escolha o modo de exibição. A paleta selecionada acompanha os dois modos.</p>
					<div class="tools theme-buttons" role="group" aria-label="Tema">
						<button class="btn" class:primary={themeMode === 'light'} type="button" aria-pressed={themeMode === 'light'} onclick={() => changeTheme('light')}>Claro</button>
						<button class="btn" class:primary={themeMode === 'dark'} type="button" aria-pressed={themeMode === 'dark'} onclick={() => changeTheme('dark')}>Escuro</button>
						<button class="btn" class:primary={themeMode === 'system'} type="button" aria-pressed={themeMode === 'system'} onclick={() => changeTheme('system')}>Sistema</button>
					</div>
					<p class="theme-status" aria-live="polite">Tema atual: {resolvedTheme === 'dark' ? 'escuro' : 'claro'}{themeMode === 'system' ? ' (sistema)' : ''}</p>
					{#if themePersistenceMessage}<p class="inline-status" role="status">{themePersistenceMessage}</p>{/if}
				</section>
				<section class="setting-card" aria-labelledby="palette-heading">
					<h3 id="palette-heading">Paletas</h3>
					<PaletteSettings preferences={palettePreferences} onChange={changePalette} />
				</section>
				<section class="setting-card" aria-labelledby="account-heading" use:trackSection={'account'}>
					<h3 id="account-heading" tabindex="-1">Conta</h3>
					<p><strong>{accountUser.username}</strong> · {accountUser.isAdmin ? 'Administrador' : 'Estudante'}</p>
					{#if transportKind === 'http'}
						<CredentialsForm user={accountUser} onUpdated={(updated) => (user = updated)} />
						<button class="text-button danger" type="button" onclick={logout}>Encerrar sessão</button>
					{:else}<p class="catalog-message">Biblioteca local neste dispositivo.</p>{/if}
				</section>
				{#if transportKind === 'http'}
					<section class="setting-card" aria-labelledby="update-heading" use:trackSection={'updates'}>
						<h3 id="update-heading" tabindex="-1">Atualizações</h3>
						{#if updateStatus}<p>Versão atual: <strong>{updateStatus.current}</strong></p>{/if}
						<button class="text-button" type="button" onclick={() => checkUpdate(true)} disabled={updateChecking} aria-busy={updateChecking}>{updateChecking ? 'Verificando…' : 'Verificar atualização'}</button>
						{#if updateStatus}
							{@const outcome = updateOutcome(updateStatus)}
							{@const releaseUrl = updateReleaseUrl(updateStatus)}
							{#if outcome !== 'error' || updateManual}<p class="catalog-message" role="status">{updateMessage(updateStatus)}</p>{/if}
							{#if outcome === 'available' && releaseUrl}<p class="catalog-message"><a href={releaseUrl} target="_blank" rel="noopener noreferrer">Ver a release</a></p>{/if}
						{/if}
					</section>
				{/if}
			</div>
		</div>
	</div>
{:else if route === 'analytics'}
	<section class="analytics" aria-labelledby="analytics-heading">
		<header class="analytics-head"><div><div class="eyebrow">Seu ritmo de aprendizagem</div><h1 id="analytics-heading">Estatísticas de estudo</h1><p>Resumo do progresso salvo nesta instalação.</p></div></header>
		<p class="tracking-note"><strong>Dados registrados pelo LearnVault.</strong> Totais baseados apenas nas posições e conclusões salvas; sem estimativas de atividade externa.</p>
		{#if studyAnalytics}
			{#if studyAnalytics.trackedCount === 0}
				<EmptyState title="Nenhum estudo registrado ainda" description="As estatísticas aparecem quando você reproduz aulas nesta instalação." action={{ label: 'Ir para os cursos', href: href('home') }} />
			{:else}
			<div class="metric-strip" aria-label="Totais de estudo">
				<div class="metric"><small>Aulas acompanhadas</small><strong>{studyAnalytics.trackedCount}</strong><em>com progresso salvo</em></div>
				<div class="metric"><small>Aulas concluídas</small><strong>{studyAnalytics.completedCount}</strong><em>marcações explícitas</em></div>
				<div class="metric"><small>Posição acumulada</small><strong>{formatDuration(studyAnalytics.totalPositionSeconds)}</strong><em>soma das posições salvas</em></div>
				<div class="metric"><small>Última atividade</small><strong>{studyAnalytics.lastActivityAt === null ? '—' : new Date(studyAnalytics.lastActivityAt * 1000).toLocaleDateString('pt-BR')}</strong><em>{studyAnalytics.lastActivityAt === null ? 'sem atividade registrada' : 'data do registro mais recente'}</em></div>
			</div>
			{/if}
		{:else if studyAnalyticsError}
			<div class="analytics-card"><EmptyState tone="error" title="Não foi possível carregar as estatísticas" description={studyAnalyticsError} /><button class="btn" type="button" onclick={() => { studyAnalyticsRequested = false; }}>Tentar novamente</button></div>
		{:else}<p class="empty" role="status">Carregando estatísticas…</p>{/if}
		<div class="analytics-grid">
			<section class="analytics-card"><h2>Atividade recente</h2><p>Últimas posições e conclusões salvas no catálogo local.</p>
				{#if recent.length}
					<div class="activity-table-wrap"><table class="activity-table" aria-label="Atividade de estudo recente"><thead><tr><th>Data</th><th>Curso</th><th>Registro</th></tr></thead><tbody>
						{#each recent.slice(0, 6) as item}
							{@const location = findLessonLocation(catalog, item.mediaId)}
							<tr><td>{item.updatedAt === null ? '—' : new Date(item.updatedAt * 1000).toLocaleString('pt-BR', { dateStyle: 'short', timeStyle: 'short' })}</td><td>{location?.courseTitle ?? 'Mídia fora do catálogo atual'}</td><td>{item.completed ? 'Concluída' : `Posição ${formatDuration(item.positionSeconds)}`}</td></tr>
						{/each}
					</tbody></table></div>
				{:else}<p class="empty" role="status">Ainda não há posições ou conclusões registradas.</p>{/if}
			</section>
			<section class="analytics-card"><h2>Progresso por curso</h2><p>Os registros atuais mostram totais e atividade recente, sem comparação por curso.</p></section>
		</div>
	</section>
{:else if route === 'notes'}
	<NotesIndex {catalog} bind:searchQuery={notesSearchQuery} />
{:else if watchRoute}
	{#if !course}<h1>Curso não encontrado</h1><p class="empty" role="status">Não foi possível encontrar este curso no catálogo atual. Ele pode ter sido removido.</p>
	{:else}
		<h1 class="visually-hidden">{course.title}</h1>
		<div class="watch-header">
			<nav class="breadcrumb" aria-label="Caminho do curso"><a href={href('home')}>Cursos</a><span aria-hidden="true">/</span><span>{course.title}</span></nav>
			{#if canManage}
				<div class="course-menu-wrap">
					<button class="course-menu-toggle" type="button" aria-haspopup="menu" aria-expanded={courseMenuOpen} onclick={() => { courseMenuOpen = !courseMenuOpen; removeConfirm = false; }}>Gerenciar curso <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg></button>
					{#if courseMenuOpen}
						<div class="course-menu" role="menu" aria-label="Gerenciar {course.title}">
							{#if removeConfirm}
								<p class="course-menu-question" role="alert">Remover “{course.title}” da biblioteca?</p>
								<p class="course-menu-hint">Os arquivos não serão apagados.</p>
								<div class="course-menu-actions">
									<button class="text-button danger" type="button" role="menuitem" onclick={removeCourse} disabled={removing} aria-busy={removing}>{removing ? 'Removendo…' : 'Remover curso'}</button>
									<button class="text-button" type="button" role="menuitem" onclick={() => (removeConfirm = false)} disabled={removing}>Cancelar</button>
								</div>
							{:else}
								<button class="text-button" type="button" role="menuitem" onclick={resetOrder} disabled={orderBusy} aria-busy={orderBusy}>{orderBusy ? 'Restaurando…' : 'Restaurar ordem'}</button>
								<button class="text-button danger" type="button" role="menuitem" onclick={() => (removeConfirm = true)}>Remover curso</button>
							{/if}
						</div>
					{/if}
				</div>
			{/if}
		</div>
		{#if removeError}<p class="error" role="alert">{removeError}</p>{/if}
		{#if orderError}<p class="error" role="alert">{orderError}</p>{/if}
		{#if orderMessage}<p class="catalog-message" role="status" aria-live="polite">{orderMessage}</p>{/if}
		<div class="watch-grid">
			<div class="watch-main">
				{#if lesson}
					<Player {lesson} courseId={courseIdOf(course)} lessonTitle={lesson.title} {mediaError} onMediaError={mediaFailed} {studyEnabled} preferences={playerPreferences}
						onPreferencesChange={savePlayerPreferences} autoNext={playerPreferences.autoNext}
						hasNextLesson={neighbors.next !== null} onAutoNext={autoAdvance}
						onAutoNextChange={(enabled) => savePlayerPreferences({ ...playerPreferences, autoNext: enabled })} />
				{:else}<p class="empty" role="status">Este curso não tem nenhuma aula disponível.</p>{/if}
				{#if activeContainer && (activeContainer.files.length || activeContainer.looseFiles.length)}<LooseFiles files={[...activeContainer.files, ...activeContainer.looseFiles]} heading="Materiais do módulo" courseId={courseIdOf(course)} />{/if}
				{#if course.looseFiles.length}<LooseFiles files={course.looseFiles} heading="Arquivos soltos do curso" courseId={courseIdOf(course)} />{/if}
				{#if neighbors.previous || neighbors.next}<nav class="lesson-nav" aria-label="Aulas vizinhas">{#if neighbors.previous}<a href={href('course', courseIdOf(course), neighbors.previous.path)}>← {neighbors.previous.title}</a>{/if}{#if neighbors.next}<a href={href('course', courseIdOf(course), neighbors.next.path)}>{neighbors.next.title} →</a>{/if}</nav>{/if}
			</div>
			<aside class="watch-playlist" aria-label="Aulas do curso">
				<div class="playlist-heading"><h2>Aulas do curso</h2><span>{formatLessonCounts(countCatalogNode(course))}</span></div>
				<div class="course-playlist-body" data-playlist-body>
					{#if course.children.length}<CatalogTree nodes={course.children} courseId={courseIdOf(course)} activePath={activePath} showFiles={false} />{:else}<p class="empty" role="status">Este curso ainda não foi indexado.</p>{/if}
				</div>
			</aside>
		</div>
	{/if}
{:else}
	{#if !lesson}<h1>Aula não encontrada</h1><p class="empty" role="status">Não foi possível encontrar esta aula no catálogo atual.</p>{:else}
		<a class="back" href={href('home')}>← Início</a>
		<h1>{lesson.title}</h1>
		<Player {lesson} courseId={courseId ?? ''} {mediaError} onMediaError={mediaFailed} {studyEnabled} preferences={playerPreferences}
			onPreferencesChange={savePlayerPreferences} autoNext={playerPreferences.autoNext}
			hasNextLesson={neighbors.next !== null} onAutoNext={autoAdvance}
			onAutoNextChange={(enabled) => savePlayerPreferences({ ...playerPreferences, autoNext: enabled })} />
		{#if neighbors.previous || neighbors.next}<nav class="lesson-nav" aria-label="Aulas vizinhas">{#if neighbors.previous}<a href={href('player', courseId ?? '', neighbors.previous.path)}>← {neighbors.previous.title}</a>{/if}{#if neighbors.next}<a href={href('player', courseId ?? '', neighbors.next.path)}>{neighbors.next.title} →</a>{/if}</nav>{/if}
	{/if}
{/if}
</main></div></div>{/if}
