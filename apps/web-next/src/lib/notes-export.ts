import { formatDuration } from '@/lib/player';

export type ExportNote = { course: string; lesson: string; positionSeconds: number; updatedAt: number; content: string };

/** Local calendar day of a note (`updatedAt` is in seconds), so "today" follows the reader's clock. */
export function dayKey(updatedAt: number): string {
	const date = new Date(updatedAt * 1000);
	return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

/** Notes written since the start of the current week (Monday, local time). */
export function notesThisWeek(notes: { updatedAt: number }[], now = new Date()): number {
	const start = new Date(now.getFullYear(), now.getMonth(), now.getDate() - ((now.getDay() + 6) % 7)).getTime() / 1000;
	return notes.filter((note) => note.updatedAt >= start).length;
}

function groupBy<T>(items: T[], key: (item: T) => string): Map<string, T[]> {
	const groups = new Map<string, T[]>();
	for (const item of items) groups.set(key(item), [...(groups.get(key(item)) ?? []), item]);
	return groups;
}

/**
 * Markdown for the notes you are looking at: a heading per course, one per lesson, and a line per
 * note with its moment. Plain text only, so it opens anywhere and nothing is rewritten.
 */
export function notesToMarkdown(notes: ExportNote[]): string {
	const lines: string[] = ['# LearnVault notes', ''];
	const byCourse = groupBy(notes, (note) => note.course);
	for (const [course, courseNotes] of byCourse) {
		lines.push(`## ${course}`, '');
		for (const [lesson, lessonNotes] of groupBy(courseNotes, (note) => note.lesson)) {
			lines.push(`### ${lesson}`, '');
			for (const note of [...lessonNotes].sort((a, b) => a.positionSeconds - b.positionSeconds)) {
				const [first, ...rest] = note.content.trim().split('\n');
				lines.push(`- **${formatDuration(note.positionSeconds)}** ${first}`, ...rest.map((line: string) => `  ${line}`));
			}
			lines.push('');
		}
	}
	return `${lines.join('\n').trimEnd()}\n`;
}
