import { useEffect, useRef } from 'react';
import { Button } from '@/components/ui/button';
import { tr } from '@/lib/i18n';
import { formatClock, modeLabel, type PomodoroMode } from '@/lib/pomodoro';

type Props = {
	/** `break` while the break runs, `over` once it ends. */
	phase: 'break' | 'over';
	mode: PomodoroMode;
	remainingSeconds: number;
	/** False while the break has not started (the timer waits for Start). */
	running: boolean;
	notes: number;
	onStart: () => void;
	onOpenNotes: () => void;
	onSkip: () => void;
	onEnd: () => void;
	onResume: () => void;
};

/**
 * Covers the video when a focus block ends: the lesson is paused, so the break is a break. It
 * offers a short review of the notes taken, and never resumes on its own (browsers block
 * autoplay anyway, and the person decides when to go back).
 */
export function BreakOverlay({ phase, mode, remainingSeconds, running, notes, onStart, onOpenNotes, onSkip, onEnd, onResume }: Props) {
	const primary = useRef<HTMLButtonElement>(null);
	useEffect(() => primary.current?.focus(), [phase]);
	return (
		<div role="region" aria-label={tr('Break', 'Pausa')} className="absolute inset-0 z-10 grid place-items-center rounded-xl bg-black/85 p-4">
			<div className="w-full max-w-sm space-y-4 rounded-2xl bg-card p-6 text-center text-card-foreground shadow-lg">
				{phase === 'break' ? (
					<>
						<p className="font-mono text-5xl font-semibold tabular-nums" aria-label={tr(`${formatClock(remainingSeconds)} remaining`, `${formatClock(remainingSeconds)} restantes`)}>{formatClock(remainingSeconds)}</p>
						<h2 className="font-serif text-2xl font-bold text-balance">{tr(`Round done. ${modeLabel(mode)}.`, `Rodada concluída. ${modeLabel(mode)}.`)}</h2>
						<p className="text-muted-foreground">{tr('The lesson is paused and waits for you.', 'A aula está pausada e espera por você.')}</p>
						{!running && <Button ref={primary} className="w-full" onClick={onStart}>{tr('Start break', 'Iniciar pausa')}</Button>}
						{notes > 0 && <Button ref={running ? primary : undefined} variant="outline" className="w-full" onClick={onOpenNotes}>{tr(`Re-read your ${notes} notes`, `Reler suas ${notes} notas`)}</Button>}
						<div className="flex justify-center gap-2">
							<Button variant="outline" onClick={onSkip}>{tr('Skip break', 'Pular pausa')}</Button>
							<Button variant="outline" onClick={onEnd}>{tr('End session', 'Encerrar sessão')}</Button>
						</div>
					</>
				) : (
					<>
						<h2 className="font-serif text-2xl font-bold text-balance">{tr('Break over', 'Pausa encerrada')}</h2>
						<p className="text-muted-foreground">{tr('Pick the lesson up where you stopped.', 'Retome a aula de onde parou.')}</p>
						<Button ref={primary} className="w-full" onClick={onResume}>{tr('Resume lesson', 'Retomar aula')}</Button>
					</>
				)}
			</div>
		</div>
	);
}
