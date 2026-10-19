<script lang="ts">
	import { onMount } from 'svelte';
	import LooseFiles from './LooseFiles.svelte';
	import CatalogTree from './CatalogTree.svelte';
	import CourseCard from './CourseCard.svelte';
	import Player from './Player.svelte';
	import { filterCatalog } from './search';
	import { countCatalogNode, formatLessonCounts, formatLocationContext } from './catalog';
	import PasswordField from './PasswordField.svelte';
	import NotesIndex from './NotesIndex.svelte';
	import { defaultLessonPath, lessonContainer, neighboringLessons } from './navigation';
	import { destinationAfterLogin, findCourse, findLesson, findLessonLocation, href, locationFromHash, type Route } from './router';
	import { transport, transportKind, COVER_ACCEPT, MAX_COVER_BYTES, courseIdOf, STUDY_NOT_AVAILABLE, type Catalog, type CatalogNode, type Lesson, type StudyProgress, type UpdateStatus, type User } from './transport';
	import { updateOutcome, updateMessage, updateReleaseUrl } from './update';
	import { defaultRoute, resolveSession } from './session';
	import { formatDuration, studySyncSupported } from './player';
	import { createDefaultPlayerPreferences, playerPreferencesScope, readPlayerPreferences, writePlayerPreferences, type PlayerPreferences } from './player-preferences';
	import Brand from './Brand.svelte';
	import { applyTheme, persistThemeMode, readThemeMode, resolveTheme, type ThemeMode } from './theme';

	let themeMode: ThemeMode = readThemeMode(safeLocalStorage());
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
	// A tela de estudo é a do curso unificado; o endereço legado `#/player/<curso>/<aula>`
	// abre a mesma tela, e `#/player/<caminho>` continua sendo o player de arquivo solto.
	$: watchRoute = route === 'course' || (route === 'player' && courseId !== null);
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
			if (themeColor) themeColor.content = resolvedTheme === 'dark' ? '#171c19' : '#f7f6f1';
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
			if (target instanceof Element && target.closest('.course-card-menu, .course-card-menu-toggle')) return;
			coverMenuId = null;
		};
		const closeMenusOnEscape = (event: KeyboardEvent) => {
			if (event.key !== 'Escape') return;
			coverMenuId = null;
			courseMenuOpen = false;
			removeConfirm = false;
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
		location.hash = href('course', courseId ?? '', neighbors.next.path);
	}
	function changeTheme(mode: ThemeMode) {
		themeMode = mode;
		resolvedTheme = resolveTheme(mode, systemDark);
		applyTheme(document.documentElement, resolvedTheme);
		const themeColor = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
		if (themeColor) themeColor.content = resolvedTheme === 'dark' ? '#171c19' : '#f7f6f1';
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
{#if loading}<div class="loading" aria-live="polite">Carregando seus cursos…</div>
{:else if !user}
	<main class="login-page" aria-labelledby="login-heading">
		<div class="brand"><Brand /> LEARNVAULT</div>
		<section>
			<h1 id="login-heading">Entrar no LearnVault</h1>
			<p class="intro">Acesse sua biblioteca de cursos.</p>
			<form onsubmit={login}>
				<fieldset class="auth-fields" disabled={authBusy}>
					<label class="form-field" for="login-username">Usuário<input id="login-username" autocomplete="username" autocapitalize="none" spellcheck={false} bind:value={username} required /></label>
					<PasswordField id="login-password" label="Senha" autocomplete="current-password" bind:value={password} />
					{#if error}<p class="error" role="alert">{error}</p>{/if}
					<button type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Entrando…' : 'Entrar'}</button>
				</fieldset>
			</form>
		</section>
	</main>
{:else}<div class="shell"><div class="layout" class:watch-layout={watchRoute}><aside class="sidebar"><div class="brand"><Brand /> LEARNVAULT</div><nav aria-label="Navegação principal"><a class:active={route === 'home' || watchRoute} href={href('home')}>Início</a><a class:active={route === 'notes'} href={href('notes')}>Anotações</a><a class:active={route === 'settings'} href={href('settings')}>Configurações</a></nav>{#if transportKind === 'http'}<button class="text-button sidebar-logout" onclick={logout}>Sair</button>{/if}</aside><main class:home={route === 'home'} class:watch={watchRoute}>{#if error}<p class="error" role="alert">{error}</p>{/if}
{#if route === 'home'}
	<h1 class="visually-hidden">Início</h1>
	{#if canManage || (catalog && catalog.nodes.length)}
		<div class="home-toolbar">
			{#if catalog && catalog.nodes.length}<form class="catalog-search" role="search" onsubmit={(event) => event.preventDefault()}><label class="visually-hidden" for="catalog-search-input">Buscar nos cursos</label><div class="search-field"><svg class="search-glyph" viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7" fill="none" stroke="currentColor" stroke-width="2"></circle><path d="m16.6 16.6 4.2 4.2" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"></path></svg><input id="catalog-search-input" type="search" placeholder="Buscar nos cursos" bind:value={catalogQuery} /></div></form>{/if}
			{#if canManage}
				<div class="home-actions">
					{#if transportKind === 'desktop'}<button class="primary" onclick={importCourse} disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar curso'}</button>{:else}<button class="primary" onclick={() => (importFormOpen = !importFormOpen)} aria-expanded={importFormOpen}>Importar curso</button>{/if}
					<button class="secondary" onclick={rescan} disabled={rescanning || !catalog || !catalog.nodes.length}>{rescanning ? 'Indexando…' : 'Indexar cursos'}</button>
				</div>
			{/if}
		</div>
	{/if}
	{#if studySyncSupported && recent.length}<section class="recent-study" aria-labelledby="recent-heading"><h2 id="recent-heading">Continuar assistindo</h2><ul>{#each recent.slice(0, 6) as item}{@const location = findLessonLocation(catalog, item.mediaId)}<li>{#if location}<div class="recent-study-info"><a href={href('course', location.courseId, location.lesson.path)}>{location.lesson.title}</a><small title={formatLocationContext(location.courseTitle, location.moduleTitle)}>{formatLocationContext(location.courseTitle, location.moduleTitle)}</small></div><small>{item.completed ? 'Concluído' : `Parou em ${formatDuration(item.positionSeconds)}`}</small>{:else}<span>Mídia não disponível no catálogo</span>{/if}</li>{/each}</ul></section>{/if}
	{#if !canManage && catalog && catalog.nodes.length === 0}<p class="empty" role="status">Nenhum curso disponível. Peça ao administrador para importar um curso.</p>{/if}
	{#if user.role === 'admin' && importFormOpen}<form class="import-form" onsubmit={submitImport}><label for="import-path">Caminho da pasta do curso no servidor</label><input id="import-path" bind:value={importPath} placeholder="C:\cursos\Meu curso" required /><div class="import-form-actions"><button class="primary" disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar'}</button><button type="button" class="text-button" onclick={() => (importFormOpen = false)}>Cancelar</button></div></form>{/if}
	{#if importError}<p class="error" role="alert">{importError}</p>{/if}
	{#if user.role === 'admin' && rescanMessage}<p class="catalog-message" aria-live="polite">{rescanMessage}</p>{/if}
	{#if canManage}<input class="visually-hidden course-cover-input" type="file" accept={COVER_ACCEPT} bind:this={coverInput} onchange={pickCover} tabindex="-1" aria-hidden="true" />{/if}
	{#if coverError}<p class="error" role="alert">{coverError}</p>{/if}
	{#if coverMessage}<p class="catalog-message" role="status" aria-live="polite">{coverMessage}</p>{/if}
	{#if catalog && catalog.nodes.length}
	<h2 id="courses-heading" class="courses-heading">Cursos</h2>
{#if filteredNodes.length}<ul class="course-grid" aria-labelledby="courses-heading">{#each filteredNodes as courseItem (courseItem.courseId ?? courseItem.path)}<li><CourseCard course={courseItem} profile={user.userId} mediaUrl={(file, courseId) => transport.mediaUrl(file, courseId)} manualCoverUrl={transport.coverUrl(courseIdOf(courseItem), courseItem.coverVersion ?? null)} {canManage} menuOpen={coverMenuId === courseIdOf(courseItem)} onMenuOpen={(open) => toggleCoverMenu(courseItem, open)} onChangeCover={requestCourseCover} onRemoveCover={removeCover} /></li>{/each}</ul>
		{:else}<p class="empty" role="status">Nenhum curso corresponde a “{catalogQuery.trim()}”.</p>{/if}
	{:else if canManage}<div class="empty-state"><p class="empty">Nenhum curso por aqui ainda.</p><p class="intro">Clique em “Importar curso” e escolha a pasta de um curso. Nada é movido, alterado ou apagado — a pasta permanece somente leitura.</p></div>{/if}
	{#if catalog && catalog.looseFiles.length}<LooseFiles files={catalog.looseFiles} heading="Arquivos soltos" />{/if}
{:else if route === 'settings'}<h1>Preferências</h1><p class="intro">Escolha como o LearnVault aparece neste dispositivo.</p><fieldset class="theme-options"><legend>Tema</legend><label><input type="radio" name="theme" value="system" checked={themeMode === 'system'} onchange={() => changeTheme('system')} />Sistema <span>Acompanha a preferência do sistema.</span></label><label><input type="radio" name="theme" value="light" checked={themeMode === 'light'} onchange={() => changeTheme('light')} />Claro <span>Usa sempre o tema claro.</span></label><label><input type="radio" name="theme" value="dark" checked={themeMode === 'dark'} onchange={() => changeTheme('dark')} />Escuro <span>Usa sempre o tema escuro.</span></label></fieldset><p class="theme-status" aria-live="polite">Tema atual: {resolvedTheme === 'dark' ? 'escuro' : 'claro'}{themeMode === 'system' ? ' (sistema)' : ''}</p><section class="account-settings" aria-labelledby="account-heading"><h2 id="account-heading">Conta</h2><p><strong>{user.username}</strong> · {user.isAdmin ? 'Administrador' : 'Estudante'}</p><form class="credentials-form" onsubmit={credentials}><fieldset class="auth-fields" disabled={authBusy}><legend>Credenciais de acesso</legend><PasswordField id="settings-current-password" label="Senha atual" autocomplete="current-password" bind:value={currentPassword} /><label class="form-field" for="settings-new-username">Novo usuário<input id="settings-new-username" autocomplete="username" bind:value={username} required /></label><PasswordField id="settings-new-password" label="Nova senha" autocomplete="new-password" bind:value={password} minLength={12} hint="Use ao menos 12 caracteres." />{#if accountError}<p class="error" role="alert">{accountError}</p>{/if}{#if accountStatus}<p class="catalog-message" role="status">{accountStatus}</p>{/if}<button type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Salvando…' : 'Atualizar credenciais'}</button></fieldset></form>{#if transportKind === 'http'}<button class="text-button" onclick={logout}>Encerrar sessão</button>{:else}<p class="catalog-message">Biblioteca local neste dispositivo.</p>{/if}</section>
{#if transportKind === 'http'}<section class="account-settings" aria-labelledby="update-heading"><h2 id="update-heading">Atualização</h2>{#if updateStatus}<p>Versão atual: <strong>{updateStatus.current}</strong></p>{/if}<button class="text-button" onclick={() => checkUpdate(true)} disabled={updateChecking} aria-busy={updateChecking}>{updateChecking ? 'Verificando…' : 'Verificar atualização'}</button>{#if updateStatus}{@const outcome = updateOutcome(updateStatus)}{@const releaseUrl = updateReleaseUrl(updateStatus)}{#if outcome !== 'error' || updateManual}<p class="catalog-message" role="status">{updateMessage(updateStatus)}</p>{/if}{#if outcome === 'available' && releaseUrl}<p class="catalog-message"><a href={releaseUrl} target="_blank" rel="noopener noreferrer">Ver a release</a></p>{/if}{/if}</section>{/if}
{:else if route === 'notes'}<NotesIndex {catalog} />
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
