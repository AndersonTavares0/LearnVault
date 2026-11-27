import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CourseCard, progressLabel } from '@/components/course-card';
import { transport } from '@/lib/transport';
import type { CourseEntry } from '@/lib/library';

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

const entry = (patch: Partial<CourseEntry> = {}): CourseEntry => ({
	course: { title: 'Rust from Zero', path: 'rust', role: 'course', courseId: 'c1', files: [], looseFiles: [], children: [] },
	courseId: 'c1',
	status: 'in_progress',
	done: 1,
	total: 4,
	favorite: false,
	next: { path: 'rust/02', title: 'Ownership' },
	lastStudiedAt: 1_700_000_000,
	...patch
});

function show(variant: 'grid' | 'shelf' | 'list' | 'compact', patch?: Partial<CourseEntry>, canManage = false) {
	render(
		<QueryClientProvider client={new QueryClient()}>
			<MemoryRouter><ul><CourseCard entry={entry(patch)} profile="" variant={variant} canManage={canManage} /></ul></MemoryRouter>
		</QueryClientProvider>
	);
}

describe('CourseCard variants', () => {
	it.each(['grid', 'shelf', 'list', 'compact'] as const)('%s links the title to the course', (variant) => {
		show(variant);
		expect(screen.getByRole('link', { name: 'Rust from Zero' }).getAttribute('href')).toContain('c1');
	});

	it('shelf shows progress and the favorite toggle in a fixed-width card', () => {
		show('shelf');
		expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('25');
		expect(screen.getByRole('button', { name: 'Add Rust from Zero to favorites' }).getAttribute('aria-pressed')).toBe('false');
		expect(screen.getByRole('listitem').className).toContain('w-64');
	});

	it('grid is a borderless cover with lesson count, done count, segments and the favorite toggle', () => {
		show('grid');
		expect(screen.getAllByText('4 lessons')).toHaveLength(1);
		expect(screen.getAllByText('Rust from Zero')).toHaveLength(1);
		expect(screen.getByText('1 done')).toBeTruthy();
		expect(screen.getByRole('progressbar', { name: '1 of 4 lessons done' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Add Rust from Zero to favorites' })).toBeTruthy();
		expect(screen.queryByText('Resume')).toBeNull();
	});

	it('grid says Started or Not started when no lesson is finished', () => {
		show('grid', { done: 0, status: 'in_progress' });
		expect(screen.getByText('Started')).toBeTruthy();
		cleanup();
		show('grid', { done: 0, status: 'not_started' });
		expect(screen.getByText('Not started')).toBeTruthy();
	});

	it('list shows the next lesson and Resume, without the favorite toggle', () => {
		show('list');
		expect(screen.getByText('Next: Ownership')).toBeTruthy();
		expect(screen.getByText('Resume')).toBeTruthy();
		expect(screen.queryByRole('button', { name: /favorites/ })).toBeNull();
	});

	it('compact is a single row with Resume and no progressbar role', () => {
		show('compact');
		expect(screen.getByText('Resume')).toBeTruthy();
		expect(screen.queryByRole('progressbar')).toBeNull();
	});

	it('hides progress and labels zero-lesson courses distinctly', () => {
		show('grid', { total: 0, done: 0, status: 'not_started' });
		expect(screen.queryByRole('progressbar')).toBeNull();
		expect(screen.getByText('No lessons found')).toBeTruthy();
		cleanup();
		show('list', { total: 0, done: 0, status: 'not_started' });
		expect(screen.getAllByText('No lessons found').length).toBeGreaterThan(0);
		expect(screen.queryByText('Not started')).toBeNull();
	});

	it('marks a favorite as pressed and offers removal', () => {
		show('grid', { favorite: true });
		expect(screen.getByRole('button', { name: 'Remove Rust from Zero from favorites' }).getAttribute('aria-pressed')).toBe('true');
	});

	it('announces a failed favorite change and lets the learner retry', async () => {
		const save = vi.spyOn(transport, 'setCourseFavorite')
			.mockRejectedValueOnce(new Error('Offline'))
			.mockResolvedValueOnce(true);
		show('grid');
		const favorite = screen.getByRole('button', { name: 'Add Rust from Zero to favorites' });
		fireEvent.click(favorite);
		expect((await screen.findByRole('alert')).textContent).toContain('Try again');
		expect(favorite.getAttribute('aria-pressed')).toBe('false');
		fireEvent.click(favorite);
		await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
		expect(save).toHaveBeenCalledTimes(2);
	});

	it('never offers local-only management for a course on a remote source', () => {
		show('grid', { course: { ...entry().course, remote: true } }, true);
		expect(screen.queryByRole('button', { name: 'Manage Rust from Zero' })).toBeNull();
		cleanup();
		show('compact', { course: { ...entry().course, remote: true } }, true);
		expect(screen.queryByRole('button', { name: 'Manage Rust from Zero' })).toBeNull();
	});

	it('shows the management menu only when the account can manage', () => {
		show('grid');
		expect(screen.queryByRole('button', { name: 'Manage Rust from Zero' })).toBeNull();
		cleanup();
		show('grid', undefined, true);
		expect(screen.getByRole('button', { name: 'Manage Rust from Zero' })).toBeTruthy();
	});

	it('puts the course link before its separate favorite and management controls', () => {
		show('grid', undefined, true);
		const course = screen.getByRole('link', { name: 'Rust from Zero' });
		const favorite = screen.getByRole('button', { name: 'Add Rust from Zero to favorites' });
		const manage = screen.getByRole('button', { name: 'Manage Rust from Zero' });
		expect(course.compareDocumentPosition(favorite)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
		expect(favorite.compareDocumentPosition(manage)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
	});
});

describe('progressLabel', () => {
	it('covers in progress, not started and completed', () => {
		expect(progressLabel(entry())).toBe('25% complete · 3 lessons left');
		expect(progressLabel(entry({ status: 'not_started', done: 0 }))).toBe('4 lessons');
		expect(progressLabel(entry({ status: 'completed', done: 4 }))).toBe('Completed');
	});
});
