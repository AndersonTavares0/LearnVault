import { Pause, Play, Square } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { tr } from '@/lib/i18n';
import { formatClock, modeLabel } from '@/lib/pomodoro';
import { pauseTimer, pomodoro, resetTimer, startTimer } from '@/lib/pomodoro-store';

/**
 * The Pomodoro timer beside the lesson: a ring, the round and its controls. It is the same timer
 * as the one in the top bar, so there is one source of truth; the bar shows it when you are
 * elsewhere. The clock is not announced every second (WCAG 4.1.3); the break screen announces
 * the changes that matter.
 */
export function FocusCard() {
	useLocale();
	const state = useStore(pomodoro);
	const running = state.status === 'running';
	const idle = state.status === 'idle';
	const percent = state.totalSeconds > 0 ? 1 - state.remainingSeconds / state.totalSeconds : 0;
	const circumference = 2 * Math.PI * 26;
	const clock = formatClock(state.remainingSeconds);
	const title = state.mode === 'focus' ? tr(`Focus, round ${state.cycle + 1}`, `Foco, rodada ${state.cycle + 1}`) : modeLabel(state.mode);
	return (
		<section aria-label={tr('Focus timer', 'Timer de foco')} className="flex items-center gap-4 rounded-xl border bg-card p-4">
			<div className="relative size-16 shrink-0">
				<svg viewBox="0 0 64 64" aria-hidden="true" className="size-full -rotate-90">
					<circle cx="32" cy="32" r="26" fill="none" strokeWidth="6" className="stroke-muted" />
					<circle cx="32" cy="32" r="26" fill="none" strokeWidth="6" strokeLinecap="round" strokeDasharray={`${circumference * percent} ${circumference}`} className="stroke-primary" />
				</svg>
				<span className="absolute inset-0 grid place-items-center font-mono text-sm font-semibold tabular-nums">{clock}</span>
			</div>
			<div className="min-w-0 flex-1">
				<p className="font-semibold">{title}</p>
				<p className="text-sm text-muted-foreground">{idle ? tr('Start a round for this lesson.', 'Inicie uma rodada para esta aula.') : running ? tr('Running', 'Em andamento') : tr('Paused', 'Pausado')}</p>
			</div>
			<div className="flex gap-1.5">
				<Button variant={idle ? 'default' : 'outline'} size="icon" aria-label={running ? tr('Pause timer', 'Pausar timer') : idle ? tr('Start timer', 'Iniciar timer') : tr('Resume timer', 'Retomar timer')} onClick={() => (running ? pauseTimer() : startTimer())}>
					{running ? <Pause /> : <Play />}
				</Button>
				{!idle && <Button variant="outline" size="icon" aria-label={tr('Stop session', 'Encerrar sessão')} onClick={resetTimer}><Square /></Button>}
			</div>
		</section>
	);
}
