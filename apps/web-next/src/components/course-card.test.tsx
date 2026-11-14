import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, describe, expect, it } from 'vitest';
import { CourseCard, progressLabel } from '@/components/course-card';
import type { CourseEntry } from '@/lib/library';

afterEach(cleanup);

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

	it('grid and shelf show progress and the favorite toggle; shelf is a fixed-width card', () => {
		show('shelf');
		expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('25');
		expect(screen.getByRole('button', { name: 'Add Rust from Zero to favorites' }).getAttribute('aria-pressed')).toBe('false');
		expect(screen.getByRole('listitem').className).toContain('w-64');
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

	it('hides progress for a course without lessons', () => {
		show('grid', { total: 0, done: 0, status: 'not_started' });
		expect(screen.queryByRole('progressbar')).toBeNull();
	});

	it('marks a favorite as pressed and offers removal', () => {
		show('grid', { favorite: true });
		expect(screen.getByRole('button', { name: 'Remove Rust from Zero from favorites' }).getAttribute('aria-pressed')).toBe('true');
	});

	it('shows the management menu only when the account can manage', () => {
		show('grid');
		expect(screen.queryByRole('button', { name: 'Manage Rust from Zero' })).toBeNull();
		cleanup();
		show('grid', undefined, true);
		expect(screen.getByRole('button', { name: 'Manage Rust from Zero' })).toBeTruthy();
	});
});

describe('progressLabel', () => {
	it('covers in progress, not started and completed', () => {
		expect(progressLabel(entry())).toBe('25% complete · 3 lessons left');
		expect(progressLabel(entry({ status: 'not_started', done: 0 }))).toBe('4 lessons');
		expect(progressLabel(entry({ status: 'completed', done: 4 }))).toBe('Completed');
	});
});
