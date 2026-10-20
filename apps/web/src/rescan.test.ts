import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

/**
 * Guarda de estrutura do shell para o gerenciamento de cursos.
 *
 * Importar e indexar são ações explícitas e administrativas: a inicialização nunca indexa
 * sozinha, o rescan passa pelo transporte e remover um curso exige uma confirmação
 * explícita. Como o repositório não traz um renderizador de aplicação inteira, o contrato
 * é verificado no próprio shell Svelte, do mesmo modo que os guardas do crate desktop
 * verificam `command.rs`/`lib.rs` por texto.
 */
const source = readFileSync(new URL('./App.svelte', import.meta.url), 'utf8');

function body(startMarker: string, endMarker: string): string {
	const start = source.indexOf(startMarker);
	const end = source.indexOf(endMarker, start);
	expect(start).toBeGreaterThan(-1);
	expect(end).toBeGreaterThan(start);
	return source.slice(start, end);
}

describe('course management is explicit and administrative', () => {
	it('never triggers a rescan during initialization', () => {
		const initialize = body('async function initialize', 'async function login');
		expect(initialize).not.toContain('rescan');
	});

	it('routes the rescan through the transport instead of a direct command', () => {
		expect(source).toContain('await transport.rescan()');
		expect(source).not.toContain("invoke('desktop_rescan'");
	});

	it('only offers import and rescan behind the administrator guard', () => {
		const rescanAt = source.indexOf('onclick={rescan}');
		const desktopImportAt = source.indexOf('onclick={importCourse}');
		const serverImportAt = source.indexOf('onclick={toggleImportForm}');
		expect(rescanAt).toBeGreaterThan(-1);
		// A ação da biblioteca fica no bloco `{#if canManage}`: a guarda está logo antes
		// e o botão não tem uma chamada solta fora dele.
		expect(source.lastIndexOf('{#if canManage}', rescanAt)).toBeGreaterThan(rescanAt - 2000);
		expect(source.lastIndexOf('{#if canManage}', desktopImportAt)).toBeGreaterThan(desktopImportAt - 400);
		// O botão HTTP fica dentro do mesmo bloco canManage, mas depois do ramo desktop
		// (`{#if transportKind === 'desktop'}`), que pode ocupar mais que 400 bytes.
		expect(source.lastIndexOf('{#if canManage}', serverImportAt)).toBeGreaterThan(serverImportAt - 2000);
	});

	it('imports a course and rescans before reloading the catalog', () => {
		const desktopImport = body('async function importCourse', 'async function submitImport');
		expect(desktopImport.indexOf('transport.importCourse()')).toBeGreaterThan(-1);
		expect(desktopImport.indexOf('transport.rescan()')).toBeGreaterThan(desktopImport.indexOf('transport.importCourse()'));
		expect(desktopImport.indexOf('transport.catalog()')).toBeGreaterThan(desktopImport.indexOf('transport.rescan()'));
		expect(desktopImport).not.toContain('invoke(');
	});

	it('registers a server course by the typed path through the transport', () => {
		const serverImport = body('async function submitImport', 'async function removeCourse');
		expect(serverImport).toContain('transport.importCoursePath(');
		expect(serverImport.indexOf('transport.rescan()')).toBeGreaterThan(serverImport.indexOf('transport.importCoursePath('));
		expect(serverImport).not.toContain('invoke(');
	});

	it('removes a course only after an explicit confirmation and reloads the catalog', () => {
		expect(source).toContain('onclick={removeCourse}');
		const button = source.indexOf('onclick={removeCourse}');
		const guard = source.lastIndexOf('{#if removeConfirm}', button);
		expect(guard).toBeGreaterThan(-1);
		expect(guard).toBeLessThan(button);

		const remove = body('async function removeCourse', 'async function loadRecent');
		expect(remove.indexOf('transport.removeCourse(')).toBeGreaterThan(-1);
		expect(remove.indexOf('transport.catalog()')).toBeGreaterThan(remove.indexOf('transport.removeCourse('));
	});
});
