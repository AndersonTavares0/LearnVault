import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import Player from './Player.svelte';
import { clampSpeed, formatDuration, mediaAvailability, mediaPlaybackKind, nextSpeed, PLAYBACK_SPEEDS, playerShortcutAction, selectPlayableMedia, selectPlayableSubtitle, shouldAutoAdvance, srtConversionSupported, srtToWebVtt, studySyncEnabled, studySyncSupported, subtitleFormat } from './player';
import { createDefaultPlayerPreferences, updateLessonPreference } from './player-preferences';
import type { FileEntry, Lesson } from './transport';

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
		expect(playerShortcutAction(' ', false)).toBeNull();
		expect(playerShortcutAction('k', false)).toBeNull();
		expect(playerShortcutAction('Enter', true)).toBeNull();
		expect(shouldAutoAdvance(true, true)).toBe(true);
		expect(shouldAutoAdvance(false, true)).toBe(false);
		expect(shouldAutoAdvance(true, false)).toBe(false);
	});
});

function media(name: string, availability = 'available'): FileEntry {
	return { name, path: name, size: 10, mediaId: name, kind: 'media', availability };
}

function lesson(mediaFiles: FileEntry[]): Lesson {
	return { title: 'Aula', path: 'curso/aula', media: mediaFiles, subtitles: [], materials: [] };
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

describe('player without playable media', () => {
	it('renders no media element and no media URL when the file is gone from disk', () => {
		// Acesso direto pela URL da aula: o player não pode pedir um arquivo que a varredura
		// marcou como ausente — sem `<video>`, sem `<audio>` e sem URL de mídia no HTML.
		const { body } = render(Player, { props: { lesson: lesson([media('aula.mp4', 'missing')]), courseId: 'course-1', mediaError: '', onMediaError: () => undefined } });
		expect(body).toContain('A mídia desta aula não está no disco.');
		expect(body).not.toContain('<video');
		expect(body).not.toContain('<audio');
		expect(body).not.toContain('/api/media');
	});

	it('keeps the plain message for a lesson without any media file', () => {
		const { body } = render(Player, { props: { lesson: lesson([]), courseId: 'course-1', mediaError: '', onMediaError: () => undefined } });
		expect(body).toContain('Esta aula não possui mídia disponível.');
		expect(body).not.toContain('não está no disco');
	});

	it('renders the media element with its URL when the media is available', () => {
		const { body } = render(Player, { props: { lesson: lesson([media('aula.mp4')]), courseId: 'course-1', mediaError: '', onMediaError: () => undefined } });
		expect(body).toContain('<video');
		expect(body).toContain('path=aula.mp4');
	});
});

describe('player preference controls', () => {
	const baseProps = {
		lesson: lesson([media('a.mp4'), media('b.mp4')]),
		courseId: 'course-1',
		mediaError: '',
		onMediaError: () => undefined,
		onPreferencesChange: () => true
	};

	it('restores the selected media and exposes a selector only for multiple choices', () => {
		const preferences = updateLessonPreference(createDefaultPlayerPreferences(), 'course-1', 'curso/aula', { mediaId: 'b.mp4' });
		const { body } = render(Player, { props: { ...baseProps, preferences } });

		expect(body).toContain('aria-label="Mídia da aula"');
		expect(body).toContain('selected');
		expect(body).toContain('path=b.mp4');
	});

	it('offers subtitles off and restores the remembered subtitle selection', () => {
		const lessonWithSubtitles = {
			...baseProps.lesson,
			subtitles: [media('a.vtt'), media('b.vtt')].map((file) => ({ ...file, kind: 'subtitle' }))
		};
		const preferences = updateLessonPreference(createDefaultPlayerPreferences(), 'course-1', 'curso/aula', { subtitleId: 'b.vtt' });
		const { body } = render(Player, { props: { ...baseProps, lesson: lessonWithSubtitles, preferences } });

		expect(body).toContain('aria-label="Legenda da aula"');
		expect(body).toContain('Sem legendas');
		expect(body).toContain('b.vtt');
	});

	it('keeps the player shortcuts available without repeating the hint on screen', () => {
		const { body } = render(Player, { props: baseProps });

		expect(body).toContain('aria-keyshortcuts="Space ArrowLeft ArrowRight k j l [ ] f p"');
		expect(body).not.toContain('Atalhos:');
	});

	it('shows the lesson title below the media when a title is provided', () => {
		const { body } = render(Player, { props: { ...baseProps, lessonTitle: 'Aula 1' } });

		expect(body).toContain('<h2 class="player-lesson-title">Aula 1</h2>');
		expect(body.indexOf('player-lesson-title')).toBeGreaterThan(body.indexOf('</video>'));
		expect(body.indexOf('player-lesson-title')).toBeLessThan(body.indexOf('aria-label="Controles da aula"'));
	});

	it('keeps a single media choice simple and leaves auto-advance off by default', () => {
		const oneMediaLesson = lesson([media('aula.mp4')]);
		const { body } = render(Player, { props: { ...baseProps, lesson: oneMediaLesson } });

		expect(body).not.toContain('aria-label="Mídia da aula"');
		expect(body).toContain('Avançar automaticamente');
		expect(body).toContain('<input type="checkbox"/>');
	});

	it('restores the auto-advance preference', () => {
		const preferences = { ...createDefaultPlayerPreferences(), autoNext: true };
		const { body } = render(Player, { props: { ...baseProps, lesson: lesson([media('aula.mp4')]), preferences, autoNext: true } });

		expect(body).toContain('<input type="checkbox" checked=""/>');
	});

	it('renders study tools tabs with notes and flashcards when study sync is active', () => {
		const { body } = render(Player, { props: baseProps });

		expect(body).toContain('aria-label="Ferramentas de estudo da aula"');
		expect(body).toContain('Anotações da aula');
		expect(body).toContain('Cartões de revisão');
	});

	it('displays detailed explanation when mediaError is present', () => {
		const { body } = render(Player, { props: { ...baseProps, mediaError: 'Não foi possível carregar esta mídia.' } });
		expect(body).toContain('Não foi possível carregar esta mídia.');
		expect(body).toContain('O arquivo local pode ter sido movido');
	});

	it('renders quick bar with speed options, navigation, and fullscreen', () => {
		const { body } = render(Player, { props: baseProps });
		expect(body).toContain('aria-label="Controles rápidos do reprodutor"');
		expect(body).toContain('Alternar tela cheia (F)');
		expect(body).toContain('Velocidade:');
	});
});
