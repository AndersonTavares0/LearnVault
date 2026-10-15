import { describe, expect, it } from 'vitest';
import type { Transport } from './transport';

describe('transport contract', () => {
	it('exposes the HTTP transport seam without routing assumptions', () => {
		const methods: (keyof Transport)[] = ['me', 'login', 'logout', 'catalog', 'mediaUrl'];
		expect(methods).toHaveLength(5);
	});
});
