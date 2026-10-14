<svelte:head>
	<title>LearnVault — sua biblioteca de estudos</title>
	<meta name="description" content="Sua biblioteca pessoal de cursos, em um só lugar." />
</svelte:head>

<script lang="ts">
	import { onMount } from 'svelte';

	let username = '';
	let password = '';
	let currentUser: { username: string; role: string } | null = null;
	let errorMessage = '';
	let busy = false;
	type CatalogFile = { name: string; path: string; size: number };
	type Lesson = { title: string; path: string; media: CatalogFile[]; subtitles: CatalogFile[]; materials: CatalogFile[] };
	type Catalog = { categories: { title: string; courses: { title: string; modules: { title: string; lessons: Lesson[] }[] }[] }[] };
	let catalog: Catalog | null = null;
	let catalogBusy = false;
	let catalogMessage = '';
	let selectedLesson: (Lesson & { path: string }) | null = null;

	onMount(async () => {
		try {
			const response = await fetch('/api/me');
			if (response.ok) {
				currentUser = await response.json();
				await loadCatalog();
			}
		} catch {
			errorMessage = 'Não foi possível conectar ao serviço.';
		}
	});

	async function login(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		errorMessage = '';
		try {
			const response = await fetch('/api/login', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ username, password })
			});
			if (!response.ok) throw new Error(response.status === 401 ? 'Usuário ou senha incorretos.' : 'Não foi possível entrar.');
			currentUser = await response.json();
			await loadCatalog();
			password = '';
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'Não foi possível conectar ao serviço.';
		} finally {
			busy = false;
		}
	}

	async function loadCatalog() {
		try {
			const response = await fetch('/api/catalog');
			if (response.ok) catalog = await response.json();
		} catch {
			catalogMessage = 'Não foi possível carregar o catálogo.';
		}
	}

	async function rescan() {
		catalogBusy = true;
		catalogMessage = 'Lendo a biblioteca…';
		try {
			const response = await fetch('/api/admin/catalog/rescan', { method: 'POST' });
			if (!response.ok) throw new Error(response.status === 409 ? 'Já existe um rescan em andamento.' : 'Não foi possível indexar a biblioteca.');
			await loadCatalog();
			catalogMessage = 'Catálogo atualizado.';
		} catch (error) {
			catalogMessage = error instanceof Error ? error.message : 'Falha ao indexar.';
		} finally {
			catalogBusy = false;
		}
	}

	async function logout() {
		await fetch('/api/logout', { method: 'POST' });
		currentUser = null;
	}
</script>

