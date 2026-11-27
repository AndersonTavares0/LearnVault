import type { FileEntry } from './transport';

/**
 * Tells whether on-demand conversion of SRT subtitles is available.
 *
 * Both transports convert SRT to WebVTT in the browser: HTTP fetches the subtitle file and the
 * desktop reads it via `desktop_read_media_slice` IPC without opening the network.
 */
export const srtConversionSupported: boolean = true;

/**
 * Tells whether the transport implements study progress.
 *
 * Both transports implement it: HTTP through the authenticated routes and the desktop through the
 * comandos IPC (`desktop_study_progress`, `desktop_record_position`,
 * `desktop_set_completed`, `desktop_recent_study`), with no `fetch`. The constant speaks only of the
 * transport's capability — whether there is progress to sync is decided by
 * [`studySyncEnabled`], which also requires a media source delivered by the transport.
 */
export const studySyncSupported: boolean = true;

/**
 * Decides whether the player may read and write study progress for the current media.
 *
 * It requires three things: the transport implements Study, study is enabled in the session and the
 * transport delivered a source for the media (`source`). The source can be HTTP or the desktop's
 * opaque protocol; in both cases the position represents real playback.
 */
export function studySyncEnabled(studyEnabled: boolean, mediaId: string | null, source: string | null): boolean {
	return studySyncSupported && studyEnabled && mediaId !== null && source !== null;
}

export const PLAYBACK_SPEEDS = [0.75, 1, 1.25, 1.5, 1.75, 2] as const;
export type PlaybackSpeed = (typeof PLAYBACK_SPEEDS)[number];

export function clampSpeed(speed: number): PlaybackSpeed {
	return (PLAYBACK_SPEEDS as readonly number[]).includes(speed) ? (speed as PlaybackSpeed) : 1;
}

export function nextSpeed(current: number, direction: 'up' | 'down'): PlaybackSpeed {
	const currentSpeed = clampSpeed(current);
	const index = PLAYBACK_SPEEDS.indexOf(currentSpeed);
	if (direction === 'up') {
		const nextIndex = Math.min(PLAYBACK_SPEEDS.length - 1, index + 1);
		return PLAYBACK_SPEEDS[nextIndex];
	}
	const prevIndex = Math.max(0, index - 1);
	return PLAYBACK_SPEEDS[prevIndex];
}

export type PlayerShortcutAction =
	| 'toggle-playback'
	| 'seek-backward'
	| 'seek-forward'
	| 'toggle-mute'
	| 'volume-up'
	| 'volume-down'
	| 'open-notes'
	| { type: 'seek-percent'; percent: number };

/** Shortcuts only apply when the player's container, not an inner control, has focus. */
export function playerShortcutAction(key: string, playerRootHasFocus: boolean): PlayerShortcutAction | null {
	if (!playerRootHasFocus) return null;
	if (key === ' ' || key === 'Spacebar' || key === 'k' || key === 'K') return 'toggle-playback';
	if (key === 'ArrowLeft' || key === 'j' || key === 'J') return 'seek-backward';
	if (key === 'ArrowRight' || key === 'l' || key === 'L') return 'seek-forward';
	if (key === 'm' || key === 'M') return 'toggle-mute';
	if (key === 'ArrowUp') return 'volume-up';
	if (key === 'ArrowDown') return 'volume-down';
	if (key === 'n' || key === 'N') return 'open-notes';
	if (key >= '0' && key <= '9') {
		return { type: 'seek-percent', percent: parseInt(key, 10) * 10 };
	}
	return null;
}

/** Adjusts the volume in steps of 10%, keeping it between 0 and 1. */
export function stepVolume(currentVolume: number, direction: 'up' | 'down'): number {
	const step = direction === 'up' ? 0.1 : -0.1;
	const next = Math.round((currentVolume + step) * 10) / 10;
	return Math.max(0, Math.min(1, next));
}

/** Computes the playback position from a percentage (0-100%). */
export function seekPercentPosition(duration: number, percent: number): number {
	if (!Number.isFinite(duration) || duration <= 0) return 0;
	const clamped = Math.max(0, Math.min(100, percent));
	return (clamped / 100) * duration;
}

/**
 * Where playback should start.
 *
 * Clamps the saved position to the known duration and avoids resuming less than two seconds from
 * the end: by then the lesson has already finished, so it restarts from zero instead of stopping at the final credits.
 */
