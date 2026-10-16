import type { FileEntry } from './transport';

/**
 * Indica se a conversão de legendas SRT sob demanda está disponível.
 *
 * O desktop é offline: o caminho que baixa o SRT com `fetch` só existe no build HTTP.
 * A constante é resolvida em tempo de compilação por `__LEARNVAULT_TRANSPORT__`, para
 * que o código do `fetch` saia do bundle desktop por dead-code elimination. A decisão
 * fica fora do componente Svelte porque ali `===` vira uma chamada de runtime que o
 * minificador não consegue dobrar.
 */
export const srtConversionSupported: boolean = __LEARNVAULT_TRANSPORT__ === 'http';
export const studySyncSupported: boolean = __LEARNVAULT_TRANSPORT__ === 'http';

export function formatDuration(totalSeconds: number): string {
	if (!Number.isFinite(totalSeconds) || totalSeconds < 0) return '0:00';
	const seconds = Math.floor(totalSeconds % 60);
	const minutes = Math.floor((totalSeconds / 60) % 60);
	const hours = Math.floor(totalSeconds / 3600);
	const paddedSeconds = String(seconds).padStart(2, '0');
	if (hours > 0) return `${hours}:${String(minutes).padStart(2, '0')}:${paddedSeconds}`;
	return `${minutes}:${paddedSeconds}`;
}

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
