import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { transport, type Catalog, type RemoteSource } from '@/lib/transport';
import { admin, renderApp } from '@/test/render-app';

const student = { userId: 'u2', username: 'ana', role: 'user', isAdmin: false, mustChangeCredentials: false };
const catalog: Catalog = { complete: true, looseFiles: [], nodes: [] };
const source: RemoteSource = { id: 'src-1', name: 'Cursos', url: 'https://dav.example.test/cursos/', username: 'ana', hasPassword: true, credentialError: false };

beforeEach(() => {
	vi.spyOn(transport, 'listSources').mockResolvedValue([source]);
});
afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe('remote source settings', () => {
	it('shows the section only to administrators', async () => {
		renderApp('/settings#sources', { catalog, user: student });
		expect(await screen.findByRole('heading', { name: 'Settings' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Remote sources' })).toBeNull();
		expect(screen.queryByRole('heading', { name: 'Remote sources' })).toBeNull();
		cleanup();
		renderApp('/settings#sources', { catalog, user: admin });
		expect(await screen.findByRole('heading', { name: 'Remote sources' })).toBeTruthy();
		expect(await screen.findByText(source.url)).toBeTruthy();
	});

	it('adds a source without ever sending the stored password back', async () => {
		const create = vi.spyOn(transport, 'createSource').mockResolvedValue({ ...source, id: 'src-2', name: 'Novo' });
		renderApp('/settings#sources', { catalog, user: admin });
		const form = (await screen.findByRole('heading', { name: 'Add a source' })).closest('form')!;
		const password = within(form).getByLabelText('Password (optional)', { selector: 'input' }) as HTMLInputElement;
		expect(password.value).toBe('');

		fireEvent.change(within(form).getByLabelText('Name'), { target: { value: 'Novo' } });
		fireEvent.change(within(form).getByLabelText('Address'), { target: { value: 'davs://dav.example.test/novo/' } });
		fireEvent.change(within(form).getByLabelText('Username (optional)'), { target: { value: 'bia' } });
		fireEvent.change(password, { target: { value: 'synthetic-secret' } });
		fireEvent.click(within(form).getByRole('button', { name: 'Add source' }));

		await waitFor(() => expect(create).toHaveBeenCalledWith({ name: 'Novo', url: 'davs://dav.example.test/novo/', username: 'bia', password: 'synthetic-secret' }));
		await waitFor(() => expect((within(form).getByLabelText('Name') as HTMLInputElement).value).toBe(''));
		expect(screen.queryByDisplayValue('synthetic-secret')).toBeNull();
		expect(screen.queryByDisplayValue('ana')).toBeNull();
	});

	it('rejects a scheme the server would refuse before sending it', async () => {
		const create = vi.spyOn(transport, 'createSource');
		renderApp('/settings#sources', { catalog, user: admin });
		const form = (await screen.findByRole('heading', { name: 'Add a source' })).closest('form')!;
		fireEvent.change(within(form).getByLabelText('Name'), { target: { value: 'FTP' } });
		fireEvent.change(within(form).getByLabelText('Address'), { target: { value: 'ftp://server/share' } });
		expect(within(form).getByText(/Use a dav:\/\//)).toBeTruthy();
		expect((within(form).getByRole('button', { name: 'Add source' }) as HTMLButtonElement).disabled).toBe(true);
		expect(create).not.toHaveBeenCalled();
	});

	it('accepts an SMB address and warns about plain transports', async () => {
		renderApp('/settings#sources', { catalog, user: admin });
		const form = (await screen.findByRole('heading', { name: 'Add a source' })).closest('form')!;
		fireEvent.change(within(form).getByLabelText('Address'), { target: { value: 'smb://server/share/cursos' } });
		expect(screen.queryByText(/Use a dav:\/\//)).toBeNull();
		expect((within(form).getByRole('button', { name: 'Add source' }) as HTMLButtonElement).disabled).toBe(false);
		fireEvent.change(within(form).getByLabelText('Address'), { target: { value: 'dav://server/dav/' } });
		expect(within(form).getByText(/without transport encryption/)).toBeTruthy();
	});

	it('requires a share name in an SMB address', async () => {
		renderApp('/settings#sources', { catalog, user: admin });
		const form = (await screen.findByRole('heading', { name: 'Add a source' })).closest('form')!;
		const address = within(form).getByLabelText('Address');
		fireEvent.change(address, { target: { value: 'smb://server/' } });
		expect(within(form).getByText(/Add the share name/)).toBeTruthy();
		expect((within(form).getByRole('button', { name: 'Add source' }) as HTMLButtonElement).disabled).toBe(true);
		fireEvent.change(address, { target: { value: 'smb://server/share' } });
		expect(within(form).getByText(/Format: smb:\/\/server\/share\/folder/)).toBeTruthy();
	});

	it('shows a localized message instead of a raw error when the test request fails', async () => {
		vi.spyOn(transport, 'testSource').mockRejectedValue(new Error('ECONNRESET 10.0.0.5'));
		renderApp('/settings#sources', { catalog, user: admin });
		const row = (await screen.findByText(source.name)).closest('li')!;
		fireEvent.click(within(row).getByRole('button', { name: 'Test connection' }));
		expect(await screen.findByText('The connection test failed.')).toBeTruthy();
		expect(screen.queryByText(/ECONNRESET/)).toBeNull();
	});

	it('tests the connection and reports success and failure', async () => {
		vi.spyOn(transport, 'testSource')
			.mockResolvedValueOnce({ ok: true, entries: 3, error: null })
			.mockResolvedValueOnce({ ok: false, entries: null, error: 'denied' });
		renderApp('/settings#sources', { catalog, user: admin });
		const row = (await screen.findByText(source.name)).closest('li')!;

		fireEvent.click(within(row).getByRole('button', { name: 'Test connection' }));
		expect(await screen.findByText('Reachable · 3 entries')).toBeTruthy();

		fireEvent.click(within(row).getByRole('button', { name: 'Test connection' }));
		expect(await screen.findByText('The server refused these credentials.')).toBeTruthy();
	});

	it('asks for confirmation and removes the source', async () => {
		const remove = vi.spyOn(transport, 'deleteSource').mockResolvedValue(true);
		renderApp('/settings#sources', { catalog, user: admin });
		const row = (await screen.findByText(source.name)).closest('li')!;

		fireEvent.click(within(row).getByRole('button', { name: 'Remove' }));
		const dialog = await screen.findByRole('alertdialog');
		expect(within(dialog).getByText(/Remove the source/)).toBeTruthy();
		expect(remove).not.toHaveBeenCalled();

		fireEvent.click(within(dialog).getByRole('button', { name: 'Remove source' }));
		await waitFor(() => expect(remove).toHaveBeenCalledWith('src-1'));
	});
});
