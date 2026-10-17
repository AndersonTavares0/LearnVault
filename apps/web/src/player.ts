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

/**
 * Indica se o transporte implementa o progresso de estudo.
 *
 * Os dois transportes implementam: o HTTP pelas rotas autenticadas e o desktop pelos
 * comandos IPC (`desktop_study_progress`, `desktop_record_position`,
 * `desktop_set_completed`, `desktop_recent_study`), sem `fetch`. A constante fala apenas da
 * capacidade do transporte — quem decide se há progresso a sincronizar é
 * [`studySyncEnabled`], que também exige uma fonte de mídia entregue pelo transporte.
 */
export const studySyncSupported: boolean = true;

/**
 * Decide se o player pode ler e gravar progresso de estudo na mídia atual.
 *
 * Exige três coisas: o transporte implementa Study, o estudo está habilitado na sessão e o
 * transporte entregou uma fonte para a mídia (`source`). A fonte pode ser HTTP ou o protocolo
 * opaco do desktop; em ambos os casos a posição representa reprodução real.
 */
export function studySyncEnabled(studyEnabled: boolean, mediaId: string | null, source: string | null): boolean {
	return studySyncSupported && studyEnabled && mediaId !== null && source !== null;
}

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

export function selectPlayableSubtitle(subtitles: FileEntry[]): FileEntry | null {
	return subtitles.find((file) => file.availability?.trim().toLowerCase() !== 'missing') ?? null;
}

export type MediaPlaybackKind = 'video' | 'audio';

export function mediaPlaybackKind(file: FileEntry): MediaPlaybackKind {
	const extension = file.name.split(/[?#]/, 1)[0].split('.').pop()?.toLowerCase();
	return extension && ['aac', 'flac', 'm4a', 'mp3', 'oga', 'ogg', 'opus', 'wav', 'weba', 'wma'].includes(extension) ? 'audio' : 'video';
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
