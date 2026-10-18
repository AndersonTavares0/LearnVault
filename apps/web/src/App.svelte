<script lang="ts">
	import { onMount } from 'svelte';
	import LooseFiles from './LooseFiles.svelte';
	import CatalogTree from './CatalogTree.svelte';
	import CourseCard from './CourseCard.svelte';
	import Player from './Player.svelte';
	import { filterCatalog } from './search';
	import { countCatalogNode, formatCourseMeta, formatLocationContext } from './catalog';
	import PasswordField from './PasswordField.svelte';
	import { lessonSections, neighboringLessons, type LessonSection } from './navigation';
	import { destinationAfterLogin, findCourse, findLesson, findLessonLocation, href, locationFromHash, type Route } from './router';
	import { transport, transportKind, STUDY_NOT_AVAILABLE, type Catalog, type CatalogNode, type Lesson, type StudyProgress, type User } from './transport';
	import { resolveSession } from './session';
	import { formatDuration, studySyncSupported } from './player';
	import Brand from './Brand.svelte';
	import { applyTheme, persistThemeMode, readThemeMode, resolveTheme, type ThemeMode } from './theme';

	let themeMode: ThemeMode = readThemeMode(safeLocalStorage());
	let systemDark = window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;
	let resolvedTheme = resolveTheme(themeMode, systemDark);
	let route: Route = locationFromHash(location.hash).route;
	let courseId: string | null = locationFromHash(location.hash).courseId;
	let lessonPath: string | null = locationFromHash(location.hash).lessonPath;
	let user: User | null = null;
	let catalog: Catalog | null = null;
	let lesson: Lesson | null = null;
	let catalogQuery = '';
	let filteredNodes: CatalogNode[] = [];
	let sections: LessonSection[] = [];
	$: course = findCourse(catalog, courseId);
	$: courseMeta = course ? formatCourseMeta(countCatalogNode(course)) : '';
	$: filteredNodes = catalog ? filterCatalog(catalog.nodes, catalogQuery) : [];
	// Navegação e índice saem da subárvore do curso aberto. Quando uma reindexação remove
	// a aula aberta, `neighboringLessons` devolve os dois lados nulos e os controles somem
	// em vez de apontar para uma aula que não existe mais.
	$: neighbors = course ? neighboringLessons([course], lessonPath) : { previous: null, next: null };
	$: sections = course ? lessonSections([course]) : [];
	$: lessonCount = sections.reduce((total, section) => total + section.lessons.length, 0);
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
	let recent: StudyProgress[] = [];
	let recentLoading = false;
	let studyEnabled = studySyncSupported;

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
			const next = locationFromHash(location.hash);
			route = next.route;
			courseId = next.courseId;
			lessonPath = next.lessonPath;
			lesson = findLesson(catalog, courseId, lessonPath);
			removeConfirm = false;
			removeError = '';
			mediaError = '';
		};
		window.addEventListener('hashchange', change);
		void initialize();
		return () => {
			window.removeEventListener('hashchange', change);
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
			error = session.error;
			if (user) {
				catalog = await transport.catalog();
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
			password = '';
			catalog = await transport.catalog();
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
		lesson = findLesson(nextCatalog, courseId, lessonPath);
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
		if (!studySyncSupported) return;
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
	}

	function mediaFailed() {
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
{:else}<div class="shell"><header><div class="brand"><Brand /> LEARNVAULT</div><button class="text-button" onclick={logout}>Sair</button></header><div class="layout"><nav aria-label="Navegação principal"><a class:active={route === 'home'} href={href('home')}>Início</a><a class:active={route === 'settings'} href={href('settings')}>Configurações</a></nav><main><p class="eyebrow">BIBLIOTECA PESSOAL</p>{#if error}<p class="error" role="alert">{error}</p>{/if}
{#if route === 'home'}
	<h1>Olá, {user.username}.</h1>
	<p class="intro">{catalog && catalog.nodes.length ? 'Escolha um curso para continuar estudando.' : 'Importe seu primeiro curso para montar sua biblioteca.'}</p>
	{#if studySyncSupported && recent.length}<section class="recent-study" aria-labelledby="recent-heading"><h2 id="recent-heading">Continuar assistindo</h2><ul>{#each recent as item}{@const location = findLessonLocation(catalog, item.mediaId)}<li>{#if location}<div class="recent-study-info"><a href={href('player', location.courseId, location.lesson.path)}>{location.lesson.title}</a><small title={formatLocationContext(location.courseTitle, location.moduleTitle)}>{formatLocationContext(location.courseTitle, location.moduleTitle)}</small></div><small>{item.completed ? 'Concluído' : `Parou em ${formatDuration(item.positionSeconds)}`}</small>{:else}<span>Mídia não disponível no catálogo</span>{/if}</li>{/each}</ul></section>{/if}
	{#if canManage || (catalog && catalog.nodes.length)}
		<div class="home-toolbar">
			{#if catalog && catalog.nodes.length}<form class="catalog-search" role="search" onsubmit={(event) => event.preventDefault()}><label for="catalog-search-input">Buscar nos cursos</label><input id="catalog-search-input" type="search" placeholder="Nome do curso" bind:value={catalogQuery} /></form>{/if}
			{#if canManage}
				<div class="home-actions">
					{#if transportKind === 'desktop'}<button class="primary" onclick={importCourse} disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar curso'}</button>{:else}<button class="primary" onclick={() => (importFormOpen = !importFormOpen)} aria-expanded={importFormOpen}>Importar curso</button>{/if}
					<button class="text-button" onclick={rescan} disabled={rescanning || !catalog || !catalog.nodes.length}>{rescanning ? 'Indexando…' : 'Indexar cursos'}</button>
				</div>
			{/if}
		</div>
	{/if}
	{#if !canManage && catalog && catalog.nodes.length === 0}<p class="empty" role="status">Nenhum curso disponível. Peça ao administrador para importar um curso.</p>{/if}
	{#if user.role === 'admin' && importFormOpen}<form class="import-form" onsubmit={submitImport}><label for="import-path">Caminho da pasta do curso no servidor</label><input id="import-path" bind:value={importPath} placeholder="C:\cursos\Meu curso" required /><div class="import-form-actions"><button class="primary" disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar'}</button><button type="button" class="text-button" onclick={() => (importFormOpen = false)}>Cancelar</button></div></form>{/if}
	{#if importError}<p class="error" role="alert">{importError}</p>{/if}
	{#if user.role === 'admin' && rescanMessage}<p class="catalog-message" aria-live="polite">{rescanMessage}</p>{/if}
	{#if catalog && catalog.nodes.length}
	<h2 id="courses-heading" class="courses-heading">Cursos</h2>
	{#if filteredNodes.length}<ul class="course-grid" aria-labelledby="courses-heading">{#each filteredNodes as courseItem (courseItem.courseId ?? courseItem.path)}<li><CourseCard course={courseItem} /></li>{/each}</ul>
		{:else}<p class="empty" role="status">Nenhum curso corresponde a “{catalogQuery.trim()}”.</p>{/if}
	{:else if canManage}<div class="empty-state"><p class="empty">Nenhum curso por aqui ainda.</p><p class="intro">Clique em “Importar curso” e escolha a pasta de um curso. Nada é movido, alterado ou apagado — a pasta permanece somente leitura.</p></div>{/if}
	{#if catalog && catalog.looseFiles.length}<LooseFiles files={catalog.looseFiles} heading="Arquivos soltos" />{/if}
{:else if route === 'settings'}<h1>Preferências</h1><p class="intro">Escolha como o LearnVault aparece neste dispositivo.</p><fieldset class="theme-options"><legend>Tema</legend><label><input type="radio" name="theme" value="system" checked={themeMode === 'system'} onchange={() => changeTheme('system')} />Sistema <span>Acompanha a preferência do sistema.</span></label><label><input type="radio" name="theme" value="light" checked={themeMode === 'light'} onchange={() => changeTheme('light')} />Claro <span>Usa sempre o tema claro.</span></label><label><input type="radio" name="theme" value="dark" checked={themeMode === 'dark'} onchange={() => changeTheme('dark')} />Escuro <span>Usa sempre o tema escuro.</span></label></fieldset><p class="theme-status" aria-live="polite">Tema atual: {resolvedTheme === 'dark' ? 'escuro' : 'claro'}{themeMode === 'system' ? ' (sistema)' : ''}</p><section class="account-settings" aria-labelledby="account-heading"><h2 id="account-heading">Conta</h2><p><strong>{user.username}</strong> · {user.isAdmin ? 'Administrador' : 'Estudante'}</p><form class="credentials-form" onsubmit={credentials}><fieldset class="auth-fields" disabled={authBusy}><legend>Credenciais de acesso</legend><PasswordField id="settings-current-password" label="Senha atual" autocomplete="current-password" bind:value={currentPassword} /><label class="form-field" for="settings-new-username">Novo usuário<input id="settings-new-username" autocomplete="username" bind:value={username} required /></label><PasswordField id="settings-new-password" label="Nova senha" autocomplete="new-password" bind:value={password} minLength={12} hint="Use ao menos 12 caracteres." />{#if accountError}<p class="error" role="alert">{accountError}</p>{/if}{#if accountStatus}<p class="catalog-message" role="status">{accountStatus}</p>{/if}<button type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Salvando…' : 'Atualizar credenciais'}</button></fieldset></form>{#if transportKind === 'http'}<button class="text-button" onclick={logout}>Encerrar sessão</button>{:else}<p class="catalog-message">Biblioteca local neste dispositivo.</p>{/if}</section>
{:else if route === 'course'}
	<a class="back" href={href('home')}>← Início</a>
	{#if !course}<h1>Curso não encontrado</h1><p class="empty" role="status">Não foi possível encontrar este curso no catálogo atual. Ele pode ter sido removido.</p>
	{:else}
		<header class="course-page-header"><h1>{course.title}</h1>{#if courseMeta}<p class="course-meta">{courseMeta}</p>{/if}</header>
		{#if canManage}<div class="course-actions">{#if removeConfirm}<span class="remove-question" role="alert">Remover “{course.title}” da biblioteca? Os arquivos não serão apagados.</span><button class="text-button danger" onclick={removeCourse} disabled={removing} aria-busy={removing}>{removing ? 'Removendo…' : 'Remover curso'}</button><button class="text-button" onclick={() => (removeConfirm = false)} disabled={removing}>Cancelar</button>{:else}<button class="text-button" onclick={() => (removeConfirm = true)}>Remover curso</button><button class="text-button" onclick={rescan} disabled={rescanning}>{rescanning ? 'Indexando…' : 'Indexar cursos'}</button>{/if}</div>{/if}
		{#if removeError}<p class="error" role="alert">{removeError}</p>{/if}
		{#if rescanMessage}<p class="catalog-message" aria-live="polite">{rescanMessage}</p>{/if}
		{#if course.looseFiles.length}<LooseFiles files={course.looseFiles} heading="Arquivos soltos" courseId={course.courseId ?? course.path} />{/if}
		{#if course.children.length}<CatalogTree nodes={course.children} courseId={course.courseId ?? course.path} />{:else}<p class="empty" role="status">Este curso ainda não foi indexado. Use “Indexar cursos” para montar o catálogo.</p>{/if}
	{/if}
{:else}
	{#if !lesson}<h1>Aula não encontrada</h1><p class="empty" role="status">Não foi possível encontrar esta aula no catálogo atual.</p>{:else}
		<a class="back" href={course ? href('course', course.courseId ?? course.path) : href('home')}>← {course ? course.title : 'Início'}</a>
		<h1>{lesson.title}</h1>
		<Player {lesson} courseId={courseId ?? ''} {mediaError} onMediaError={mediaFailed} {studyEnabled} />
		{#if lessonCount > 1}<nav class="lesson-index" aria-label="Índice de aulas"><h2>Índice de aulas</h2><div class="lesson-index-body">{#each sections as section}{#if section.title}<h3>{section.title}</h3>{/if}<ol>{#each section.lessons as lessonItem}<li><a class:active={lessonItem.path === lesson.path} href={href('player', courseId ?? '', lessonItem.path)} aria-current={lessonItem.path === lesson.path ? 'page' : undefined} use:revealActive={lessonItem.path === lesson.path}>{lessonItem.title}</a></li>{/each}</ol>{/each}</div></nav>{/if}
		{#if neighbors.previous || neighbors.next}<nav class="lesson-nav" aria-label="Aulas vizinhas">{#if neighbors.previous}<a href={href('player', courseId ?? '', neighbors.previous.path)}>← {neighbors.previous.title}</a>{/if}{#if neighbors.next}<a href={href('player', courseId ?? '', neighbors.next.path)}>{neighbors.next.title} →</a>{/if}</nav>{/if}
	{/if}
{/if}
</main></div></div>{/if}
