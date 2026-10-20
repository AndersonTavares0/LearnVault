<script lang="ts">
	import { onMount } from 'svelte';
	import LooseFiles from './LooseFiles.svelte';
	import CatalogTree from './CatalogTree.svelte';
	import CourseCard from './CourseCard.svelte';
	import Player from './Player.svelte';
	import Pomodoro from './Pomodoro.svelte';
	import { filterCatalog } from './search';
	import { countCatalogNode, formatCourseMeta, formatLocationContext } from './catalog';
	import PasswordField from './PasswordField.svelte';
	import { lessonSections, neighboringLessons, type LessonSection } from './navigation';
	import { destinationAfterLogin, findCourse, findLesson, findLessonLocation, href, locationFromHash, type Route } from './router';
 import { courseIdOf, isCoverFile, transport, transportKind, STUDY_NOT_AVAILABLE, type Catalog, type CatalogNode, type Lesson, type StudyAnalytics, type StudyProgress, type User } from './transport';
	import { defaultRoute, resolveSession } from './session';
	import { formatDuration, studySyncSupported } from './player';
	import ImportPreview from './ImportPreview.svelte';
	import { applyTheme, persistThemeMode, readThemeMode, resolveTheme, type ThemeMode } from './theme';
	import { courseCover } from './cover';

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
	$: courseCounts = course ? countCatalogNode(course) : null;
	$: courseCoverArt = course ? courseCover(course.title) : null;
	$: courseCoverFile = course?.looseFiles.find(isCoverFile) ?? null;
	$: progressCourseIds = new Set(recent.map((item) => findLessonLocation(catalog, item.mediaId)?.courseId).filter((id): id is string => Boolean(id)));
	$: visibleCourses = [...filteredNodes]
		.filter((node) => courseFilter === 'all' || progressCourseIds.has(courseIdOf(node)))
		.sort((left, right) => courseSort === 'title' ? left.title.localeCompare(right.title, 'pt-BR') : recentRank(left) - recentRank(right));
	$: courseRecent = course
		? recent.find((item) => findLessonLocation(catalog, item.mediaId)?.courseId === courseIdOf(course!)) ?? null
		: null;
	$: courseResumeLocation = courseRecent ? findLessonLocation(catalog, courseRecent.mediaId) : null;

	function recentRank(node: CatalogNode): number {
		const id = courseIdOf(node);
		const index = recent.findIndex((item) => findLessonLocation(catalog, item.mediaId)?.courseId === id);
		return index === -1 ? Number.MAX_SAFE_INTEGER : index;
	}

	// A referência mostra a data relativa no resumo do curso. O carimbo vem do
	// progresso real (segundos Unix); sem registro, a interface não inventa atividade.
	function activityLabel(item: StudyProgress | null): string {
		if (!item?.updatedAt) return 'Sem registro';
		const date = new Date(item.updatedAt * 1000);
		const days = Math.floor((Date.now() - date.getTime()) / 86_400_000);
		if (days <= 0) return 'Hoje';
		if (days === 1) return 'Ontem';
		return date.toLocaleDateString('pt-BR', { day: '2-digit', month: 'short' });
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
	let importPreview: import('./transport').ImportPreview | null = null;
	let previewingImport = false;
	let removing = false;
	let removeConfirm = false;
	let removeError = '';
	let organizingCatalog = false;
	let catalogOrderSaving = false;
	let catalogOrderMessage = '';
	let catalogOrderError = false;
	let mediaError = '';
	let recent: StudyProgress[] = [];
	let studyAnalytics: StudyAnalytics | null = null;
	let recentLoading = false;
	let studyEnabled = studySyncSupported;
	let courseFilter: 'all' | 'progress' = 'all';
	let courseSort: 'recent' | 'title' = 'recent';
	let playerTab: 'curriculum' | 'notes' | 'materials' = 'curriculum';
	let panelCollapsed = false;
	let libraryActionsOpen = false;
	let expandAllModules = true;

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
			if (themeColor) themeColor.content = resolvedTheme === 'dark' ? '#14232b' : '#f2f5f7';
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
			organizingCatalog = false;
			catalogOrderMessage = '';
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
		if (themeColor) themeColor.content = resolvedTheme === 'dark' ? '#14232b' : '#f2f5f7';
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
		if (!importPath.trim()) return;
		previewingImport = true;
		importError = '';
		try {
			importPreview = await transport.previewImport(importPath.trim());
		} catch (cause) {
			importError = cause instanceof Error ? cause.message : 'Não foi possível importar o curso.';
		} finally {
			previewingImport = false;
		}
	}

	function cancelImportPreview() { importPreview = null; }

	async function confirmImport() {
		if (!importPath.trim() || !importPreview) return;
		importing = true;
		importError = '';
		try {
			await transport.importCoursePath(importPath.trim());
			importPath = '';
			importPreview = null;
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

	async function saveCatalogOrder(parentPath: string, childPaths: string[]) {
		if (!course) return;
		catalogOrderSaving = true;
		catalogOrderMessage = '';
		catalogOrderError = false;
		try {
			await transport.saveCatalogOrder(course.courseId ?? course.path, parentPath, childPaths);
			syncCatalog(await transport.catalog());
			catalogOrderMessage = 'Ordem salva.';
		} catch (cause) {
			catalogOrderError = true;
			catalogOrderMessage = cause instanceof Error ? cause.message : 'Não foi possível salvar a ordem.';
		} finally {
			catalogOrderSaving = false;
		}
	}

	async function resetCatalogOrder(parentPath: string) {
		if (!course || !window.confirm('Restaurar a ordem automática desta lista?')) return;
		catalogOrderSaving = true;
		catalogOrderMessage = '';
		catalogOrderError = false;
		try {
			await transport.resetCatalogOrder(course.courseId ?? course.path, parentPath);
			syncCatalog(await transport.catalog());
			catalogOrderMessage = 'Ordem automática restaurada.';
		} catch (cause) {
			catalogOrderError = true;
			catalogOrderMessage = cause instanceof Error ? cause.message : 'Não foi possível restaurar a ordem.';
		} finally {
			catalogOrderSaving = false;
		}
	}

	async function loadRecent() {
		if (!studySyncSupported) return;
		recentLoading = true;
		try {
			const [result, analytics] = await Promise.all([transport.recentStudy(6), transport.studyAnalytics()]);
			recent = result.items;
			studyAnalytics = analytics;
		} catch (cause) {
			recent = [];
			studyAnalytics = null;
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
<main class="board login-board" aria-labelledby="login-heading"><section class="screen active"><div class="login">
<section class="login-identity"><div class="brand">Learn<i>Vault</i></div><div><h1>Seu próximo capítulo começa de onde você parou.</h1><p>Entre no seu ambiente de aprendizagem local.</p></div><span class="small">Seus cursos ficam no seu dispositivo ou servidor.</span></section>
<form class="login-card" onsubmit={login}>
<h2 id="login-heading">Entrar</h2>
<p class="small">Use as credenciais da sua instalação.</p>
				<fieldset class="auth-fields" disabled={authBusy}>
<label for="login-username">Usuário</label><input class="field" id="login-username" autocomplete="username" autocapitalize="none" spellcheck={false} placeholder="Seu usuário" bind:value={username} required />
					<PasswordField id="login-password" label="Senha" autocomplete="current-password" placeholder="Sua senha" bind:value={password} />
					{#if error}<p class="error" role="alert">{error}</p>{/if}
<button class="btn primary" type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Entrando…' : 'Entrar no LearnVault'}</button>
				</fieldset>
</form>
</div></section></main>
{:else}<div class="app">{#if route !== 'player'}<header class="top"><a class="brand" href={href('home')}>Learn<i>Vault</i></a><nav class="globalnav" aria-label="Navegação principal"><a class:on={route === 'home' || route === 'course'} href={href('home')}>Aprender</a><a class:on={route === 'notes'} href={href('notes')}>Anotações</a>{#if route === 'analytics'}<a class:on={route === 'analytics'} href={href('analytics')}>Estatísticas</a>{/if}<a class:on={route === 'settings'} href={href('settings')}>Configurações</a></nav><form class="search-form" role="search" onsubmit={(event) => { event.preventDefault(); location.hash = href('home'); }}><span aria-hidden="true">⌕</span><input class="search" aria-label="Buscar cursos" placeholder="Buscar cursos" bind:value={catalogQuery} /></form>{#if studySyncSupported}<Pomodoro />{/if}{#if transportKind === 'http'}<button class="user" type="button" title="Sair" aria-label="Sair" onclick={logout}>{(user?.username ?? '?').slice(0, 1).toUpperCase()}</button>{:else}<span class="user" title={user?.username ?? 'Local'}>{(user?.username ?? '?').slice(0, 1).toUpperCase()}</span>{/if}</header>{/if}<main class="route">{#if error}<div class="wrap"><p class="error" role="alert">{error}</p></div>{/if}
{#if route === 'home'}
<div class="wrap">
	<div class="row"><div><div class="eyebrow">Seu espaço de estudo</div><h1 class="title">Meus cursos</h1><p class="sub">Retome uma aula diretamente ou escolha outro curso.</p></div><div class="tools"><button class="btn" type="button" onclick={() => (courseSort = courseSort === 'recent' ? 'title' : 'recent')} aria-label="Ordenar cursos">{courseSort === 'recent' ? 'Recentes' : 'Título'}⌄</button><button class="btn" type="button" onclick={() => (courseFilter = courseFilter === 'all' ? 'progress' : 'all')} aria-pressed={courseFilter === 'progress'}>Filtros</button></div></div>
	{#if studySyncSupported && recent.length}<div class="row"><h2 class="section-head">Continuar estudando</h2><span class="small">{recent.length} {recent.length === 1 ? 'aula' : 'aulas'} em andamento · atividade recente</span></div><div class="resume-list">{#each recent as item, index}{@const location = findLessonLocation(catalog, item.mediaId)}<article class="resume-item">{#if location}<div class="cover {index % 3 === 1 ? 'alt' : index % 3 === 2 ? 'violet' : ''}" aria-hidden="true">{courseCover(location.courseTitle).monogram}</div><div><h3><a href={href('player', location.courseId, location.lesson.path)}>{location.courseTitle}</a></h3><p>{location.lesson.title} · {formatLocationContext(location.courseTitle, location.moduleTitle)} · {item.completed ? 'aula concluída' : `parou em ${formatDuration(item.positionSeconds)}`}</p>{#if item.completed}<div class="progress" aria-hidden="true"><b style="width:100%"></b></div>{/if}</div><a class="btn primary" href={href('player', location.courseId, location.lesson.path)}>{item.completed ? 'Revisar aula' : 'Retomar aula'}</a>{:else}<span class="empty">Mídia não disponível no catálogo</span>{/if}</article>{/each}</div>{/if}
	{#if !canManage && catalog && catalog.nodes.length === 0}<p class="empty" role="status">Nenhum curso disponível. Peça ao administrador para importar um curso.</p>{/if}
	{#if catalog && catalog.nodes.length}
	<div class="row"><h2 class="section-head">Todos os cursos <span class="small">{filteredNodes.length}</span></h2><div class="tools"><button class="btn" type="button" onclick={() => (courseFilter = 'all')} aria-pressed={courseFilter === 'all'}>Tudo⌄</button><button class="btn" type="button" onclick={() => (courseFilter = 'progress')} aria-pressed={courseFilter === 'progress'}>Em andamento⌄</button></div></div>
	{#if visibleCourses.length}<div class="grid">{#each visibleCourses as courseItem, index (courseItem.courseId ?? courseItem.path)}<CourseCard course={courseItem} variant={index % 3 === 1 ? 'alt' : index % 3 === 2 ? 'violet' : ''} />{/each}</div>
		{:else}<p class="empty" role="status">{catalogQuery.trim() ? `Nenhum curso corresponde a “${catalogQuery.trim()}”.` : 'Nenhum curso em andamento.'}</p>{/if}
	{:else if canManage}<div class="empty-state"><p class="empty">Nenhum curso por aqui ainda.</p><p class="intro">Use “Ações da biblioteca” e escolha a pasta de um curso. Nada é movido, alterado ou apagado — a pasta permanece somente leitura.</p></div>{/if}
	{#if catalog && catalog.looseFiles.length}<LooseFiles files={catalog.looseFiles} heading="Arquivos soltos" />{/if}
	{#if importError}<p class="error" role="alert">{importError}</p>{/if}
	{#if importPreview}<ImportPreview preview={importPreview} onCancel={cancelImportPreview} onConfirm={confirmImport} busy={importing} />{/if}
	<div class="admin"><span>Biblioteca local atualizada há pouco</span>{#if canManage}<button class="btn" type="button" onclick={() => (libraryActionsOpen = !libraryActionsOpen)} aria-expanded={libraryActionsOpen}>Ações da biblioteca⌄</button>{/if}</div>
	{#if canManage && libraryActionsOpen}<div class="library-actions"><div class="library-actions-row">{#if transportKind === 'desktop'}<button class="btn primary" onclick={importCourse} disabled={importing} aria-busy={importing}>{importing ? 'Importando…' : 'Importar curso'}</button>{:else}<button class="btn primary" onclick={() => (importFormOpen = !importFormOpen)} aria-expanded={importFormOpen}>Importar curso</button>{/if}<button class="btn" onclick={rescan} disabled={rescanning || !catalog || !catalog.nodes.length}>{rescanning ? 'Indexando…' : 'Indexar cursos'}</button>{#if user.role === 'admin' && rescanMessage}<span class="small" aria-live="polite">{rescanMessage}</span>{/if}</div>{#if user.role === 'admin' && importFormOpen && !importPreview}<form class="import-form" onsubmit={submitImport}><label for="import-path">Caminho da pasta do curso no servidor</label><input id="import-path" bind:value={importPath} placeholder="C:\cursos\Meu curso" required /><div class="import-form-actions"><button class="btn primary" disabled={previewingImport || importing} aria-busy={previewingImport}>{previewingImport ? 'Analisando…' : 'Analisar pasta'}</button><button type="button" class="text-button" onclick={() => (importFormOpen = false)}>Cancelar</button></div></form>{/if}</div>{/if}
</div>
{:else if route === 'notes'}<div class="wrap"><div class="eyebrow">Seu material de estudo</div><h1 class="title">Anotações</h1><p class="sub">Notas privadas, organizadas por curso e ligadas ao momento da aula.</p><div class="notes-layout"><aside class="filter" aria-label="Filtros de anotações"><h3>Refinar notas</h3><label for="note-course">Curso</label><select class="field" id="note-course" disabled><option>Todos os cursos</option></select><label for="note-period">Período</label><select class="field" id="note-period" disabled><option>Qualquer data</option></select><label for="note-order">Ordenar por</label><select class="field" id="note-order" disabled><option>Mais recentes</option></select><label for="note-search">Buscar no texto</label><input class="field" id="note-search" type="search" placeholder="Buscar no texto" disabled /><p class="small">Os filtros serão ativados junto com as anotações persistentes.</p></aside><div><div class="row" style="margin-bottom:13px"><strong>0 notas</strong><span class="small">Seu progresso e materiais continuam preservados.</span></div><div class="analytics-card notes-empty"><strong>As anotações por aula ainda não estão disponíveis nesta versão.</strong><p class="sub">O painel do player já reserva o espaço para elas. Nenhuma nota fictícia é exibida enquanto a persistência não estiver implementada.</p><a class="btn" href={href('home')}>Voltar aos cursos</a></div></div></div></div>
{:else if route === 'analytics'}<div class="analytics"><div class="wrap"><div class="analytics-head"><div><div class="eyebrow">Seu ritmo de aprendizagem</div><h1 class="title">Estatísticas de estudo</h1><p class="sub">Dados pessoais desta instalação; sem comparações ou metas.</p></div></div><div class="tracking-note"><strong>Registro local do LearnVault.</strong> Os totais refletem apenas o progresso salvo neste banco; não incluem atividade externa.</div>{#if studyAnalytics}<div class="metric-strip"><div class="metric"><small>Aulas acompanhadas</small><strong>{studyAnalytics.trackedCount}</strong><em>com progresso salvo</em></div><div class="metric"><small>Concluídas</small><strong>{studyAnalytics.completedCount}</strong><em>marcações explícitas</em></div><div class="metric"><small>Tempo medido</small><strong>{formatDuration(studyAnalytics.totalPositionSeconds)}</strong><em>posição salva no catálogo</em></div><div class="metric"><small>Progresso</small><strong>{studyAnalytics.trackedCount ? Math.round((studyAnalytics.completedCount / studyAnalytics.trackedCount) * 100) : 0}%</strong><em>das aulas acompanhadas</em></div></div>{:else}<p class="empty" role="status">Estatísticas indisponíveis para esta sessão.</p>{/if}<div class="analytics-grid"><section class="analytics-card"><h3>Histórico ao longo do tempo</h3><p>Minutos de reprodução por dia.</p><p class="sub">O servidor atual expõe apenas os totais agregados. Séries diárias e gráficos históricos não estão disponíveis nesta versão.</p></section><section class="analytics-card"><h3>Progresso por curso</h3><p>Comparação entre cursos.</p><p class="sub">O contrato de analytics não detalha cursos individuais; nenhum dado é estimado aqui.</p></section></div></div></div>
{:else if route === 'settings'}<div class="wrap"><div class="eyebrow">Seu ambiente</div><h1 class="title">Configurações</h1><p class="sub">Aparência, conta e comportamento do LearnVault.</p><div class="settings-layout"><nav class="setting-nav" aria-label="Seções de configuração"><div class="sel">Aparência</div><div>Paleta</div><div>Conta e segurança</div></nav><div>
	<section class="setting-card"><h3>Tema</h3><p>Escolha a aparência. A paleta permanece selecionada ao alternar entre claro e escuro.</p><div class="modes"><label class="mode" class:selected={themeMode === 'system'}><input type="radio" name="theme" value="system" checked={themeMode === 'system'} onchange={() => changeTheme('system')} />Sistema<small>Acompanha o sistema</small></label><label class="mode" class:selected={themeMode === 'light'}><input type="radio" name="theme" value="light" checked={themeMode === 'light'} onchange={() => changeTheme('light')} />Claro<small>Sempre claro</small></label><label class="mode" class:selected={themeMode === 'dark'}><input type="radio" name="theme" value="dark" checked={themeMode === 'dark'} onchange={() => changeTheme('dark')} />Escuro<small>Sempre escuro</small></label></div><p class="small" aria-live="polite">Tema atual: {resolvedTheme === 'dark' ? 'escuro' : 'claro'}{themeMode === 'system' ? ' (sistema)' : ''}</p></section>
	<section class="setting-card"><h3>Paleta</h3><p>Prévia das cores reais aplicadas ao tema atual.</p><div class="modes"><div class="mode selected"><div class="swatches"><i></i><i></i><i></i><i></i></div><strong>Graphite</strong><div class="small">Claro e escuro</div></div></div><p class="small">Outras paletas ainda não fazem parte desta versão; nenhum preset é exibido como disponível.</p></section>
	<section class="setting-card account-settings"><h3>Conta e segurança</h3><p><strong>{user.username}</strong> · {user.isAdmin ? 'Administrador' : 'Estudante'}</p><form class="credentials-form" onsubmit={credentials}><fieldset class="auth-fields" disabled={authBusy}><legend>Credenciais de acesso</legend><PasswordField id="settings-current-password" label="Senha atual" autocomplete="current-password" bind:value={currentPassword} /><label class="form-field" for="settings-new-username">Novo usuário<input id="settings-new-username" autocomplete="username" bind:value={username} required /></label><PasswordField id="settings-new-password" label="Nova senha" autocomplete="new-password" bind:value={password} minLength={12} hint="Use ao menos 12 caracteres." />{#if accountError}<p class="error" role="alert">{accountError}</p>{/if}{#if accountStatus}<p class="catalog-message" role="status">{accountStatus}</p>{/if}<button class="btn primary" type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Salvando…' : 'Atualizar credenciais'}</button></fieldset></form>{#if transportKind === 'http'}<button class="btn" type="button" onclick={logout}>Encerrar sessão</button>{/if}</section>
</div></div></div>
{:else if route === 'course'}
	{#if !course}<h1>Curso não encontrado</h1><p class="empty" role="status">Não foi possível encontrar este curso no catálogo atual. Ele pode ter sido removido.</p>
	{:else}
		<header class="course-hero">
			<div class="wrap">
				<div>
					<div class="eyebrow course-breadcrumb"><a href={href('home')}>Meus cursos</a><span aria-hidden="true"> / </span>{course.title}</div>
					<h1>{course.title}</h1>
					<p>Curso local indexado no LearnVault; arquivos e materiais permanecem na origem.</p>
					<div class="course-stats"><span>{courseCounts?.modules ?? 0} módulos</span><span>{courseCounts?.lessons ?? 0} aulas</span><span>{courseCounts?.materials ?? 0} materiais</span>{#if courseRecent}<span>Retomado recentemente</span>{/if}</div>
				</div>
				<div class="cover" style:--cover-from={courseCoverArt?.from ?? '#20343d'} style:--cover-to={courseCoverArt?.to ?? '#276f73'} aria-hidden="true">
					{#if courseCoverFile && transport.mediaUrl(courseCoverFile, course.courseId ?? course.path)}<img src={transport.mediaUrl(courseCoverFile, course.courseId ?? course.path) ?? ''} alt="" loading="lazy" />{:else}<span>{courseCoverArt?.monogram ?? course.title.slice(0, 2).toUpperCase()}</span>{/if}
				</div>
			</div>
		</header>
		<div class="overview-grid"><div>
		<div class="row"><h2 class="section-head">Conteúdo do curso</h2><button class="btn" type="button" onclick={() => (expandAllModules = !expandAllModules)} aria-pressed={expandAllModules}>{expandAllModules ? 'Recolher módulos' : 'Expandir módulos'}</button></div>
		<div class="outline" id="course-outline">
		<div class="outline-head"><strong>{courseCounts?.modules ?? 0} módulos</strong><span class="small">· {courseCounts?.lessons ?? 0} aulas</span></div>
		{#if removeError}<p class="error" role="alert">{removeError}</p>{/if}
		{#if rescanMessage}<p class="catalog-message" aria-live="polite">{rescanMessage}</p>{/if}
		{#if catalogOrderMessage}<p class:error={catalogOrderError} class="catalog-message" role={catalogOrderError ? 'alert' : 'status'}>{catalogOrderMessage}</p>{/if}
		{#if course.looseFiles.length}<LooseFiles files={course.looseFiles} heading="Arquivos soltos" courseId={course.courseId ?? course.path} />{/if}
		{#if course.children.length}<CatalogTree nodes={course.children} courseId={course.courseId ?? course.path} expandAll={expandAllModules} currentPath={courseResumeLocation?.lesson.path ?? ''} organizing={organizingCatalog} orderSaving={catalogOrderSaving} onSaveOrder={saveCatalogOrder} onResetOrder={resetCatalogOrder} />{:else}<p class="empty" role="status">Este curso ainda não foi indexado. Use “Indexar cursos” para montar o catálogo.</p>{/if}
		{#if canManage || user}<details class="course-admin"><summary class="btn">Ações do curso</summary><div class="course-actions">{#if user}<button class="secondary" onclick={() => (organizingCatalog = !organizingCatalog)} aria-pressed={organizingCatalog}>{organizingCatalog ? 'Concluir organização' : 'Organizar aulas'}</button>{/if}{#if canManage}{#if removeConfirm}<span class="remove-question" role="alert">Remover “{course.title}” da biblioteca? Os arquivos não serão apagados.</span><button class="text-button danger" onclick={removeCourse} disabled={removing} aria-busy={removing}>{removing ? 'Removendo…' : 'Remover curso'}</button><button class="text-button" onclick={() => (removeConfirm = false)} disabled={removing}>Cancelar</button>{:else}<button class="text-button" onclick={() => (removeConfirm = true)}>Remover curso</button><button class="text-button" onclick={rescan} disabled={rescanning}>{rescanning ? 'Indexando…' : 'Indexar cursos'}</button>{/if}{/if}</div></details>{/if}
		</div>
		</div>
		<aside class="resume-box" aria-label="Seu progresso neste curso">
			<h3>Seu progresso</h3>
			{#if courseResumeLocation}
				<p class="small">{courseRecent?.completed ? 'Aula concluída · retome de onde parou' : 'Retome de onde parou'}</p>
				<a class="btn primary" href={href('player', courseResumeLocation.courseId, courseResumeLocation.lesson.path)}>Continuar estudando</a>
				<a class="btn" href="#course-outline">Abrir currículo</a>
				<div class="mini-stat"><span>Última atividade</span><strong>{activityLabel(courseRecent)}</strong></div>
				<div class="mini-stat"><span>Aula atual</span><strong>{courseResumeLocation.lesson.title}</strong></div>
			{:else}
				<p class="small">Este curso ainda não tem atividade registrada.</p>
				{#if sections[0]?.lessons[0]}<a class="btn primary" href={href('player', course.courseId ?? course.path, sections[0].lessons[0].path)}>Começar curso</a><a class="btn" href="#course-outline">Abrir currículo</a>{/if}
			{/if}
		</aside>
	</div>
	{/if}
{:else}
	{#if !lesson}<div class="wrap"><h1>Aula não encontrada</h1><p class="empty" role="status">Não foi possível encontrar esta aula no catálogo atual.</p></div>{:else}
		<div class="player-screen">
			<h1 class="visually-hidden">{lesson.title}</h1>
			<header class="player-top">
				<a class="back" href={href('home')}>‹ Meus cursos</a>
				<span class="sep" aria-hidden="true">/</span>
				<strong>{course?.title ?? lesson.title}</strong>
				<span class="grow"></span>
				<button class="player-focus" type="button" onclick={() => (panelCollapsed = !panelCollapsed)} aria-pressed={panelCollapsed}>⛶ {panelCollapsed ? 'Sair do foco' : 'Modo foco'}</button>
				{#if studySyncSupported}<Pomodoro />{/if}
				<span class="user" title={user?.username ?? 'Local'}>{(user?.username ?? '?').slice(0, 1).toUpperCase()}</span>
			</header>
			<div class:collapsed={panelCollapsed} class="player-shell">
			<section class="stage">
				<Player {lesson} courseId={courseId ?? ''} {mediaError} onMediaError={mediaFailed} {studyEnabled} materialsInPanel={!panelCollapsed} />
				<nav class="lesson-nav" aria-label="Aulas vizinhas">{#if neighbors.previous}<a href={href('player', courseId ?? '', neighbors.previous.path)}>← Aula anterior</a>{/if}{#if neighbors.next}<a href={href('player', courseId ?? '', neighbors.next.path)}>Próxima aula →</a>{/if}</nav>
				{#if panelCollapsed}<button class="panel-reopen secondary" type="button" onclick={() => (panelCollapsed = false)}>Mostrar painel</button>{/if}
			</section>
			{#if !panelCollapsed}
				<aside class="side">
					<div class="player-tools"><span class="grip" aria-hidden="true"></span><span>Painel lateral</span><button class="text-button" type="button" onclick={() => (panelCollapsed = true)}>− Recolher</button></div>
					<div class="tabs2" role="group" aria-label="Conteúdo da aula">
						<button class:active={playerTab === 'curriculum'} type="button" aria-pressed={playerTab === 'curriculum'} onclick={() => (playerTab = 'curriculum')}>Currículo</button>
						<button class:active={playerTab === 'notes'} type="button" aria-pressed={playerTab === 'notes'} onclick={() => (playerTab = 'notes')}>Anotações</button>
						<button class:active={playerTab === 'materials'} type="button" aria-pressed={playerTab === 'materials'} onclick={() => (playerTab = 'materials')}>Materiais</button>
					</div>
					{#if playerTab === 'curriculum'}
						<div class="side-head"><strong>Conteúdo do curso</strong><div>{lessonCount} {lessonCount === 1 ? 'aula' : 'aulas'} · aula atual selecionada</div></div>
						{#if lessonCount > 1}<nav class="lesson-index" aria-label="Índice de aulas"><div class="lesson-index-body">{#each sections as section}{#if section.title}<div class="mhead"><span>{section.title}</span><span>{section.lessons.length} {section.lessons.length === 1 ? 'aula' : 'aulas'}</span></div>{/if}<ol>{#each section.lessons as lessonItem}<li><a class:active={lessonItem.path === lesson.path} href={href('player', courseId ?? '', lessonItem.path)} aria-current={lessonItem.path === lesson.path ? 'page' : undefined} use:revealActive={lessonItem.path === lesson.path}>{lessonItem.title}</a></li>{/each}</ol>{/each}</div></nav>{:else}<p class="panel-empty">Esta aula não possui outras aulas no curso.</p>{/if}
					{:else if playerTab === 'notes'}
						<p class="panel-empty">As anotações por aula ainda não estão disponíveis nesta versão. Seu progresso e seus materiais continuam preservados.</p>
					{:else}
						<div class="panel-materials"><LooseFiles files={[...lesson.subtitles, ...lesson.materials]} heading="Materiais da aula" courseId={courseId ?? ''} /></div>
					{/if}
				</aside>
			{/if}
		</div>
		</div>
	{/if}
{/if}
</main></div>{/if}
