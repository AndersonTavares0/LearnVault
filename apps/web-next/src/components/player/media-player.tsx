import { useQuery } from '@tanstack/react-query';
import { Captions, Clapperboard, ExternalLink, Maximize, Minimize, Pause, PictureInPicture2, Play, Repeat, Rewind, FastForward, Volume2, VolumeX, Zap } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { cn } from '@/lib/utils';
import { Link } from 'react-router';
import { Button, buttonVariants } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { tr } from '@/lib/i18n';
import {
	formatDuration, mediaAvailability, mediaPlaybackKind, nextSpeed, PLAYBACK_SPEEDS, playerShortcutAction, requiresTranscodingProbe,
	seekPercentPosition, selectPlayableMedia, selectPlayableSubtitle, srtConversionSupported, srtToWebVtt, stepVolume, subtitleFormat, type PlaybackSpeed
} from '@/lib/player';
import { lessonPreferenceKey, updateLessonPreference, type PlayerPreferences } from '@/lib/player-preferences';
import { transport, type FileEntry, type Lesson } from '@/lib/transport';

export type PlayerHandle = { seek: (seconds: number) => void; pause: () => void; play: () => void };

type Props = {
	lesson: Lesson;
	courseId: string;
	initialPosition: number | null;
	preferences: PlayerPreferences;
	onPreferencesChange: (next: PlayerPreferences) => void;
	/** Called every few seconds and on pause with the absolute position. */
	onPosition: (mediaId: string, seconds: number) => void;
	onEnded: (mediaId: string) => void;
	onTime: (seconds: number) => void;
	onOpenNotes: () => void;
	/** Cinema mode: the page gives the picture the whole width. The button shows only when the page offers it. */
	cinema?: boolean;
	onCinema?: () => void;
	/** Lets the page seek the player (notes jump to their timestamp). */
	handle: (handle: PlayerHandle | null) => void;
};

const SAVE_INTERVAL_MS = 5000;

// HTTP build only. The inline build constant lets the bundler drop the `fetch` from the
// desktop bundle, which must never open the network (src/build-output.test.ts).
const loadSrtAsVtt = __LEARNVAULT_TRANSPORT__ === 'http'
	? async (url: string) => {
		const response = await fetch(url);
		if (!response.ok) throw new Error(tr('Failed to load subtitle.', 'Falha ao carregar legenda.'));
		return URL.createObjectURL(new Blob([srtToWebVtt(await response.text())], { type: 'text/vtt' }));
	}
	: async (_url: string): Promise<string | null> => null;

/** Turns a subtitle file into something `<track>` can play, or explains why it cannot. */
function useCaptions(subtitle: FileEntry | null, courseId: string) {
	const url = subtitle ? transport.mediaUrl(subtitle, courseId) : null;
	const format = subtitle ? subtitleFormat(subtitle.path) : null;
	const converted = useQuery({
		queryKey: ['captions', url],
		enabled: Boolean(url) && format === 'srt' && srtConversionSupported,
		staleTime: Infinity,
		queryFn: () => loadSrtAsVtt(url!)
	});
	if (!subtitle || !url) return { src: null, message: '', download: null };
	if (format === 'vtt') return { src: url, message: '', download: url };
	if (format === 'srt' && srtConversionSupported) return { src: converted.data ?? null, message: converted.isError ? tr('Could not convert the SRT subtitle.', 'Não foi possível converter a legenda SRT.') : '', download: url };
	const message = format === 'ass'
		? tr('ASS/SSA subtitles use styles the player cannot show. Download the file to use it in another player.', 'Legendas ASS/SSA usam estilos que o player não mostra. Baixe o arquivo para usar em outro player.')
		: format === 'sub'
			? tr('.sub subtitles are not supported by the player. Download the file to use it in another player.', 'Legendas .sub não são suportadas pelo player. Baixe o arquivo para usar em outro player.')
			: format === 'srt'
				? tr('SRT conversion needs the server app. Download the file to use it in another player.', 'A conversão de SRT exige o app do servidor. Baixe o arquivo para usar em outro player.')
				: tr('Unsupported subtitle format. Download the file to use it in another player.', 'Formato de legenda não suportado. Baixe o arquivo para usar em outro player.');
	return { src: null, message, download: url };
}

