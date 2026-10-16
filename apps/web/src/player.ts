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
 * transporte entregou uma fonte para a mídia (`source`). A última condição é o gating
 * honesto do desktop: sem entrega de mídia não existe reprodução nem posição real, então a
 * barra de estudo permanece fechada em vez de prometer retomada e conclusão que não podem
 * acontecer. No HTTP a fonte acompanha a mídia, então o comportamento é o mesmo de antes.
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

/**
 * Indica que a reprodução de vídeo/áudio está indisponível no desktop nesta fatia.
 *
 * O protocolo de mídia do desktop entrega apenas arquivos pequenos; vídeo e áudio ficam
 * fora por causa da reprodução parcial incompatível no webview do Linux (WebKitGTK). A
 * constante é resolvida em tempo de compilação para que o HTTP mantenha a mensagem
 * original e o desktop apresente um estado explícito e sanitizado.
 */
export const desktopPlaybackUnavailable: boolean = __LEARNVAULT_TRANSPORT__ === 'desktop';

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
