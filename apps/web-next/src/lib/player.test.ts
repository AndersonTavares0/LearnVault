import { describe, expect, it } from 'vitest';
import { clampSpeed, formatDuration, mediaAvailability, mediaPlaybackKind, nextSpeed, PLAYBACK_SPEEDS, playerShortcutAction, resumeTarget, seekPercentPosition, selectPlayableMedia, selectPlayableSubtitle, shouldAutoAdvance, shouldSavePosition, srtConversionSupported, srtToWebVtt, stepVolume, studySyncEnabled, studySyncSupported, subtitleFormat } from './player';
import type { FileEntry } from './transport';

describe('player media and subtitles', () => {
	it('selects the first media that is not missing', () => {
		expect(selectPlayableMedia([
			{ name: 'a.mp4', path: 'a.mp4', size: 1, kind: 'media', availability: 'missing' },
			{ name: 'b.mp4', path: 'b.mp4', size: 2, kind: 'media', availability: 'available' }
		])?.path).toBe('b.mp4');
		expect(selectPlayableMedia([{ name: 'a.mp4', path: 'a.mp4', size: 1, availability: 'MISSING' }])).toBeNull();
	});
	it('selects available captions and distinguishes video from audio', () => {
		expect(selectPlayableSubtitle([
			{ name: 'aula.srt', path: 'aula.srt', size: 1, kind: 'subtitle', availability: 'missing' },
			{ name: 'aula.vtt', path: 'aula.vtt', size: 2, kind: 'subtitle', availability: 'available' }
		])?.path).toBe('aula.vtt');
		expect(mediaPlaybackKind({ name: 'aula.mp4', path: 'aula.mp4', size: 1, kind: 'media' })).toBe('video');
		expect(mediaPlaybackKind({ name: 'aula.m4a', path: 'aula.m4a', size: 1, kind: 'media' })).toBe('audio');
	});
	it('restores a remembered playable media and falls back when it disappeared', () => {
		const media = [
			{ name: 'a.mp4', path: 'a.mp4', size: 1, mediaId: 'media-a', availability: 'available' },
			{ name: 'b.mp4', path: 'b.mp4', size: 2, mediaId: 'media-b', availability: 'available' }
		];

		expect(selectPlayableMedia(media, 'media-b')?.mediaId).toBe('media-b');
		expect(selectPlayableMedia(media, 'removed')?.mediaId).toBe('media-a');
	});
	it('restores a remembered subtitle or keeps captions explicitly off', () => {
		const subtitles = [
			{ name: 'a.vtt', path: 'a.vtt', size: 1, mediaId: 'subtitle-a', availability: 'available' },
			{ name: 'b.vtt', path: 'b.vtt', size: 2, mediaId: 'subtitle-b', availability: 'available' }
		];

		expect(selectPlayableSubtitle(subtitles, 'subtitle-b')?.mediaId).toBe('subtitle-b');
		expect(selectPlayableSubtitle(subtitles, 'removed')?.mediaId).toBe('subtitle-a');
		expect(selectPlayableSubtitle(subtitles, null)).toBeNull();
	});
	it('converts SRT deterministically and classifies formats', () => {
		expect(srtToWebVtt('1\r\n00:00:01,500 --> 00:00:02,500\r\nOlá\r\n')).toBe('WEBVTT\n\n00:00:01.500 --> 00:00:02.500\nOlá\n');
		expect(subtitleFormat('legenda.VTT')).toBe('vtt');
		expect(subtitleFormat('legenda.srt')).toBe('srt');
		expect(subtitleFormat('legenda.ass')).toBe('ass');
		expect(subtitleFormat('legenda.ssa')).toBe('ass');
		expect(subtitleFormat('legenda.sub')).toBe('sub');
	});
	it('formats durations for resume labels', () => {
		expect(formatDuration(0)).toBe('0:00');
		expect(formatDuration(65)).toBe('1:05');
		expect(formatDuration(3661)).toBe('1:01:01');
		expect(formatDuration(-1)).toBe('0:00');
	});
	it('advertises study sync on both transports and keeps SRT conversion on HTTP', () => {
		// Os dois transportes implementam Study: o HTTP pelas rotas e o desktop pelos
		// comandos IPC. A conversão de SRT sob demanda continua dependendo de `fetch`.
		expect(studySyncSupported).toBe(true);
		expect(srtConversionSupported).toBe(__LEARNVAULT_TRANSPORT__ === 'http');
	});
	it('only syncs study when the transport delivers a media source', () => {
		// HTTP: a fonte acompanha a mídia, então o player sincroniza como antes.
		expect(studySyncEnabled(true, 'm1', '/api/media?path=aula.mp4')).toBe(true);
		// Desktop também entrega fonte por media_id, então a barra de estudo permanece ativa.
		expect(studySyncEnabled(true, 'm1', 'learnvault-media://localhost/m1')).toBe(true);
		// Sem mídia conhecida ou com o estudo desabilitado, nada é sincronizado.
		expect(studySyncEnabled(true, null, '/api/media?path=aula.mp4')).toBe(false);
		expect(studySyncEnabled(false, 'm1', '/api/media?path=aula.mp4')).toBe(false);
	});
	it('defines supported playback speeds and sanitizes speed choices', () => {
		expect(PLAYBACK_SPEEDS).toEqual([0.75, 1, 1.25, 1.5, 1.75, 2]);
		expect(clampSpeed(1)).toBe(1);
		expect(clampSpeed(1.5)).toBe(1.5);
		expect(clampSpeed(0.5)).toBe(1);
		expect(clampSpeed(3)).toBe(1);
		expect(clampSpeed(Number.NaN)).toBe(1);
	});
	it('steps playback speed up and down within bounds', () => {
		expect(nextSpeed(1, 'up')).toBe(1.25);
		expect(nextSpeed(1.25, 'up')).toBe(1.5);
		expect(nextSpeed(2, 'up')).toBe(2);
		expect(nextSpeed(1, 'down')).toBe(0.75);
		expect(nextSpeed(0.75, 'down')).toBe(0.75);
	});
	it('handles only focused player shortcuts including j, k, l and never loops auto-next at the end', () => {
		expect(playerShortcutAction(' ', true)).toBe('toggle-playback');
		expect(playerShortcutAction('k', true)).toBe('toggle-playback');
		expect(playerShortcutAction('K', true)).toBe('toggle-playback');
		expect(playerShortcutAction('ArrowLeft', true)).toBe('seek-backward');
		expect(playerShortcutAction('j', true)).toBe('seek-backward');
		expect(playerShortcutAction('J', true)).toBe('seek-backward');
		expect(playerShortcutAction('ArrowRight', true)).toBe('seek-forward');
		expect(playerShortcutAction('l', true)).toBe('seek-forward');
		expect(playerShortcutAction('L', true)).toBe('seek-forward');
		expect(playerShortcutAction('m', true)).toBe('toggle-mute');
		expect(playerShortcutAction('M', true)).toBe('toggle-mute');
		expect(playerShortcutAction('ArrowUp', true)).toBe('volume-up');
		expect(playerShortcutAction('ArrowDown', true)).toBe('volume-down');
		expect(playerShortcutAction('n', true)).toBe('open-notes');
		expect(playerShortcutAction('N', true)).toBe('open-notes');
		expect(playerShortcutAction('0', true)).toEqual({ type: 'seek-percent', percent: 0 });
		expect(playerShortcutAction('5', true)).toEqual({ type: 'seek-percent', percent: 50 });
		expect(playerShortcutAction('9', true)).toEqual({ type: 'seek-percent', percent: 90 });
		expect(playerShortcutAction(' ', false)).toBeNull();
		expect(playerShortcutAction('k', false)).toBeNull();
		expect(playerShortcutAction('m', false)).toBeNull();
		expect(playerShortcutAction('5', false)).toBeNull();
		expect(playerShortcutAction('Enter', true)).toBeNull();
		expect(shouldAutoAdvance(true, true)).toBe(true);
		expect(shouldAutoAdvance(false, true)).toBe(false);
		expect(shouldAutoAdvance(true, false)).toBe(false);
	});
	it('calculates volume stepping and percent seek positions', () => {
		expect(stepVolume(0.5, 'up')).toBe(0.6);
		expect(stepVolume(0.95, 'up')).toBe(1);
		expect(stepVolume(1, 'up')).toBe(1);
		expect(stepVolume(0.1, 'down')).toBe(0);
		expect(stepVolume(0, 'down')).toBe(0);
		expect(seekPercentPosition(100, 50)).toBe(50);
		expect(seekPercentPosition(200, 10)).toBe(20);
		expect(seekPercentPosition(0, 50)).toBe(0);
		expect(seekPercentPosition(-10, 50)).toBe(0);
	});
	it('resumes inside the media, clamps to the duration and restarts a finished lesson', () => {
		expect(resumeTarget(42, 100)).toBe(42);
		expect(resumeTarget(120, 100)).toBe(0);
		expect(resumeTarget(99.5, 100)).toBe(0);
		expect(resumeTarget(0, 100)).toBe(0);
		expect(resumeTarget(-5, 100)).toBe(0);
		// Duration unknown: keep the saved position instead of discarding it.
		expect(resumeTarget(42, Number.NaN)).toBe(42);
		expect(resumeTarget(42, 0)).toBe(42);
	});
	it('saves periodically at five-second intervals, never on every tick', () => {
		expect(shouldSavePosition(0, 1000)).toBe(false);
		expect(shouldSavePosition(0, 5000)).toBe(true);
		expect(shouldSavePosition(5000, 9000)).toBe(false);
		expect(shouldSavePosition(5000, 10000)).toBe(true);
	});
});

function media(name: string, availability = 'available'): FileEntry {
	return { name, path: name, size: 10, mediaId: name, kind: 'media', availability };
}


describe('mediaAvailability', () => {
	it('distinguishes playable, missing and absent media', () => {
		expect(mediaAvailability([])).toBe('absent');
		expect(mediaAvailability([media('aula.mp4', 'missing')])).toBe('missing');
		expect(mediaAvailability([media('aula.mp4', 'missing'), media('aula.mp3')])).toBe('playable');
		expect(mediaAvailability([media('aula.mp4')])).toBe('playable');
	});

	it('reads availability tolerantly like the player selection', () => {
		expect(mediaAvailability([{ ...media('aula.mp4'), availability: ' Missing ' }])).toBe('missing');
	});
});