<main>
	<div class="topline"><span class="mark">LV</span><span>LEARNVAULT</span><span class="local">BIBLIOTECA PESSOAL</span></div>
	<section>
		<p class="eyebrow">SEU ESPAÇO DE APRENDIZAGEM</p>
		<h1>Conhecimento,<br /><em>no seu ritmo.</em></h1>
		<p class="intro">Uma casa tranquila para seus cursos, ideias e próximos passos.</p>
		{#if currentUser}
			<div class="notice"><span class="dot"></span><div><strong>Olá, {currentUser.username}.</strong><br />Sua biblioteca está protegida e pronta para ser indexada.<button class="text-button" onclick={logout}>Sair da conta</button></div></div>
			<div class="catalog-panel">
				<div class="catalog-header"><div><p class="eyebrow">CATÁLOGO FÍSICO</p><h2>Seus cursos</h2></div><button class="scan-button" onclick={rescan} disabled={catalogBusy}>{catalogBusy ? 'Indexando…' : 'Indexar biblioteca'}</button></div>
				{#if catalogMessage}<p class="catalog-message">{catalogMessage}</p>{/if}
				{#if catalog && catalog.categories.length > 0}
					{#each catalog.categories as category}<div class="category"><h3>{category.title}</h3>{#each category.courses as course}<details><summary>{course.title}<span>{course.modules.length} módulos</span></summary>{#each course.modules as module}<div class="module"><strong>{module.title}</strong>{#each module.lessons as lesson}<button class="lesson" onclick={() => selectedLesson = lesson}><span>{lesson.title}</span><small>{lesson.media.length} vídeo · {lesson.materials.length + lesson.subtitles.length} materiais</small></button>{/each}</div>{/each}</details>{/each}</div>{/each}
				{:else}<p class="empty">O catálogo ainda está vazio. A indexação lê a biblioteca sem modificar seus arquivos.</p>{/if}
				{#if selectedLesson && selectedLesson.media.length > 0}
					<div class="player"><div class="player-header"><div><p class="eyebrow">AULA SELECIONADA</p><h3>{selectedLesson.title}</h3></div><button class="close-player" onclick={() => selectedLesson = null}>Fechar</button></div><video controls preload="metadata" src={`/api/media?path=${encodeURIComponent(selectedLesson.media[0].path)}`}><track kind="captions" src={selectedLesson.subtitles.length > 0 ? `/api/media?path=${encodeURIComponent(selectedLesson.subtitles[0].path)}` : 'data:text/vtt,WEBVTT'} label="Legenda" default /></video><div class="materials"><strong>Materiais</strong><span>{selectedLesson.subtitles.length} legenda(s) · {selectedLesson.materials.length} arquivo(s)</span></div></div>
				{/if}
			</div>
		{:else}
			<form class="login" onsubmit={login}>
				<label>Usuário<input autocomplete="username" bind:value={username} required /></label>
				<label>Senha<input type="password" autocomplete="current-password" bind:value={password} required /></label>
				{#if errorMessage}<p class="error" role="alert">{errorMessage}</p>{/if}
				<button disabled={busy}>{busy ? 'Entrando…' : 'Entrar'}</button>
				<p class="hint">A conta inicial é criada pelo administrador na primeira inicialização.</p>
			</form>
		{/if}
	</section>
	<footer><span>FEITO PARA APRENDER</span><span>SELF-HOSTED · PRIVADO</span></footer>
</main>

<style>
	:global(*){box-sizing:border-box} :global(body){margin:0;background:#f4f1e9;color:#252923;font-family:Inter,ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif}
	main{min-height:100svh;max-width:1440px;margin:auto;padding:32px clamp(24px,7vw,104px);display:flex;flex-direction:column}
	.topline,footer{display:flex;align-items:center;gap:12px;font-size:11px;letter-spacing:.14em;font-weight:700;color:#596256}
	.mark{display:grid;place-items:center;width:34px;height:34px;border-radius:50%;background:#2f493d;color:#f6f2e9;font:700 11px Georgia,serif;letter-spacing:0}
	.local{margin-left:auto;font-weight:500;letter-spacing:.1em}
	section{margin:auto 0;max-width:640px;padding:90px 0}.eyebrow{font-size:11px;letter-spacing:.19em;font-weight:700;color:#77806e}
	h1{font:400 clamp(56px,9vw,108px)/.98 Georgia,"Times New Roman",serif;letter-spacing:-.055em;margin:26px 0;color:#25372e}h1 em{font-weight:400;color:#8a7351}
	.intro{font-size:17px;line-height:1.7;color:#687066;max-width:430px;margin-top:28px}
	.notice{display:flex;gap:14px;align-items:flex-start;margin-top:52px;padding:18px 20px;background:#e9e7db;border:1px solid #dedccf;border-radius:12px;color:#62685e;font-size:13px;line-height:1.6;max-width:430px}.notice strong{color:#303e33;font-weight:650}.dot{width:8px;height:8px;flex:none;border-radius:50%;background:#718b63;margin-top:6px}
 	.catalog-panel{margin-top:32px;max-width:720px;padding:22px;background:#faf9f5;border:1px solid #dedccf;border-radius:14px}.catalog-header{display:flex;justify-content:space-between;gap:20px;align-items:end}.catalog-header .eyebrow{margin:0 0 8px}.catalog-header h2{margin:0;color:#304438;font:400 30px Georgia,serif}.scan-button{border:1px solid #87957f;border-radius:7px;background:#eef0e8;color:#344b3b;padding:9px 13px;font:inherit;font-size:12px;cursor:pointer}.scan-button:disabled{opacity:.6;cursor:wait}.catalog-message,.empty{font-size:12px;color:#777d71}.category{margin-top:22px}.category h3{font-size:13px;color:#68745f;margin:0 0 8px}.category details{border-top:1px solid #e6e4da;padding:11px 0}.category summary{cursor:pointer;color:#304438;font-weight:650;font-size:14px}.category summary span{float:right;font-weight:400;color:#92988c;font-size:11px}.module{margin:12px 0 0 16px;padding-left:12px;border-left:2px solid #dce1d5}.module>strong{font-size:12px;color:#657064}.lesson{display:flex;justify-content:space-between;gap:12px;margin-top:8px;font-size:12px;color:#4b554c;width:100%;border:0;background:none;text-align:left;padding:0;cursor:pointer;font:inherit}.lesson:hover span{text-decoration:underline}.lesson small{color:#92988c;white-space:nowrap}.empty{padding:10px 0;margin:0}.player{margin-top:24px;background:#202b25;border-radius:12px;padding:16px;color:#f5f2e9}.player-header{display:flex;justify-content:space-between;align-items:start;gap:12px}.player h3{margin:0;font:400 23px Georgia,serif}.player .eyebrow{color:#b9c6ae;margin:0 0 6px}.player video{display:block;width:100%;margin-top:16px;border-radius:7px;background:#111}.close-player{border:1px solid #71806d;border-radius:6px;background:none;color:#f5f2e9;padding:6px 9px;cursor:pointer}.materials{display:flex;justify-content:space-between;gap:12px;margin-top:12px;font-size:11px;color:#c3cabe}.materials strong{color:#f5f2e9}
	.login{display:grid;gap:15px;margin-top:42px;max-width:380px}.login label{display:grid;gap:7px;color:#576157;font-size:12px;font-weight:600}.login input{border:1px solid #d5d4c9;border-radius:8px;padding:12px 13px;background:#faf9f5;color:#252923;font:inherit}.login input:focus{outline:2px solid #82927d;outline-offset:1px}.login button{justify-self:start;border:0;border-radius:8px;background:#2f493d;color:#fff;padding:11px 24px;font:inherit;font-size:13px;font-weight:650;cursor:pointer}.login button:disabled{opacity:.65;cursor:wait}.hint,.error{margin:0;font-size:12px;line-height:1.5;color:#85877b}.error{color:#a23f32}.text-button{display:block;margin-top:14px;border:0;padding:0;background:none;color:#526747;text-decoration:underline;cursor:pointer;font:inherit;font-size:12px}
	footer{justify-content:space-between;margin-top:auto;border-top:1px solid #dddacf;padding-top:18px;font-size:9px;letter-spacing:.14em;color:#888b7e}
	@media(max-width:600px){main{padding:22px 24px}.local{font-size:8px}section{padding:64px 0}h1{font-size:64px}.intro{font-size:15px}footer{font-size:8px}}
</style>
