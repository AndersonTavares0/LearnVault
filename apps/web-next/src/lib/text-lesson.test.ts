import { describe, expect, it } from 'vitest';
import { decodeText, findLessonText, isTextFile } from './text-lesson';
import type { FileEntry, Lesson } from './transport';

const file = (name: string, extra: Partial<FileEntry> = {}): FileEntry => ({ name, path: name, size: 10, mediaId: `id-${name}`, ...extra });
const lesson = (over: Partial<Lesson>): Lesson => ({ title: 'L', path: 'L', media: [], subtitles: [], materials: [], ...over });

describe('findLessonText', () => {
	it('picks the first readable text file of a lesson with nothing to play', () => {
		expect(findLessonText(lesson({ materials: [file('capa.png'), file('notas.TXT'), file('b.md')] }), false)?.name).toBe('notas.TXT');
	});
	it('leaves a lesson with media, a PDF lesson, a missing file or one without an id alone', () => {
		expect(findLessonText(lesson({ media: [file('a.mp4')], materials: [file('notas.txt')] }), false)).toBeNull();
		expect(findLessonText(lesson({ materials: [file('notas.txt')] }), true)).toBeNull();
		expect(findLessonText(lesson({ materials: [file('notas.txt', { availability: 'missing' })] }), false)).toBeNull();
		expect(findLessonText(lesson({ materials: [{ name: 'notas.txt', path: 'notas.txt', size: 1 }] }), false)).toBeNull();
	});
	it('knows text files by extension', () => {
		expect(['a.txt', 'a.MD', 'a.markdown'].every((name) => isTextFile({ name }))).toBe(true);
		expect(isTextFile({ name: 'a.txt.exe' })).toBe(false);
	});
});

describe('decodeText', () => {
	it('reads UTF-8 and drops a byte order mark', () => {
		expect(decodeText(new Uint8Array([0xef, 0xbb, 0xbf, ...new TextEncoder().encode('Aula çã')]))).toBe('Aula çã');
	});
	it('falls back to Windows-1252 for Latin-1 notes', () => {
		expect(decodeText(new Uint8Array([0x41, 0x75, 0x6c, 0x61, 0x20, 0xe7, 0xe3]))).toBe('Aula çã');
	});
});