export function MediaPlayer({ lesson, courseId, initialPosition, preferences, onPreferencesChange, onPosition, onEnded, onTime, onOpenNotes, cinema = false, onCinema, handle }: Props) {
	const lessonPreference = preferences.lessons[lessonPreferenceKey(courseId, lesson.path)] ?? {};
	const available = (files: FileEntry[]) => files.filter((file) => file.availability?.trim().toLowerCase() !== 'missing');
	const playable = selectPlayableMedia(lesson.media, lessonPreference.mediaId);
	const subtitle = selectPlayableSubtitle(lesson.subtitles, lessonPreference.subtitleId);
	const captions = useCaptions(subtitle, courseId);
	const capabilities = useQuery({ queryKey: ['media-capabilities'], staleTime: Infinity, queryFn: () => transport.mediaCapabilities() });
	const mediaId = playable?.mediaId ?? null;
	const source = playable ? transport.mediaUrl(playable, courseId) : null;
	const kind = playable ? mediaPlaybackKind(playable) : 'video';
	const speed = kind === 'audio' ? preferences.audioSpeed : preferences.videoSpeed;

	// Transcoded streams restart at `offset`, so the element's own clock is relative to it.
	const [transcode, setTranscode] = useState<{ offset: number; reencode: boolean } | null>(null);
	const [errorCode, setErrorCode] = useState<number | null>(null);
	// Bumping this remounts the media element, so a failed load is requested again.
	const [attempt, setAttempt] = useState(0);
	const [notice, setNotice] = useState('');
	const media = useRef<HTMLVideoElement & HTMLAudioElement>(null);
	const container = useRef<HTMLDivElement>(null);
	const lastSave = useRef(0);
	const appliedStart = useRef<string | null>(null);
	const [pip, setPip] = useState(false);
	// What the control bar shows; the element stays the source of truth.
	const [playing, setPlaying] = useState(false);
	const [now, setNow] = useState(0);
	const [total, setTotal] = useState(0);
	const [volume, setVolume] = useState(1);
	const [muted, setMuted] = useState(false);
	const [captionsOn, setCaptionsOn] = useState(true);
	// The button's choice also applies to a subtitle file picked later, so it stays in step with the tracks.
	useEffect(() => {
		for (const track of Array.from(media.current?.textTracks ?? [])) track.mode = captionsOn ? 'showing' : 'hidden';
	}, [captionsOn, captions.src]);
	const [fullscreen, setFullscreen] = useState(false);
	useEffect(() => {
		const change = () => setFullscreen(document.fullscreenElement === container.current);
		document.addEventListener('fullscreenchange', change);
		return () => document.removeEventListener('fullscreenchange', change);
	}, []);
	// React has no props for these two events, so the element is listened to directly.
	useEffect(() => {
		const element = media.current;
		if (!element) return;
		const enter = () => setPip(true);
		const leave = () => setPip(false);
		element.addEventListener('enterpictureinpicture', enter);
		element.addEventListener('leavepictureinpicture', leave);
		return () => {
			element.removeEventListener('enterpictureinpicture', enter);
			element.removeEventListener('leavepictureinpicture', leave);
		};
	}, [errorCode, kind, source]);
	// A–B loop: A then B are set from the current time; passing B jumps back to A.
	const [loop, setLoop] = useState<{ a: number; b: number | null } | null>(null);
	const supportsPip = typeof document !== 'undefined' && Boolean(document.pictureInPictureEnabled);

	const activeSource = playable && transcode ? transport.transcodeUrl(playable, courseId, transcode.offset, transcode.reencode) ?? source : source;
	const absoluteTime = () => (media.current?.currentTime ?? 0) + (transcode?.offset ?? 0);

	// A different file resets transcoding and errors.
	const [currentSource, setCurrentSource] = useState(source);
	if (source !== currentSource) {
		setCurrentSource(source);
		setLoop(null);
		setTranscode(null);
		setErrorCode(null);
	}

	const seek = (seconds: number) => {
		const element = media.current;
		if (!element) return;
		const target = Math.max(0, element.duration ? Math.min(seconds, (transcode?.offset ?? 0) + element.duration) : seconds);
		if (transcode) setTranscode({ ...transcode, offset: target });
		else element.currentTime = target;
	};
	useEffect(() => {
		handle({
			seek: (seconds) => { seek(seconds); void media.current?.play().catch(() => undefined); },
			pause: () => media.current?.pause(),
			play: () => { void media.current?.play().catch(() => undefined); }
		});
		return () => handle(null);
	});

	useEffect(() => {
		if (media.current) media.current.playbackRate = speed;
	}, [speed, activeSource]);

	const changeSpeed = (next: PlaybackSpeed) => onPreferencesChange(kind === 'audio' ? { ...preferences, audioSpeed: next } : { ...preferences, videoSpeed: next });
	const toggleFullscreen = () => {
		if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined);
		else void container.current?.requestFullscreen().catch(() => undefined);
	};
	const togglePip = async () => {
		const element = media.current;
		if (!(element instanceof HTMLVideoElement)) return;
		try {
			if (document.pictureInPictureElement === element) await document.exitPictureInPicture();
			else await element.requestPictureInPicture();
		} catch {
			// The browser may refuse picture-in-picture; nothing else depends on it.
		}
	};
	const openSystemPlayer = async () => {
		if (!playable || !transport.openInSystemPlayer) return;
		try {
			const opened = await transport.openInSystemPlayer(playable);
			setNotice(opened ? tr('Opened in your system media player.', 'Aberto no player de mídia do sistema.') : '');
		} catch {
			setNotice(tr('Could not open the system player.', 'Não foi possível abrir o player do sistema.'));
		}
	};
	const retry = () => {
		setErrorCode(null);
		setAttempt((count) => count + 1);
	};
	const startTranscode = (reencode: boolean) => {
		setTranscode({ offset: absoluteTime() || initialPosition || 0, reencode });
		setErrorCode(null);
	};

	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		if (event.ctrlKey || event.metaKey || event.altKey) return;
		const target = event.target as HTMLElement;
		if (target.closest('input, textarea, select, [contenteditable="true"]')) return;
		const element = media.current;
		if (event.key === '[' || event.key === ']') { event.preventDefault(); changeSpeed(nextSpeed(speed, event.key === '[' ? 'down' : 'up')); return; }
		if (event.key.toLowerCase() === 'f' && kind === 'video') { event.preventDefault(); toggleFullscreen(); return; }
		if (event.key.toLowerCase() === 't' && kind === 'video' && onCinema) { event.preventDefault(); onCinema(); return; }
		if (event.key.toLowerCase() === 'p' && kind === 'video' && supportsPip) { event.preventDefault(); void togglePip(); return; }
		// Space on a focused button presses that button.
		if (target.tagName === 'BUTTON' && event.key === ' ') return;
		const action = playerShortcutAction(event.key, true);
		if (!action || !element) return;
		event.preventDefault();
		if (action === 'toggle-playback') void (element.paused ? element.play().catch(() => undefined) : element.pause());
		else if (action === 'seek-backward') seek(absoluteTime() - 10);
		else if (action === 'seek-forward') seek(absoluteTime() + 10);
		else if (action === 'toggle-mute') element.muted = !element.muted;
		else if (action === 'volume-up') { element.volume = stepVolume(element.volume, 'up'); element.muted = false; }
		else if (action === 'volume-down') element.volume = stepVolume(element.volume, 'down');
		else if (action === 'open-notes') onOpenNotes();
		else if (typeof action === 'object' && element.duration > 0) seek(seekPercentPosition((transcode?.offset ?? 0) + element.duration, action.percent));
	};

	const errorMessage = useMemo(() => {
		if (errorCode === 4) return [tr('This media format or codec is not supported here.', 'Este formato ou codec de mídia não é suportado aqui.'), requiresTranscodingProbe(playable?.name ?? '') ? tr('This kind of file usually needs on-demand transcoding or the system player.', 'Esse tipo de arquivo geralmente precisa de transcodificação sob demanda ou do player do sistema.') : tr('Examples: HEVC/H.265 video or AC3 audio.', 'Exemplos: vídeo HEVC/H.265 ou áudio AC3.')];
		if (errorCode === 3) return [tr('The media could not be decoded.', 'Não foi possível decodificar a mídia.'), tr('The file may be damaged or use a codec your hardware cannot decode.', 'O arquivo pode estar corrompido ou usar um codec que seu hardware não decodifica.')];
		return [tr('Could not load this media.', 'Não foi possível carregar esta mídia.'), tr('The file may have moved or its drive may be disconnected. Reconnect it and use Refresh courses in My learning; your progress is kept.', 'O arquivo pode ter sido movido ou o disco desconectado. Reconecte e use Atualizar cursos em Meu aprendizado; seu progresso fica salvo.')];
	}, [errorCode, playable?.name]);

	if (!activeSource) {
		const missing = mediaAvailability(lesson.media) === 'missing';
		return (
			<div role="status" className="rounded-lg border bg-card p-6">
				<p className="font-medium">{missing ? tr('Media unavailable', 'Mídia indisponível') : tr('This lesson has no media to play.', 'Esta aula não tem mídia para reproduzir.')}</p>
				{missing && <p className="mt-1 text-sm text-muted-foreground">{tr('The media for this lesson is not on disk. The lesson and your progress are kept. Reconnect the drive or restore the file, then use Refresh courses.', 'A mídia desta aula não está no disco. A aula e o seu progresso foram preservados. Reconecte o disco ou reponha o arquivo e use Atualizar cursos.')}</p>}
			</div>
		);
	}

	const mediaProps = {
		ref: media,
		src: activeSource,
		// The browser's own controls are off: the bar below is the one set of controls.
		preload: 'metadata',
		'aria-label': tr(`Lesson player: ${lesson.title}`, `Player da aula: ${lesson.title}`),
		onLoadedMetadata: () => {
			const element = media.current!;
			element.playbackRate = speed;
			// Keep the last known length when the element reports none (a transcoded stream can).
			if (Number.isFinite(element.duration)) setTotal((transcode?.offset ?? 0) + element.duration);
			// Loading a new source leaves the element paused without a pause event, so the bar is told here.
			setPlaying(!element.paused);
			setVolume(element.volume);
			setMuted(element.muted);
			if (!transcode && initialPosition && appliedStart.current !== `${activeSource}@${initialPosition}`) {
				element.currentTime = initialPosition;
				appliedStart.current = `${activeSource}@${initialPosition}`;
			}
		},
		onTimeUpdate: () => {
			const at = absoluteTime();
			if (loop?.b != null && at >= loop.b) { seek(loop.a); return; }
			setNow(at);
			onTime(at);
			if (mediaId && performance.now() - lastSave.current > SAVE_INTERVAL_MS && at > 0) {
				lastSave.current = performance.now();
				onPosition(mediaId, Math.floor(at));
			}
		},
		onEmptied: () => setPlaying(false),
		onDurationChange: () => { const element = media.current; if (element && Number.isFinite(element.duration)) setTotal((transcode?.offset ?? 0) + element.duration); },
		onVolumeChange: () => { const element = media.current; if (element) { setVolume(element.volume); setMuted(element.muted); } },
		onPlay: () => setPlaying(true),
		onPause: () => { setPlaying(false); if (mediaId && absoluteTime() > 0) onPosition(mediaId, Math.floor(absoluteTime())); },
		onEnded: () => { setPlaying(false); if (mediaId) onEnded(mediaId); },
		onError: () => setErrorCode(media.current?.error?.code ?? null),
		className: 'w-full bg-black'
	} as const;

	const onVideo = kind === 'video';
	const hasChoices = available(lesson.media).length > 1 || available(lesson.subtitles).length > 0;
	// On the picture the bar is white on a dark fade; under audio it is an ordinary card.
	const look = cn('size-11 rounded-full', onVideo && 'text-white hover:bg-white/20 hover:text-white aria-pressed:bg-white/25');
	const loopText = !loop ? null : loop.b === null ? tr(`Loop from ${formatDuration(loop.a)}: set end`, `Laço de ${formatDuration(loop.a)}: marcar fim`) : tr(`Looping ${formatDuration(loop.a)}–${formatDuration(loop.b)}: clear`, `Repetindo ${formatDuration(loop.a)}–${formatDuration(loop.b)}: limpar`);
	const togglePlay = () => { const element = media.current; if (element) void (element.paused ? element.play().catch(() => undefined) : element.pause()); };
	const toggleCaptions = () => {
		setCaptionsOn(!captionsOn);
	};
	const span = Math.max(1, Math.floor(total));
	const bar = (
		<div className={cn('space-y-1', onVideo ? 'relative bg-black px-3 pt-1 pb-2 text-white sm:absolute sm:inset-x-0 sm:bottom-0 sm:z-10 sm:bg-transparent sm:bg-gradient-to-t sm:from-black/85 sm:via-black/55 sm:to-transparent sm:pt-12 sm:transition-opacity' : 'rounded-[1.5rem] border bg-card p-3', onVideo && (playing ? 'sm:opacity-0 sm:group-hover/player:opacity-100 sm:group-focus-within/player:opacity-100 sm:[@media(hover:none)]:opacity-100' : ''))}>
			<input type="range" aria-label={tr('Seek', 'Posição')} min={0} max={span} step={1} value={Math.min(Math.floor(now), span)} aria-valuetext={`${formatDuration(now)} / ${formatDuration(total)}`}
				onChange={(event) => seek(Number(event.target.value))} className="block h-5 w-full cursor-pointer accent-[var(--reward-fill)]" />
			<div role="toolbar" aria-label={tr('Player controls', 'Controles do player')} className="flex flex-wrap items-center gap-x-0.5">
				<Button variant="ghost" size="icon" className={look} onClick={togglePlay} aria-label={playing ? tr('Pause', 'Pausar') : tr('Play', 'Reproduzir')} aria-keyshortcuts="k Space">{playing ? <Pause /> : <Play />}</Button>
				<Button variant="ghost" size="icon" className={look} onClick={() => seek(absoluteTime() - 10)} aria-keyshortcuts="ArrowLeft j"><Rewind /><span className="sr-only">{tr('Back 10s', 'Voltar 10s')}</span></Button>
				<Button variant="ghost" size="icon" className={look} onClick={() => seek(absoluteTime() + 10)} aria-keyshortcuts="ArrowRight l"><FastForward /><span className="sr-only">{tr('Forward 10s', 'Avançar 10s')}</span></Button>
				<Button variant="ghost" size="icon" className={look} aria-pressed={muted} onClick={() => { if (media.current) media.current.muted = !media.current.muted; }} aria-label={muted ? tr('Unmute', 'Ativar o som') : tr('Mute', 'Silenciar')} aria-keyshortcuts="m">{muted || volume === 0 ? <VolumeX /> : <Volume2 />}</Button>
				<input type="range" aria-label={tr('Volume', 'Volume')} min={0} max={1} step={0.05} value={muted ? 0 : volume} onChange={(event) => { const element = media.current; if (!element) return; element.volume = Number(event.target.value); element.muted = false; }} className="hidden h-5 w-20 cursor-pointer accent-[var(--reward-fill)] sm:block" />
				<span className="mx-2 text-sm tabular-nums"><span className="sr-only">{tr('Time', 'Tempo')}: </span>{formatDuration(now)} <span className={onVideo ? 'text-white/70' : 'text-muted-foreground'}>/ {formatDuration(total)}</span></span>
				<div className="ml-auto flex flex-wrap items-center justify-end">
					{captions.src && <Button variant="ghost" size="icon" className={look} aria-pressed={captionsOn} onClick={toggleCaptions} aria-label={tr('Subtitles', 'Legendas')}><Captions /></Button>}
					<Button variant="ghost" size={loopText ? 'sm' : 'icon'} className={cn(look, loopText && 'h-11 w-auto px-3 text-xs')} aria-pressed={Boolean(loop)} onClick={() => setLoop(!loop ? { a: absoluteTime(), b: null } : loop.b === null ? (absoluteTime() > loop.a + 1 ? { a: loop.a, b: absoluteTime() } : loop) : null)}>
						<Repeat />{loopText ?? <span className="sr-only">{tr('Loop', 'Laço')}</span>}
					</Button>
					<Label htmlFor="player-speed" className="sr-only">{tr('Speed', 'Velocidade')}</Label>
					<select id="player-speed" value={speed} onChange={(event) => changeSpeed(Number(event.target.value) as PlaybackSpeed)} className={cn('h-11 w-16 cursor-pointer appearance-none rounded-full bg-transparent px-2 text-center text-sm font-medium', onVideo ? 'text-white hover:bg-white/20 [&>option]:bg-card [&>option]:text-foreground' : 'hover:bg-muted')} aria-keyshortcuts="[ ]">
						{PLAYBACK_SPEEDS.map((value) => <option key={value} value={value}>{value}×</option>)}
					</select>
					{capabilities.data?.transcoding && (
						<Button variant="ghost" size="icon" className={look} aria-pressed={Boolean(transcode)} onClick={() => (transcode ? setTranscode(null) : startTranscode(false))}><Zap /><span className="sr-only">{tr('Transcode', 'Transcodificar')}</span></Button>
					)}
					{capabilities.data?.systemPlayer && <Button variant="ghost" size="icon" className={look} onClick={openSystemPlayer} aria-label={tr('Open in system player', 'Abrir no player do sistema')}><ExternalLink /></Button>}
					{onVideo && supportsPip && <Button variant="ghost" size="icon" className={look} aria-pressed={pip} onClick={togglePip} aria-label={tr('Picture-in-picture', 'Picture-in-picture')} aria-keyshortcuts="p"><PictureInPicture2 /></Button>}
					{onVideo && onCinema && <Button variant="ghost" size="icon" className={cn(look, 'hidden lg:inline-flex')} aria-pressed={cinema} onClick={onCinema} aria-label={tr('Cinema mode', 'Modo cinema')} aria-keyshortcuts="t"><Clapperboard /></Button>}
					{onVideo && <Button variant="ghost" size="icon" className={look} onClick={toggleFullscreen} aria-label={fullscreen ? tr('Exit fullscreen', 'Sair da tela cheia') : tr('Fullscreen', 'Tela cheia')} aria-keyshortcuts="f">{fullscreen ? <Minimize /> : <Maximize />}</Button>}
				</div>
			</div>
		</div>
	);

	return (
		<div className="space-y-3">
			<p role="status" aria-live="polite" className="text-sm text-muted-foreground empty:hidden">{notice}</p>
			{errorCode !== null ? (
				<div role="alert" className="space-y-3 rounded-lg border border-destructive/40 bg-card p-6">
					<p className="font-medium">{errorMessage[0]}</p>
					<p className="text-sm text-muted-foreground">{errorMessage[1]}</p>
					<div className="flex flex-wrap gap-2">
						{errorCode !== 4 && <Button variant="outline" onClick={retry}>{tr('Try again', 'Tentar de novo')}</Button>}
						{capabilities.data?.transcoding && <Button onClick={() => startTranscode(false)}><Zap />{tr('Play with on-demand transcoding', 'Reproduzir com transcodificação sob demanda')}</Button>}
						{capabilities.data?.transcoding && <Button variant="outline" onClick={() => startTranscode(true)}>{tr('Re-encode video (H.264)', 'Recodificar vídeo (H.264)')}</Button>}
						{capabilities.data?.systemPlayer && <Button variant="outline" onClick={openSystemPlayer}><ExternalLink />{tr('Open in system player', 'Abrir no player do sistema')}</Button>}
						<Link to="/" className={buttonVariants({ variant: 'ghost' })}>{tr('Go to My learning', 'Ir para Meu aprendizado')}</Link>
					</div>
				</div>
			) : (
				// The group takes focus so single-key shortcuts only act on the player (WCAG 2.1.4).
				// eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex, jsx-a11y/no-noninteractive-element-interactions
				<div ref={container} role="group" tabIndex={0} onKeyDown={onKeyDown} data-player aria-label={tr('Media player', 'Player de mídia')} aria-describedby="player-shortcuts" aria-keyshortcuts="Space ArrowLeft ArrowRight ArrowUp ArrowDown k j l [ ] f p m n 0 1 2 3 4 5 6 7 8 9" className="relative group/player overflow-hidden rounded-xl bg-black focus-visible:ring-3 focus-visible:ring-ring">
					{/* Captions come from the course's own subtitle files; many lessons have none (WCAG 1.2.2 depends on the content). */}
					{kind === 'audio' ? (
						// eslint-disable-next-line jsx-a11y/media-has-caption
						<audio key={attempt} {...mediaProps}>{captions.src && <track kind="captions" src={captions.src} label={tr('Subtitles', 'Legenda')} default />}</audio>
					) : (
						// eslint-disable-next-line jsx-a11y/media-has-caption
						<video key={attempt} {...mediaProps} onClick={togglePlay} className="aspect-video w-full cursor-pointer bg-black lg:max-h-[calc(100svh-19rem)]">{captions.src && <track kind="captions" src={captions.src} label={tr('Subtitles', 'Legenda')} default />}</video>
					)}
					{bar}
				</div>
			)}

			{hasChoices && (
				<div className="flex flex-wrap items-center gap-2">
					{available(lesson.media).length > 1 && (
					<>
						<Label htmlFor="player-media" className="ml-2">{tr('Video', 'Vídeo')}</Label>
						<select id="player-media" value={mediaId ?? ''} onChange={(event) => onPreferencesChange(updateLessonPreference(preferences, courseId, lesson.path, { mediaId: event.target.value }))} className="h-11 max-w-48 rounded-lg border border-input bg-background px-2 text-sm">
							{available(lesson.media).map((file) => <option key={file.mediaId} value={file.mediaId}>{file.name}</option>)}
						</select>
					</>
				)}
				{available(lesson.subtitles).length > 0 && (
					<>
						<Label htmlFor="player-subtitles" className="ml-2">{tr('Subtitles', 'Legendas')}</Label>
						<select id="player-subtitles" value={lessonPreference.subtitleId === null ? '' : subtitle?.mediaId ?? ''} onChange={(event) => onPreferencesChange(updateLessonPreference(preferences, courseId, lesson.path, { subtitleId: event.target.value || null }))} className="h-11 max-w-48 rounded-lg border border-input bg-background px-2 text-sm">
							<option value="">{tr('Off', 'Desligadas')}</option>
							{available(lesson.subtitles).map((file) => <option key={file.mediaId} value={file.mediaId}>{file.name}</option>)}
						</select>
					</>
				)}
				</div>
			)}
			{captions.message && (
				<p role="status" className="text-sm text-muted-foreground">
					{captions.message} {captions.download && <a href={captions.download} download className="font-medium text-primary underline-offset-4 hover:underline">{tr('Download subtitle', 'Baixar legenda')}</a>}
				</p>
			)}
			<p id="player-shortcuts" className="sr-only">{tr('Shortcuts while the player has focus: space or k plays and pauses, left and right arrows seek 10 seconds, up and down arrows change the volume, m mutes, f is fullscreen, p is picture-in-picture, brackets change speed, n adds a note, digits jump to that tenth of the lesson.', 'Atalhos com o player em foco: espaço ou k reproduz e pausa, setas esquerda e direita avançam 10 segundos, setas para cima e para baixo mudam o volume, m silencia, f é tela cheia, p é picture-in-picture, colchetes mudam a velocidade, n adiciona nota, dígitos saltam para aquele décimo da aula.')}</p>
		</div>
	);
}
