import { describe, expect, it } from 'vitest';
import { formatDuration, selectPlayableMedia, srtConversionSupported, srtToWebVtt, studySyncEnabled, studySyncSupported, subtitleFormat } from './player';

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
	it('advertises study sync on both transports and keeps SRT conversion on HTTP', () => {
		// Os dois transportes implementam Study: o HTTP pelas rotas e o desktop pelos
		// comandos IPC. A conversão de SRT sob demanda continua dependendo de `fetch`.
		expect(studySyncSupported).toBe(true);
		expect(srtConversionSupported).toBe(__LEARNVAULT_TRANSPORT__ === 'http');
	});
	it('only syncs study when the transport delivers a media source', () => {
		// HTTP: a fonte acompanha a mídia, então o player sincroniza como antes.
		expect(studySyncEnabled(true, 'm1', '/api/media?path=aula.mp4')).toBe(true);
		// Desktop: a mídia existe no catálogo, mas o transporte não entrega fonte — sem
		// reprodução não há posição real, então a barra de estudo permanece fechada.
		expect(studySyncEnabled(true, 'm1', null)).toBe(false);
		// Sem mídia conhecida ou com o estudo desabilitado, nada é sincronizado.
		expect(studySyncEnabled(true, null, '/api/media?path=aula.mp4')).toBe(false);
		expect(studySyncEnabled(false, 'm1', '/api/media?path=aula.mp4')).toBe(false);
	});
});
