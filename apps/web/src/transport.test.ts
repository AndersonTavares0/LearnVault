import { describe, expect, it, vi } from 'vitest';
import { HttpTransport, type Transport } from './transport';

describe('transport contract', () => {
	it('exposes the HTTP transport seam without routing assumptions', () => {
		const methods: (keyof Transport)[] = ['me', 'login', 'logout', 'catalog', 'rescan', 'mediaUrl'];
		expect(methods).toHaveLength(6);
	});
	it('builds media and caption URLs using the existing endpoint contract', () => {
		const transport = new HttpTransport();
		expect(transport.mediaUrl('módulo/aula.mp4')).toBe('/api/media?path=m%C3%B3dulo%2Faula.mp4');
		expect(transport.mediaUrl('legendas/aula.vtt')).toBe('/api/media?path=legendas%2Faula.vtt');
	});
	it('posts the admin rescan endpoint and maps a conflict', async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetchMock);
		await new HttpTransport().rescan();
		expect(fetchMock).toHaveBeenCalledWith('/api/admin/catalog/rescan', { method: 'POST' });
		fetchMock.mockResolvedValue(new Response(null, { status: 409 }));
		await expect(new HttpTransport().rescan()).rejects.toThrow('Já existe um rescan em andamento.');
		vi.unstubAllGlobals();
	});
});
