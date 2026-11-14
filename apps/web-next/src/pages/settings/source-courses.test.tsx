import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from '@/lib/i18n';
import { transport, type Catalog, type RemoteSource, type SourceFolder, type SourceListing, type SourceRootEntry } from '@/lib/transport';
import { admin, renderApp } from '@/test/render-app';

const catalog: Catalog = { complete: true, looseFiles: [], nodes: [] };
const source: RemoteSource = { id: 'src-1', name: 'Cursos', url: 'smb://nas.example.test/cursos', username: 'ana', hasPassword: true, credentialError: false };
const folder = (name: string, path: string, state: SourceFolder['state'] = 'free'): SourceFolder => ({ name, path, state });
const listing = (path: string, folders: SourceFolder[], extra: Partial<SourceListing> = {}): SourceListing => ({ path, folders, mediaFiles: 0, otherFiles: 0, truncated: false, ...extra });

let roots: SourceRootEntry[];
const tree: Record<string, SourceListing> = {
	'': listing('', [folder('Pentest', 'Pentest'), folder('Infra', 'Infra')]),
	Pentest: listing('Pentest', [folder('Redes', 'Pentest/Redes'), folder('Web', 'Pentest/Web')], { mediaFiles: 2 })
};

beforeEach(() => {
	roots = [];
	vi.spyOn(transport, 'listSources').mockResolvedValue([source]);
	vi.spyOn(transport, 'browseSource').mockImplementation(async (_id, path) => tree[path] ?? listing(path, []));
	vi.spyOn(transport, 'listSourceRoots').mockImplementation(async () => roots);
});
afterEach(() => {
	setLocale('en');
	cleanup();
	vi.restoreAllMocks();
});

async function openDialog() {
	renderApp('/settings#sources', { catalog, user: admin });
	const row = (await screen.findByText(source.name, { selector: 'p' })).closest('li')!;
	fireEvent.click(within(row).getByRole('button', { name: 'Choose courses' }));
	return screen.findByRole('dialog');
}

