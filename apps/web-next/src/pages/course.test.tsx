import { cleanup, fireEvent, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { Catalog, CatalogNode, FileEntry } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const file = (name: string, kind: string, availability = 'available'): FileEntry => ({ name, path: `rust/${name}`, size: 1, mediaId: kind === 'media' ? name : undefined, kind, availability });
const lesson = (title: string, files: FileEntry[]): CatalogNode => ({ title, path: `rust/${title}`, role: 'lesson', files, looseFiles: [], children: [] });
const module = (title: string, children: CatalogNode[]): CatalogNode => ({ title, path: `rust/${title}`, role: 'module', files: [], looseFiles: [], children });
const catalog: Catalog = {
	complete: true,
	looseFiles: [],
	nodes: [{
		title: 'Rust from zero', path: 'rust', role: 'course', courseId: 'rust', files: [], looseFiles: [],
		children: [
			module('Basics', [lesson('Ownership', [file('own.mp4', 'media'), file('slides.pdf', 'material')]), lesson('Borrowing', [file('borrow.mp4', 'media')])]),
			module('Advanced', [lesson('Lifetimes', [file('life.mp4', 'media', 'missing')])])
		]
	}]
};

afterEach(cleanup);

describe('course page', () => {
	it('shows progress, resumes the last lesson and lists modules with per-lesson state', async () => {
		renderApp('/course/rust', { catalog, recent: [{ mediaId: 'borrow.mp4', positionSeconds: 70, completed: false, updatedAt: 2, recorded: true }, { mediaId: 'own.mp4', positionSeconds: 0, completed: true, updatedAt: 1, recorded: true }] });
		expect(await screen.findByRole('heading', { level: 1, name: 'Rust from zero' })).toBeTruthy();
		expect(screen.getByRole('progressbar', { name: '1 of 2 lessons completed (50%)' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Continue studying' }).getAttribute('href')).toBe('/course/rust/rust%2FBorrowing?t=70');
		expect(screen.getByText('1 of 2 done')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Mark “Ownership” as not completed' }).getAttribute('aria-pressed')).toBe('true');
		expect(screen.getByRole('link', { name: /Borrowing/ }).getAttribute('aria-current')).toBe('page');
		expect(screen.getByText('Missing media')).toBeTruthy();
	});

	it('lists course materials with a download link', async () => {
		renderApp('/course/rust', { catalog });
		fireEvent.click(await screen.findByRole('tab', { name: /Materials/ }));
		const panel = screen.getByRole('tabpanel');
		expect(within(panel).getByText('slides.pdf')).toBeTruthy();
		expect(within(panel).getByRole('link', { name: 'Download slides.pdf' })).toBeTruthy();
	});

	it('offers course management to admins only', async () => {
		renderApp('/course/rust', { catalog, user: { userId: 'u2', username: 'ana', role: 'user', isAdmin: false, mustChangeCredentials: false } });
		await screen.findByRole('heading', { level: 1, name: 'Rust from zero' });
		expect(screen.queryByRole('button', { name: 'Manage course' })).toBeNull();
		cleanup();
		renderApp('/course/rust', { catalog });
		expect(await screen.findByRole('button', { name: 'Manage course' })).toBeTruthy();
	});

	it('explains a course that is gone', async () => {
		renderApp('/course/unknown', { catalog });
		expect(await screen.findByRole('heading', { name: 'Course not found' })).toBeTruthy();
	});
});
