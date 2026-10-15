import type { FileEntry } from './transport';

export function selectPlayableMedia(media: FileEntry[]): FileEntry | null {
	return media.find((file) => file.availability?.trim().toLowerCase() !== 'missing') ?? null;
}

export function subtitleFormat(path: string): 'vtt' | 'srt' | 'ass' | 'unsupported' {
	const extension = path.split(/[?#]/, 1)[0].split('.').pop()?.toLowerCase();
	if (extension === 'vtt') return 'vtt';
	if (extension === 'srt') return 'srt';
	if (extension === 'ass' || extension === 'ssa') return 'ass';
	return 'unsupported';
}

function timestamp(value: string): string {
	const normalized = value.trim().replace(',', '.');
	return normalized.split(':').length === 2 ? `00:${normalized}` : normalized;
}

export function srtToWebVtt(srt: string): string {
	const blocks = srt.replace(/^\uFEFF/, '').replace(/\r\n?/g, '\n').trim().split(/\n{2,}/);
	const cues = blocks.flatMap((block) => {
		const lines = block.split('\n');
		const timing = lines.findIndex((line) => line.includes('-->'));
		if (timing < 0) return [];
		const times = lines[timing].split('-->');
		if (times.length !== 2) return [];
		const text = lines.slice(timing + 1).join('\n').trim();
		return text ? [`${timestamp(times[0])} --> ${timestamp(times[1])}\n${text}`] : [];
	});
	return `WEBVTT\n\n${cues.join('\n\n')}\n`;
}
