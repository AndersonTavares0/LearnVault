<script lang="ts">
	import { onMount } from 'svelte';
	import LooseFiles from './LooseFiles.svelte';
	import CatalogTree from './CatalogTree.svelte';
	import { filterCatalog } from './search';
	import Player from './Player.svelte';
	import PasswordField from './PasswordField.svelte';
	import { lessonSections, neighboringLessons, type LessonSection } from './navigation';
	import { destinationAfterLogin, findLesson, findLessonByMediaId, href, locationFromHash, type Route } from './router';
	import { STUDY_NOT_AVAILABLE, transport, transportKind, type Catalog, type CatalogNode, type Lesson, type StudyProgress, type User } from './transport';
	import { desktopLocalUser, initialView, resolveSession, type ConfigState } from './session';
	import { formatDuration, studySyncSupported } from './player';
	import Brand from './Brand.svelte';
	import { applyTheme, persistThemeMode, readThemeMode, resolveTheme, type ThemeMode } from './theme';

	let themeMode: ThemeMode = readThemeMode(safeLocalStorage());
	let systemDark = window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;
	let resolvedTheme = resolveTheme(themeMode, systemDark);
	let route: Route = locationFromHash(location.hash).route;
	let lessonPath: string | null = locationFromHash(location.hash).lessonPath;
	let user: User | null = null;
	let catalog: Catalog | null = null;
	let lesson: Lesson | null = null;
	let config: ConfigState = 'unknown';
	let catalogQuery = '';
	let filteredNodes: CatalogNode[] = [];
	let sections: LessonSection[] = [];
	$: filteredNodes = catalog ? filterCatalog(catalog.nodes, catalogQuery) : [];
	// Navegação e índice saem da árvore canônica do catálogo. Quando uma reindexação
	// remove a aula aberta, `neighboringLessons` devolve os dois lados nulos e os
	// controles somem em vez de apontar para uma aula que não existe mais.
	$: neighbors = neighboringLessons(catalog?.nodes ?? [], lessonPath);
	$: sections = lessonSections(catalog?.nodes ?? []);
	$: lessonCount = sections.reduce((total, section) => total + section.lessons.length, 0);
	let loading = true;
	let authBusy = false;
	let error = '';
	let username = '';
	let password = '';
	let currentPassword = '';
	let rescanning = false;
	let rescanMessage = '';
	let mediaError = '';
	let choosing = false;
	let libraryError = '';
	let recent: StudyProgress[] = [];
	let recentLoading = false;
	let studyEnabled = studySyncSupported;

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
			lessonPath = next.lessonPath;
			lesson = findLesson(catalog, lessonPath);
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
			const session = await resolveSession(() => transport.me(), () => transport.config(), transportKind);
			user = session.user;
			config = session.config;
			error = session.error;
			if (user && !user.mustChangeCredentials) {
				catalog = await transport.catalog();
				lesson = findLesson(catalog, lessonPath);
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
			if (!user.mustChangeCredentials) {
				catalog = await transport.catalog();
				lesson = findLesson(catalog, lessonPath);
				await loadRecent();
				location.hash = destinationAfterLogin(destination);
			}
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível entrar.';
		} finally {
			authBusy = false;
		}
	}

	async function credentials(event: SubmitEvent) {
		event.preventDefault();
		if (authBusy) return;
		error = '';
		authBusy = true;
		try {
			user = await transport.updateCredentials(username, password, currentPassword);
			username = '';
			password = '';
			currentPassword = '';
			catalog = await transport.catalog();
			lesson = findLesson(catalog, lessonPath);
			await loadRecent();
			location.hash = destinationAfterLogin(location.hash);
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível atualizar as credenciais.';
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

	function syncLesson(nextCatalog: Catalog) {
		catalog = nextCatalog;
		lesson = findLesson(nextCatalog, lessonPath);
		mediaError = '';
	}

	async function rescan() {
		rescanning = true;
		rescanMessage = 'Lendo a biblioteca…';
		try {
			await transport.rescan();
			syncLesson(await transport.catalog());
			await loadRecent();
			rescanMessage = 'Catálogo atualizado.';
		} catch (cause) {
			rescanMessage = cause instanceof Error ? cause.message : 'Falha ao indexar.';
		} finally {
			rescanning = false;
		}
	}

	// O webview só dispara o seletor nativo; nenhum caminho cruza o IPC. Cancelar preserva
	// a configuração anterior, e uma falha é recuperável — o botão volta a ficar ativo.
	async function chooseLibrary() {
		choosing = true;
		libraryError = '';
		try {
			const status = await transport.chooseLibrary();
			config = status.configured ? 'configured' : 'unconfigured';
			if (transportKind === 'desktop' && config === 'configured') {
				catalog = null;
				lesson = null;
				recent = [];
				mediaError = '';
				await transport.rescan();
				const nextCatalog = await transport.catalog();
				syncLesson(nextCatalog);
				await loadRecent();
				user = desktopLocalUser;
			}
		} catch (cause) {
			libraryError = cause instanceof Error ? cause.message : 'Não foi possível configurar a biblioteca.';
		} finally {
			choosing = false;
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
{#if loading}<div class="loading" aria-live="polite">Carregando sua biblioteca…</div>
{:else if initialView(user, config, transportKind) === 'offline'}<main class="login-page"><div class="brand"><Brand /> LEARNVAULT</div><section><p class="eyebrow">{config === 'configured' ? 'BIBLIOTECA CONFIGURADA' : 'MODO OFFLINE'}</p><h1>{config === 'configured' ? 'Biblioteca pronta' : 'Biblioteca não configurada'}</h1>{#if config === 'configured'}<p class="intro">A pasta da biblioteca foi salva. Indexe quando quiser para montar o catálogo — nada na pasta é movido, alterado ou apagado.</p>{:else}<p class="intro">Escolha a pasta que contém seus cursos. Nada é movido, alterado ou apagado — a biblioteca permanece somente leitura.</p>{/if}{#if transportKind === 'desktop'}{#if config === 'configured'}<button class="primary" onclick={rescan} disabled={rescanning} aria-busy={rescanning}>{rescanning ? 'Indexando…' : 'Indexar biblioteca'}</button><p class="catalog-message" aria-live="polite">{rescanMessage}</p>{/if}<button class="primary" onclick={chooseLibrary} disabled={choosing} aria-busy={choosing}>{choosing ? 'Abrindo o seletor…' : config === 'configured' ? 'Trocar pasta da biblioteca' : 'Escolher pasta da biblioteca'}</button>{#if libraryError}<p class="error" role="alert">{libraryError}</p>{/if}{:else}<p class="intro">A configuração da biblioteca é feita no servidor.</p>{/if}</section></main>
{:else if !user}<main class="login-page"><div class="brand"><Brand /> LEARNVAULT</div><section><p class="eyebrow">SEU ESPAÇO DE APRENDIZAGEM</p><h1>Conhecimento,<br /><em>no seu ritmo.</em></h1><p class="intro">Uma casa tranquila para seus cursos, ideias e próximos passos.</p><form onsubmit={login}><fieldset class="auth-fields" disabled={authBusy}><label class="form-field" for="login-username">Usuário<input id="login-username" autocomplete="username" bind:value={username} required /></label><PasswordField id="login-password" label="Senha" autocomplete="current-password" bind:value={password} />{#if error}<p class="error" role="alert">{error}</p>{/if}<button type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Entrando…' : 'Entrar'}</button></fieldset></form></section></main>
{:else if user.mustChangeCredentials}<main class="login-page"><div class="brand"><Brand /> LEARNVAULT</div><section><h1>Configure sua conta</h1><p>Defina seu usuário e sua senha para continuar.</p><form onsubmit={credentials}><fieldset class="auth-fields" disabled={authBusy}><PasswordField id="current-password" label="Senha atual" autocomplete="current-password" bind:value={currentPassword} /><label class="form-field" for="new-username">Novo usuário<input id="new-username" autocomplete="username" bind:value={username} required /></label><PasswordField id="new-password" label="Nova senha" autocomplete="new-password" bind:value={password} minLength={12} hint="Use ao menos 12 caracteres." />{#if error}<p class="error" role="alert">{error}</p>{/if}<button type="submit" disabled={authBusy} aria-busy={authBusy}>{authBusy ? 'Salvando…' : 'Continuar'}</button></fieldset></form></section></main>
{:else}<div class="shell"><header><div class="brand"><Brand /> LEARNVAULT</div><button class="text-button" onclick={logout}>Sair</button></header><div class="layout"><aside aria-label="Navegação principal"><a class:active={route === 'home'} href={href('home')}>Início</a><a class:active={route === 'catalog'} href={href('catalog')}>Catálogo</a><a class:active={route === 'settings'} href={href('settings')}>Configurações</a></aside><main><p class="eyebrow">BIBLIOTECA PESSOAL</p>{#if error}<p class="error" role="alert">{error}</p>{/if}{#if route === 'home'}<h1>Olá, {user.username}.</h1><p class="intro">Sua biblioteca está protegida e pronta para estudar.</p>
{#if studySyncSupported && recent.length}<section class="recent-study" aria-labelledby="recent-heading"><h2 id="recent-heading">Continuar assistindo</h2><ul>{#each recent as item}{@const recentLesson = findLessonByMediaId(catalog, item.mediaId)}<li>{#if recentLesson}<a href={href('player', recentLesson.path)}>{recentLesson.title}</a><small>{item.completed ? 'Concluído' : `Parou em ${formatDuration(item.positionSeconds)}`}</small>{:else}<span>Mídia não disponível no catálogo</span>{/if}</li>{/each}</ul></section>{/if}
<a class="primary" href={href('catalog')}>Abrir catálogo</a>
{:else if route === 'settings'}<h1>Preferências</h1><p class="intro">Escolha como o LearnVault aparece neste dispositivo.</p><fieldset class="theme-options"><legend>Tema</legend><label><input type="radio" name="theme" value="system" checked={themeMode === 'system'} onchange={() => changeTheme('system')} />Sistema <span>Acompanha a preferência do sistema.</span></label><label><input type="radio" name="theme" value="light" checked={themeMode === 'light'} onchange={() => changeTheme('light')} />Claro <span>Usa sempre o tema claro.</span></label><label><input type="radio" name="theme" value="dark" checked={themeMode === 'dark'} onchange={() => changeTheme('dark')} />Escuro <span>Usa sempre o tema escuro.</span></label></fieldset><p class="theme-status" aria-live="polite">Tema atual: {resolvedTheme === 'dark' ? 'escuro' : 'claro'}{themeMode === 'system' ? ' (sistema)' : ''}</p><section class="account-settings" aria-labelledby="account-heading"><h2 id="account-heading">Conta</h2><p><strong>{user.username}</strong> · {user.isAdmin ? 'Administrador' : 'Estudante'}</p>{#if transportKind === 'http'}<button class="text-button" onclick={logout}>Encerrar sessão</button>{:else}<p class="catalog-message">Biblioteca local neste dispositivo.</p><button class="text-button" onclick={chooseLibrary} disabled={choosing}>{choosing ? 'Abrindo o seletor…' : 'Trocar pasta da biblioteca'}</button>{#if libraryError}<p class="error" role="alert">{libraryError}</p>{/if}{/if}</section>
{:else if route === 'catalog'}<h1>Seus cursos</h1>{#if (catalog && catalog.nodes.length) || user.role === 'admin'}<div class="catalog-toolbar">{#if catalog && catalog.nodes.length}<form class="catalog-search" role="search" onsubmit={(event) => event.preventDefault()}><label for="catalog-search-input">Buscar no catálogo</label><input id="catalog-search-input" type="search" placeholder="Curso, módulo ou aula" bind:value={catalogQuery} /></form>{/if}{#if user.role === 'admin'}<button class="primary" onclick={rescan} disabled={rescanning}>{rescanning ? 'Indexando…' : 'Indexar biblioteca'}</button>{/if}</div>{/if}{#if user.role === 'admin'}<p class="catalog-message" aria-live="polite">{rescanMessage}</p>{/if}{#if catalog && catalog.looseFiles.length}<LooseFiles files={catalog.looseFiles} heading="Arquivos soltos" />{/if}{#if catalog && catalog.nodes.length}{#if filteredNodes.length}<CatalogTree nodes={filteredNodes} expandAll={catalogQuery.trim().length > 0} />{:else}<p class="empty" role="status">Nenhum resultado para “{catalogQuery.trim()}”.</p>{/if}{:else if !catalog || !catalog.looseFiles.length}<p class="empty">O catálogo ainda está vazio.</p>{/if}{:else}<h1>{lesson?.title ?? 'Aula não encontrada'}</h1>{#if !lesson}<p class="empty" role="status">Não foi possível encontrar esta aula no catálogo atual.</p>{:else}<Player lesson={lesson} mediaError={mediaError} onMediaError={mediaFailed} {studyEnabled} />{#if lessonCount > 1}<nav class="lesson-index" aria-label="Índice de aulas"><h2>Índice de aulas</h2><div class="lesson-index-body">{#each sections as section}{#if section.title}<h3>{section.title}</h3>{/if}<ol>{#each section.lessons as lessonItem}<li><a class:active={lessonItem.path === lesson.path} href={href('player', lessonItem.path)} aria-current={lessonItem.path === lesson.path ? 'page' : undefined} use:revealActive={lessonItem.path === lesson.path}>{lessonItem.title}</a></li>{/each}</ol>{/each}</div></nav>{/if}{#if neighbors.previous || neighbors.next}<nav class="lesson-nav" aria-label="Navegação entre aulas">{#if neighbors.previous}<a href={href('player', neighbors.previous.path)} rel="prev">← {neighbors.previous.title}</a>{/if}{#if neighbors.next}<a href={href('player', neighbors.next.path)} rel="next">{neighbors.next.title} →</a>{/if}</nav>{/if}{/if}<a class="back" href={href('catalog')}>Voltar ao catálogo</a>{/if}</main></div></div>{/if}
