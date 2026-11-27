import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { transport, type StudyProgress } from '@/lib/transport';
import { useRecent } from './data';

const lesson = (n: number): StudyProgress => ({ mediaId: `m${n}`, positionSeconds: 1, completed: n % 2 === 0, updatedAt: 1000 - n, recorded: true });

describe('useRecent', () => {
	afterEach(() => vi.restoreAllMocks());

	it('walks every page so a course of more than 100 lessons keeps its progress', async () => {
		const studyState = vi.spyOn(transport, 'studyState')
			.mockResolvedValueOnce({ items: Array.from({ length: 100 }, (_, n) => lesson(n)), nextCursor: '900.m99' })
			.mockResolvedValueOnce({ items: [lesson(100)], nextCursor: null });
		const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
		const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;

		const { result } = renderHook(() => useRecent(), { wrapper });

		await waitFor(() => expect(result.current.data).toHaveLength(101));
		expect(studyState).toHaveBeenNthCalledWith(1, null, 500);
		expect(studyState).toHaveBeenNthCalledWith(2, '900.m99', 500);
		expect(result.current.data?.[100].mediaId).toBe('m100');
	});
});
