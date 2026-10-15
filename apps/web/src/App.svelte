<script lang="ts">
	import { onMount } from 'svelte';
	import { findLesson, href, locationFromHash, type Route } from './router';
	import { transport, type Catalog, type Lesson, type User } from './transport';

	let route: Route = locationFromHash(location.hash).route;
	let lessonPath: string | null = locationFromHash(location.hash).lessonPath;
	let user: User | null = null;
	let catalog: Catalog | null = null;
	let lesson: Lesson | null = null;
	let loading = true;
	let error = '';
	let username = '';
	let password = '';

	onMount(() => {
		const change = () => { const next = locationFromHash(location.hash); route = next.route; lessonPath = next.lessonPath; lesson = findLesson(catalog, lessonPath); };
		window.addEventListener('hashchange', change);
		void initialize();
		return () => window.removeEventListener('hashchange', change);
	});

	async function initialize() {
		try { user = await transport.me(); if (user) { catalog = await transport.catalog(); lesson = findLesson(catalog, lessonPath); if (route === 'login') route = 'home'; } }
		catch { error = 'Não foi possível conectar ao serviço.'; }
		finally { loading = false; }
	}
	async function login(event: SubmitEvent) {
		event.preventDefault(); error = ''; loading = true;
		try { user = await transport.login(username, password); password = ''; catalog = await transport.catalog(); location.hash = href('home'); }
		catch (cause) { error = cause instanceof Error ? cause.message : 'Não foi possível entrar.'; }
		finally { loading = false; }
	}
	async function logout() { await transport.logout(); user = null; catalog = null; lesson = null; location.hash = href('login'); }
</script>

<svelte:head><title>LearnVault — sua biblioteca de estudos</title><meta name="description" content="Sua biblioteca pessoal de cursos." /></svelte:head>

{#if loading}<div class="loading" aria-live="polite">Carregando sua biblioteca…</div>
{:else if !user}
	<main class="login-page"><div class="brand"><span class="mark">LV</span> LEARNVAULT</div><section><p class="eyebrow">SEU ESPAÇO DE APRENDIZAGEM</p><h1>Conhecimento,<br /><em>no seu ritmo.</em></h1><p class="intro">Uma casa tranquila para seus cursos, ideias e próximos passos.</p><form onsubmit={login}><label>Usuário<input autocomplete="username" bind:value={username} required /></label><label>Senha<input type="password" autocomplete="current-password" bind:value={password} required /></label>{#if error}<p class="error" role="alert">{error}</p>{/if}<button disabled={loading}>Entrar</button></form></section></main>
{:else}
	<div class="shell"><header><div class="brand"><span class="mark">LV</span> LEARNVAULT</div><button class="text-button" onclick={logout}>Sair</button></header><div class="layout"><aside aria-label="Navegação principal"><a class:active={route === 'home'} href={href('home')}>Início</a><a class:active={route === 'catalog'} href={href('catalog')}>Catálogo</a></aside><main><p class="eyebrow">BIBLIOTECA PESSOAL</p>{#if error}<p class="error" role="alert">{error}</p>{/if}{#if route === 'home'}<h1>Olá, {user.username}.</h1><p class="intro">Sua biblioteca está protegida e pronta para estudar.</p><a class="primary" href={href('catalog')}>Abrir catálogo</a>{:else if route === 'catalog'}<h1>Seus cursos</h1>{#if catalog && catalog.categories.length}{#each catalog.categories as category}<section class="category"><h2>{category.title}</h2>{#each category.courses as course}<h3>{course.title}</h3>{#each course.modules as module}<div class="module"><strong>{module.title}</strong>{#each module.lessons as item}<button class="lesson" onclick={() => { lesson = item; lessonPath = item.path; location.hash = href('player', item.path); }}>{item.title}</button>{/each}</div>{/each}{/each}</section>{/each}{:else}<p class="empty">O catálogo ainda está vazio.</p>{/if}{:else}<h1>{lesson?.title ?? 'Aula não encontrada'}</h1>{#if !lesson}<p class="empty" role="status">Não foi possível encontrar esta aula no catálogo atual.</p>{:else if lesson.media[0]}<video controls preload="metadata" src={transport.mediaUrl(lesson.media[0].path)}><track kind="captions" /></video>{:else}<p class="empty">Esta aula não possui mídia.</p>{/if}<a class="back" href={href('catalog')}>Voltar ao catálogo</a>{/if}</main></div></div>
{/if}
