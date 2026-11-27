import { transport, type FileEntry, type Lesson } from './transport';

/** Most bytes of a text file the lesson page reads; a longer one is cut and says so. */
export const TEXT_LIMIT = 256 * 1024;

export const isTextFile = (file: Pick<FileEntry, 'name'>) => /\.(txt|md|markdown)$/i.test(file.name);

/**
 * A lesson with nothing to play and no PDF, but a readable text file, is a text lesson: the
 * text is shown on the page instead of an empty player.
 */
export function findLessonText(lesson: Lesson, hasPdf: boolean): FileEntry | null {
	if (hasPdf || lesson.media.length > 0) return null;
	return lesson.materials.find((file) => isTextFile(file) && file.availability?.trim().toLowerCase() !== 'missing' && file.mediaId) ?? null;
}

/** UTF-8, or Windows-1252 when the bytes are not valid UTF-8 (older course notes are often Latin-1). */
export function decodeText(bytes: Uint8Array): string {
	const text = (() => {
		try {
			return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
		} catch {
			return new TextDecoder('windows-1252').decode(bytes);
		}
	})();
	return text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
}

/** Loads a bounded prefix of a text/markdown lesson over either HTTP Range or Desktop IPC. */
export async function loadText(file: FileEntry, courseId: string): Promise<{ text: string; truncated: boolean }> {
	const { bytes, total } = await transport.readMediaSlice(file, courseId, TEXT_LIMIT);
	return { text: decodeText(bytes.subarray(0, TEXT_LIMIT)), truncated: Number.isFinite(total) && total > TEXT_LIMIT };
}
