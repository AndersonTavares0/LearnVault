import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

/**
 * Guarda de estrutura do rescan desktop.
 *
 * O rescan é explícito e nunca automático: nem a inicialização do aplicativo nem a
 * seleção da raiz podem dispará-lo. Como o repositório não traz um renderizador de
 * componentes, o contrato é verificado no próprio shell Svelte, do mesmo modo que os
 * guardas do crate desktop verificam `command.rs`/`lib.rs` por texto.
 */
const source = readFileSync(new URL('./App.svelte', import.meta.url), 'utf8');

describe('desktop rescan is explicit and configured-only', () => {
	it('never triggers a rescan during initialization', () => {
		const start = source.indexOf('async function initialize');
		const end = source.indexOf('async function login');
		expect(start).toBeGreaterThan(-1);
		expect(end).toBeGreaterThan(start);
		expect(source.slice(start, end)).not.toContain('rescan');
	});

	it('exposes a rescan control that only appears with a configured library', () => {
		const button = source.indexOf('onclick={rescan}');
		expect(button).toBeGreaterThan(-1);
		// O primeiro `onclick={rescan}` pertence à tela offline; o `{#if` mais próximo
		// precisa ser o de biblioteca configurada.
		const guard = source.lastIndexOf('{#if', button);
		expect(guard).toBeGreaterThan(-1);
		expect(source.slice(guard, button)).toContain("config === 'configured'");
	});

	it('routes the desktop rescan through the transport instead of a direct command', () => {
		// O shell fala apenas com o transporte; o `invoke` é detalhe do DesktopStub.
		expect(source).toContain('await transport.rescan()');
		expect(source).not.toContain("invoke('desktop_rescan'");
	});

	it('invalidates the previous desktop library before rescanning the selected root', () => {
		const start = source.indexOf('async function chooseLibrary');
		const end = source.indexOf('\n\tasync function loadRecent', start);
		const chooseLibrary = source.slice(start, end);
		expect(chooseLibrary).toContain('catalog = null;');
		expect(chooseLibrary).toContain('lesson = null;');
		expect(chooseLibrary).toContain('recent = [];');
		expect(chooseLibrary.indexOf('recent = [];')).toBeLessThan(chooseLibrary.indexOf('await transport.rescan()'));
		expect(chooseLibrary.indexOf('await transport.rescan()')).toBeLessThan(chooseLibrary.indexOf('const nextCatalog = await transport.catalog()'));
	});
});
