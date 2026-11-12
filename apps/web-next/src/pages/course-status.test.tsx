import { cleanup, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { Catalog, FileEntry, StudyProgress } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const media = (id: string): FileEntry => ({ name: `${id}.mp4`, path: `c/${id}.mp4`, size: 1, mediaId: id, kind: 'media', availability: 'available' });
const lesson = (title: string, id: string) => ({ title, path: `c/${title}`, role: 'lesson' as const, files: [media(id)], looseFiles: [], children: [] });
const progress = (mediaId: string): StudyProgress => ({ mediaId, positionSeconds: 0, completed: true, updatedAt: 1, recorded: true });

afterEach(cleanup);

describe('course page status', () => {
	it('explains a low-confidence structure in plain words, one line per concern', () => {
		const catalog: Catalog = { complete: true, looseFiles: [], nodes: [{
			title: 'Course', path: 'c', role: 'course', courseId: 'c', files: [], looseFiles: [], children: [lesson('Intro', 'one')],
			structure: { confidence: 'low', concerns: ['no_lessons', 'loose_materials'] },
		}] };
		renderApp('/course/c', { catalog });
		expect(screen.getByRole('note')).toBeTruthy();
		expect(screen.getByText('The structure of this course was hard to read.')).toBeTruthy();
		expect(screen.getByText('No lesson has a video, so the lesson list may be empty.')).toBeTruthy();
		expect(screen.getByText('Some materials are not attached to a lesson.')).toBeTruthy();
	});

	it('shows no note when the structure is clear', () => {
		const catalog: Catalog = { complete: true, looseFiles: [], nodes: [{
			title: 'Course', path: 'c', role: 'course', courseId: 'c', files: [], looseFiles: [], children: [lesson('Intro', 'one')],
			structure: { confidence: 'high', concerns: [] },
		}] };
		renderApp('/course/c', { catalog });
		expect(screen.queryByRole('note')).toBeNull();
	});

	it('announces completion once every lesson is done', () => {
		const catalog: Catalog = { complete: true, looseFiles: [], nodes: [{
			title: 'Course', path: 'c', role: 'course', courseId: 'c', files: [], looseFiles: [], children: [lesson('Intro', 'one'), lesson('Next', 'two')],
		}] };
		renderApp('/course/c', { catalog, recent: [progress('one'), progress('two')] });
		expect(screen.getByText('Course complete. Well done.')).toBeTruthy();
	});

	it('does not announce completion while a lesson is still open', () => {
		const catalog: Catalog = { complete: true, looseFiles: [], nodes: [{
			title: 'Course', path: 'c', role: 'course', courseId: 'c', files: [], looseFiles: [], children: [lesson('Intro', 'one'), lesson('Next', 'two')],
		}] };
		renderApp('/course/c', { catalog, recent: [progress('one')] });
		expect(screen.queryByText('Course complete. Well done.')).toBeNull();
	});
});
