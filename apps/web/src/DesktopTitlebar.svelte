<script lang="ts">
	import { onMount } from 'svelte';
	import Brand from './Brand.svelte';

	let maximized = $state(false);
	let error = $state('');

	onMount(() => {
		if (__LEARNVAULT_TRANSPORT__ !== 'desktop') return;

		let disposed = false;
		let unlistenResize: (() => void) | undefined;

		void (async () => {
			try {
				const { getCurrentWindow } = await import('@tauri-apps/api/window');
				const appWindow = getCurrentWindow();
				const syncMaximized = async () => {
					maximized = await appWindow.isMaximized();
				};
				await syncMaximized();
				const unlisten = await appWindow.onResized(syncMaximized);
				if (disposed) unlisten();
				else unlistenResize = unlisten;
			} catch (cause) {
				if (!disposed) error = `Não foi possível conectar aos controles da janela: ${message(cause)}`;
			}
		})();

		return () => {
			disposed = true;
			unlistenResize?.();
		};
	});

	async function perform(action: 'minimize' | 'toggleMaximize' | 'close') {
		if (__LEARNVAULT_TRANSPORT__ !== 'desktop') return;
		try {
			error = '';
			const { getCurrentWindow } = await import('@tauri-apps/api/window');
			const appWindow = getCurrentWindow();
			if (action === 'minimize') await appWindow.minimize();
			else if (action === 'close') await appWindow.close();
			else {
				await appWindow.toggleMaximize();
				maximized = await appWindow.isMaximized();
			}
		} catch (cause) {
			error = `A ação da janela falhou: ${message(cause)}`;
		}
	}

	function message(cause: unknown) {
		return cause instanceof Error ? cause.message : String(cause);
	}
</script>

<div class="titlebar" role="group" aria-label="Controles da janela" data-tauri-drag-region>
	<div class="brand" data-tauri-drag-region aria-hidden="true"><Brand /><span data-tauri-drag-region>LearnVault</span></div>
	<div class="window-controls">
		<button type="button" aria-label="Minimizar janela" title="Minimizar" onclick={() => perform('minimize')}>
			<svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2 8.5h8" /></svg>
		</button>
		<button type="button" aria-label={maximized ? 'Restaurar janela' : 'Maximizar janela'} title={maximized ? 'Restaurar' : 'Maximizar'} onclick={() => perform('toggleMaximize')}>
			{#if maximized}<svg viewBox="0 0 12 12" aria-hidden="true"><path d="M4 2.5h5.5V8M8 4H2.5v5.5H8z" /></svg>
			{:else}<svg viewBox="0 0 12 12" aria-hidden="true"><rect x="2.5" y="2.5" width="7" height="7" /></svg>{/if}
		</button>
		<button class="close" type="button" aria-label="Fechar janela" title="Fechar" onclick={() => perform('close')}>
			<svg viewBox="0 0 12 12" aria-hidden="true"><path d="m3 3 6 6m0-6L3 9" /></svg>
		</button>
	</div>
</div>
{#if error}<p class="titlebar-error" role="alert">{error}</p>{/if}

<style>
	.titlebar { position: sticky; top: 0; z-index: 40; height: 32px; display: flex; align-items: center; justify-content: space-between; padding-left: 10px; background: var(--surface); color: var(--ink); border-bottom: 1px solid var(--line); user-select: none; }
	.brand { --brand-mark-ink: var(--surface); display: flex; align-items: center; gap: 7px; font-size: 12px; font-weight: 650; letter-spacing: .01em; }
	.brand :global(.brand-mark) { width: 18px; height: 18px; color: var(--ink); pointer-events: none; }
	.window-controls { height: 100%; display: flex; }
	button { width: 44px; height: 100%; display: grid; place-items: center; border: 0; border-radius: 0; padding: 0; background: transparent; color: var(--ink); cursor: pointer; }
	button:hover { background: color-mix(in srgb, var(--ink) 9%, transparent); }
	button:focus-visible { outline: 2px solid var(--accent); outline-offset: -3px; }
	button.close:hover { background: #c42b1c; color: white; }
	svg { width: 12px; height: 12px; fill: none; stroke: currentColor; stroke-width: 1.1; }
	.titlebar-error { position: fixed; z-index: 1000; top: 32px; right: 8px; max-width: min(440px, calc(100vw - 16px)); margin: 0; padding: 8px 12px; border: 1px solid var(--line); border-radius: 6px; background: var(--surface); color: var(--ink); font-size: 12px; box-shadow: 0 4px 16px #0003; }
</style>