describe('choosing the courses of a remote source', () => {
	it('walks the folders one level at a time and jumps back from the path', async () => {
		const dialog = await openDialog();
		expect(await within(dialog).findByRole('button', { name: 'Open Pentest' })).toBeTruthy();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Open Pentest' }));

		expect(await within(dialog).findByRole('button', { name: 'Open Redes' })).toBeTruthy();
		expect(transport.browseSource).toHaveBeenLastCalledWith('src-1', 'Pentest');
		expect(within(dialog).getByText('2 video or audio files directly in this folder.')).toBeTruthy();
		const path = within(dialog).getByRole('navigation', { name: 'Folder path' });
		expect(within(path).getByRole('button', { name: 'Pentest' }).getAttribute('aria-current')).toBe('page');

		fireEvent.click(within(path).getByRole('button', { name: source.name }));
		expect(await within(dialog).findByRole('button', { name: 'Open Infra' })).toBeTruthy();
		expect(within(path).queryByRole('button', { name: 'Pentest' })).toBeNull();
	});

	it('moves the keyboard focus to the new level of the path when a folder opens', async () => {
		const dialog = await openDialog();
		fireEvent.click(await within(dialog).findByRole('button', { name: 'Open Pentest' }));
		const path = within(dialog).getByRole('navigation', { name: 'Folder path' });
		await waitFor(() => expect(document.activeElement).toBe(within(path).getByRole('button', { name: 'Pentest' })));

		fireEvent.click(within(path).getByRole('button', { name: source.name }));
		await waitFor(() => expect(document.activeElement).toBe(within(path).getByRole('button', { name: source.name })));
	});

	it('keeps the previous folder on screen, inactive, while the next one loads', async () => {
		let finish: (value: SourceListing) => void = () => {};
		vi.mocked(transport.browseSource!).mockImplementation((_id, path) => path === 'Pentest' ? new Promise<SourceListing>((resolve) => { finish = resolve; }) : Promise.resolve(tree[path] ?? listing(path, [])));
		const dialog = await openDialog();
		fireEvent.click(await within(dialog).findByRole('button', { name: 'Open Pentest' }));

		// Still the root listing, but nothing in it can be chosen and it is marked busy.
		const choose = await within(dialog).findByRole('button', { name: 'Choose Infra as a course' });
		expect((choose as HTMLButtonElement).disabled).toBe(true);
		// The bulk action still belongs to the root listing that is shown, and it is inactive too.
		const bulkOfRoot = within(dialog).getByRole('button', { name: `Make each of the 2 folders in “${source.name}” a course` });
		expect((bulkOfRoot as HTMLButtonElement).disabled).toBe(true);
		expect(dialog.querySelector('[aria-busy="true"]')).toBeTruthy();

		finish(tree.Pentest);
		const bulk = await within(dialog).findByRole('button', { name: 'Make each of the 2 folders in “Pentest” a course' });
		expect((bulk as HTMLButtonElement).disabled).toBe(false);
		expect(dialog.querySelector('[aria-busy="true"]')).toBeNull();
	});

	it('chooses a folder as a course and reloads the list', async () => {
		const choose = vi.spyOn(transport, 'chooseSourceRoots').mockImplementation(async () => {
			roots = [{ id: 'root-1', name: 'Redes', path: 'Pentest/Redes' }];
			return { created: roots, skipped: [] };
		});
		const dialog = await openDialog();
		fireEvent.click(await within(dialog).findByRole('button', { name: 'Open Pentest' }));
		fireEvent.click(await within(dialog).findByRole('button', { name: 'Choose Redes as a course' }));

		await waitFor(() => expect(choose).toHaveBeenCalledWith('src-1', 'Pentest/Redes', 'course'));
		expect(await within(dialog).findByText(/Chosen: 1 course\./)).toBeTruthy();
		const chosen = within(dialog).getByRole('region', { name: 'Chosen courses' });
		expect(await within(chosen).findByText('Pentest/Redes')).toBeTruthy();
	});

	it('makes every folder of the current level a course and reports what was left out', async () => {
		const choose = vi.spyOn(transport, 'chooseSourceRoots').mockResolvedValue({ created: [{ id: 'a', name: 'Web', path: 'Pentest/Web' }], skipped: [{ path: 'Pentest/Redes', reason: 'overlaps_existing_root' }] });
		const dialog = await openDialog();
		fireEvent.click(await within(dialog).findByRole('button', { name: 'Open Pentest' }));
		fireEvent.click(await within(dialog).findByRole('button', { name: 'Make each of the 2 folders in “Pentest” a course' }));

		await waitFor(() => expect(choose).toHaveBeenCalledWith('src-1', 'Pentest', 'children'));
		expect(await within(dialog).findByText(/1 folder was left out\./)).toBeTruthy();
	});

	it('does not offer a folder that is already, or holds, a chosen course', async () => {
		tree[''] = listing('', [folder('A', 'A', 'root'), folder('B', 'B', 'inside_root'), folder('C', 'C', 'contains_root'), folder('D', 'D')]);
		const dialog = await openDialog();
		await within(dialog).findByRole('button', { name: 'Open A' });
		expect(within(dialog).queryByRole('button', { name: 'Choose A as a course' })).toBeNull();
		expect(within(dialog).queryByRole('button', { name: 'Choose B as a course' })).toBeNull();
		expect(within(dialog).queryByRole('button', { name: 'Choose C as a course' })).toBeNull();
		expect(within(dialog).getByRole('button', { name: 'Choose D as a course' })).toBeTruthy();
		expect(within(dialog).getByText('Course')).toBeTruthy();
		expect(within(dialog).getByText('Part of a course')).toBeTruthy();
		expect(within(dialog).getByText('Has courses inside')).toBeTruthy();
		// With a single free folder the row's own button is enough, so the bulk action is not offered.
		expect(within(dialog).queryByRole('button', { name: /Make each of the/ })).toBeNull();
		tree[''] = listing('', [folder('Pentest', 'Pentest'), folder('Infra', 'Infra')]);
	});

	it('undoes a choice', async () => {
		roots = [{ id: 'root-1', name: 'Redes', path: 'Pentest/Redes' }];
		const remove = vi.spyOn(transport, 'removeSourceRoot').mockImplementation(async () => { roots = []; return true; });
		const dialog = await openDialog();
		fireEvent.click(await within(dialog).findByRole('button', { name: 'Undo the choice of Redes' }));

		await waitFor(() => expect(remove).toHaveBeenCalledWith('src-1', 'root-1'));
		expect(await within(dialog).findByText(/went back to the source course/)).toBeTruthy();
		expect(await within(dialog).findByText(/None yet/)).toBeTruthy();
	});

	it('shows a localized message when a folder cannot be read', async () => {
		vi.spyOn(transport, 'browseSource').mockRejectedValue(new Error('That folder was not found on the source.'));
		const dialog = await openDialog();
		expect((await within(dialog).findByRole('alert')).textContent).toBe('That folder was not found on the source.');
	});

	it('reads the source and says when some folders could not be read', async () => {
		const scan = vi.spyOn(transport, 'scanSource').mockResolvedValue({ complete: false, inserted: 3 });
		renderApp('/settings#sources', { catalog, user: admin });
		const row = (await screen.findByText(source.name, { selector: 'p' })).closest('li')!;
		fireEvent.click(within(row).getByRole('button', { name: 'Read source' }));

		await waitFor(() => expect(scan).toHaveBeenCalledWith('src-1'));
		expect(await screen.findByText('Source read: 3 new lessons. Some folders could not be read.')).toBeTruthy();
	});

	it('offers neither action when the stored password cannot be read', async () => {
		vi.spyOn(transport, 'listSources').mockResolvedValue([{ ...source, credentialError: true }]);
		renderApp('/settings#sources', { catalog, user: admin });
		const row = (await screen.findByText(source.name, { selector: 'p' })).closest('li')!;
		expect(within(row).queryByRole('button', { name: 'Choose courses' })).toBeNull();
		expect(within(row).queryByRole('button', { name: 'Read source' })).toBeNull();
	});

	it('speaks Brazilian Portuguese', async () => {
		setLocale('pt-BR');
		vi.spyOn(transport, 'chooseSourceRoots').mockResolvedValue({ created: [{ id: 'a', name: 'Web', path: 'Pentest/Web' }], skipped: [] });
		renderApp('/settings#sources', { catalog, user: admin });
		const row = (await screen.findByText(source.name, { selector: 'p' })).closest('li')!;
		expect(within(row).getByRole('button', { name: 'Ler a fonte' })).toBeTruthy();
		fireEvent.click(within(row).getByRole('button', { name: 'Escolher cursos' }));
		const dialog = await screen.findByRole('dialog');
		fireEvent.click(await within(dialog).findByRole('button', { name: 'Abrir Pentest' }));
		expect(await within(dialog).findByRole('button', { name: 'Tornar cada uma das 2 pastas de “Pentest” um curso' })).toBeTruthy();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Escolher Redes como um curso' }));
		expect(await within(dialog).findByText(/Escolhido: 1 curso\./)).toBeTruthy();
	});
});
