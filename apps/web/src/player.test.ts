import { describe, expect, it } from 'vitest';
import { formatDuration, selectPlayableMedia, srtToWebVtt, studySyncSupported, subtitleFormat } from './player';

describe('player media and subtitles', () => {
	it('selects the first media that is not missing', () => {
		expect(selectPlayableMedia([
			{ name: 'a.mp4', path: 'a.mp4', size: 1, kind: 'media', availability: 'missing' },
			{ name: 'b.mp4', path: 'b.mp4', size: 2, kind: 'media', availability: 'available' }
		])?.path).toBe('b.mp4');
		expect(selectPlayableMedia([{ name: 'a.mp4', path: 'a.mp4', size: 1, availability: 'MISSING' }])).toBeNull();
	});
	it('converts SRT deterministically and classifies formats', () => {
		expect(srtToWebVtt('1\r\n00:00:01,500 --> 00:00:02,500\r\nOlá\r\n')).toBe('WEBVTT\n\n00:00:01.500 --> 00:00:02.500\nOlá\n');
		expect(subtitleFormat('legenda.VTT')).toBe('vtt');
		expect(subtitleFormat('legenda.srt')).toBe('srt');
		expect(subtitleFormat('legenda.ass')).toBe('ass');
		expect(subtitleFormat('legenda.ssa')).toBe('ass');
	});
	it('formats durations for resume labels', () => {
		expect(formatDuration(0)).toBe('0:00');
		expect(formatDuration(65)).toBe('1:05');
		expect(formatDuration(3661)).toBe('1:01:01');
		expect(formatDuration(-1)).toBe('0:00');
	});
	it('advertises study sync only on the HTTP transport build', () => {
		expect(studySyncSupported).toBe(__LEARNVAULT_TRANSPORT__ === 'http');
	});
});
