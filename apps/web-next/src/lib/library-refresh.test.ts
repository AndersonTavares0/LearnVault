import { afterEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from '@/lib/i18n';
import { isNoCourses, refreshLibrary, refreshMessage } from '@/lib/library-refresh';
import type { RemoteSource } from '@/lib/transport';

afterEach(() => setLocale('en'));

const source = (id: string, name: string, credentialError = false): RemoteSource => ({ id, name, url: 'smb://nas.example.test/x', username: null, hasPassword: false, credentialError });
const noCourses = () => Object.assign(new Error('No courses imported yet.'), { name: 'NoCoursesError' });

function fake(sources: RemoteSource[] | undefined, over: { rescan?: () => Promise<void>; scanSource?: (id: string) => Promise<unknown> } = {}) {
	const calls: string[] = [];
	return {
		calls,
		transport: {
			rescan: vi.fn(async () => { calls.push('rescan'); await over.rescan?.(); }),
			listSources: sources ? vi.fn(async () => sources) : undefined,
			scanSource: vi.fn(async (id: string) => { calls.push(`scan:${id}`); await over.scanSource?.(id); return { complete: true, inserted: 0 }; })
		}
	};
}

describe('refreshLibrary', () => {
	it('rescans the local courses and then reads every remote source, in order', async () => {
		const { transport, calls } = fake([source('a', 'Cursos'), source('b', 'Outros')]);
		const progress: string[] = [];
		const outcome = await refreshLibrary(transport, (message) => progress.push(message));
		expect(calls).toEqual(['rescan', 'scan:a', 'scan:b']);
		expect(progress).toEqual(['Reading “Cursos”…', 'Reading “Outros”…']);
		expect(outcome).toEqual({ sources: 2, failed: [] });
	});

	it('is not a failure when only remote sources exist', async () => {
		const { transport, calls } = fake([source('a', 'Cursos')], { rescan: async () => { throw noCourses(); } });
		expect(await refreshLibrary(transport)).toEqual({ sources: 1, failed: [] });
		expect(calls).toEqual(['rescan', 'scan:a']);
	});

	it('still reports an empty library when there is nothing to read at all', async () => {
		const { transport } = fake([], { rescan: async () => { throw noCourses(); } });
		await expect(refreshLibrary(transport)).rejects.toMatchObject({ name: 'NoCoursesError' });
	});

	it('stops before the remote sources when the local rescan fails for another reason', async () => {
		const { transport, calls } = fake([source('a', 'Cursos')], { rescan: async () => { throw new Error('A rescan is already in progress.'); } });
		await expect(refreshLibrary(transport)).rejects.toThrow('already in progress');
		expect(calls).toEqual(['rescan']);
	});

	it('keeps going when one source cannot be read and says which', async () => {
		const { transport, calls } = fake([source('a', 'Cursos'), source('b', 'Outros')], { scanSource: async (id) => { if (id === 'a') throw new Error('The server could not be reached.'); } });
		const outcome = await refreshLibrary(transport);
		expect(calls).toEqual(['rescan', 'scan:a', 'scan:b']);
		expect(outcome).toEqual({ sources: 2, failed: [{ name: 'Cursos', message: 'The server could not be reached.' }] });
	});

	it('skips a source whose stored password cannot be read', async () => {
		const { transport, calls } = fake([source('a', 'Cursos', true), source('b', 'Outros')]);
		await refreshLibrary(transport);
		expect(calls).toEqual(['rescan', 'scan:b']);
	});

	it('works without a source list (desktop, or an account that cannot list sources)', async () => {
		const { transport, calls } = fake(undefined);
		expect(await refreshLibrary(transport)).toEqual({ sources: 0, failed: [] });
		expect(calls).toEqual(['rescan']);
		const denied = { ...fake([source('a', 'x')]).transport, listSources: vi.fn(async () => { throw new Error('credentials_change_required'); }) };
		expect((await refreshLibrary(denied)).sources).toBe(0);
	});

	it('recognizes the named error and nothing else', () => {
		expect(isNoCourses(noCourses())).toBe(true);
		expect(isNoCourses(new Error('No courses imported yet.'))).toBe(false);
		expect(isNoCourses('NoCoursesError')).toBe(false);
	});
});

describe('refreshMessage', () => {
	it('says what was refreshed, in both languages', () => {
		expect(refreshMessage({ sources: 0, failed: [] })).toBe('Catalog updated.');
		expect(refreshMessage({ sources: 2, failed: [] })).toBe('Catalog updated, including 2 remote sources.');
		expect(refreshMessage({ sources: 1, failed: [] })).toBe('Catalog updated, including 1 remote source.');
		expect(refreshMessage({ sources: 2, failed: [{ name: 'Cursos', message: 'The server took too long to answer.' }] })).toBe('Catalog updated, but this source could not be read: “Cursos”. The server took too long to answer.');
		setLocale('pt-BR');
		expect(refreshMessage({ sources: 1, failed: [] })).toBe('Catálogo atualizado, incluindo 1 fonte remota.');
		expect(refreshMessage({ sources: 2, failed: [{ name: 'A', message: 'x' }, { name: 'B', message: 'y' }] })).toBe('Catálogo atualizado, mas estas fontes não puderam ser lidas: “A”, “B”. x');
	});
});