export function resumeTarget(position: number, duration: number): number {
	if (!Number.isFinite(position) || position <= 0) return 0;
	if (!Number.isFinite(duration) || duration <= 0) return position;
	const target = Math.min(position, Math.max(0, duration - 1));
	return duration - target < 2 ? 0 : target;
}

/** The periodic clock saves at most every `intervalMs`; discrete events are forced. */
export function shouldSavePosition(lastSavedAt: number, now: number, intervalMs = 5000): boolean {
	return now - lastSavedAt >= intervalMs;
}

/** There is no automatic advance if the preference is off or the sequence has ended. */
export function shouldAutoAdvance(enabled: boolean, hasNextLesson: boolean): boolean {
	return enabled && hasNextLesson;
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

export function selectPlayableMedia(media: FileEntry[], preferredMediaId?: string): FileEntry | null {
	const available = (file: FileEntry) => file.availability?.trim().toLowerCase() !== 'missing';
	if (preferredMediaId) {
		const preferred = media.find((file) => file.mediaId === preferredMediaId && available(file));
		if (preferred) return preferred;
	}
	return media.find(available) ?? null;
}

/**
 * State of a lesson's media, derived only from the catalog.
 *
 * `playable`: there is available media. `missing`: the lesson has catalogued media and none of it
 * is available — the file left the disk (external drive disconnected, file
 * renamed) and the scan marked `availability = 'missing'`, preserving the lesson and the
 * progress. `absent`: the lesson has no media file at all in the catalog.
 *
 * The distinction exists so the player's message is factual: a lesson whose media
 * disappeared from the disk is not the same thing as a lesson with no media at all.
 */
export type MediaAvailability = 'playable' | 'missing' | 'absent';

export function mediaAvailability(media: FileEntry[]): MediaAvailability {
	if (selectPlayableMedia(media)) return 'playable';
	return media.length > 0 ? 'missing' : 'absent';
}

export function selectPlayableSubtitle(subtitles: FileEntry[], preferredSubtitleId?: string | null, preferredLanguage?: string): FileEntry | null {
	if (preferredSubtitleId === null) return null;
	const available = (file: FileEntry) => file.availability?.trim().toLowerCase() !== 'missing';
	if (preferredSubtitleId) {
		const preferred = subtitles.find((file) => file.mediaId === preferredSubtitleId && available(file));
		if (preferred) return preferred;
	}
	if (preferredLanguage) {
		const localized = subtitles.find((file) => available(file) && subtitleLanguage(file.name)?.toLowerCase() === preferredLanguage.toLowerCase());
		if (localized) return localized;
	}
	return subtitles.find(available) ?? null;
}

/** Language tag of a sidecar subtitle (`aula.pt-BR.vtt` → `pt-BR`); null when it has none. */
export function subtitleLanguage(name: string): string | null {
	const stem = (name.split(/[?#]/, 1)[0] ?? '').replace(/\.[^.]+$/, '');
	const tag = stem.slice(stem.lastIndexOf('.') + 1);
	if (tag === stem) return null;
	const [primary, ...rest] = tag.split(/[-_]/);
	if (!/^[A-Za-z]{2,3}$/.test(primary ?? '') || rest.some((part) => !/^[A-Za-z0-9]{2,4}$/.test(part))) return null;
	return tag;
}

export type MediaPlaybackKind = 'video' | 'audio';

/**
 * Identifies formats that commonly need compatibility or on-demand
 * transcoding when the browser has no native decoders.
 */
export function requiresTranscodingProbe(fileName: string): boolean {
	const ext = fileName.split(/[?#]/, 1)[0].split('.').pop()?.toLowerCase() ?? '';
	return ['mkv', 'avi', 'wmv', 'flv', 'ts'].includes(ext);
}

export function mediaPlaybackKind(file: FileEntry): MediaPlaybackKind {
	const extension = file.name.split(/[?#]/, 1)[0].split('.').pop()?.toLowerCase();
	return extension && ['aac', 'flac', 'm4a', 'mp3', 'oga', 'ogg', 'opus', 'wav', 'weba', 'wma'].includes(extension) ? 'audio' : 'video';
}

export function subtitleFormat(path: string): 'vtt' | 'srt' | 'ass' | 'sub' | 'unsupported' {
	const extension = path.split(/[?#]/, 1)[0].split('.').pop()?.toLowerCase();
	if (extension === 'vtt') return 'vtt';
	if (extension === 'srt') return 'srt';
	if (extension === 'ass' || extension === 'ssa') return 'ass';
	if (extension === 'sub') return 'sub';
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
