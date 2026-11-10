import { Timer } from 'lucide-react';
import { useState } from 'react';
import { Link } from 'react-router';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { tr } from '@/lib/i18n';
import { formatClock, modeLabel, POMODORO_MODES } from '@/lib/pomodoro';
import { abandonTimer, focusSync, pauseTimer, pomodoro, resetTimer, retryFocusSessions, setMode, skipTimer, startTimer } from '@/lib/pomodoro-store';
import { cn } from '@/lib/utils';

/**
 * Pomodoro in the topbar. The timer keeps running while you move between screens; the
 * dialog only announces mode changes and completions, never every second (WCAG 4.1.3).
 */
export function FocusMenu() {
	useLocale();
	const state = useStore(pomodoro);
	const sync = useStore(focusSync);
	const [open, setOpen] = useState(false);
	const [confirmAbandon, setConfirmAbandon] = useState(false);
	const running = state.status === 'running';
	const paused = state.status === 'paused';
	const status = running ? tr('running', 'em andamento') : paused ? tr('paused', 'pausado') : tr('ready', 'pronto');
	const clock = formatClock(state.remainingSeconds);

	// Completion summary; the live region announces it when a block finishes.
	const announcement = !state.lastCompleted ? '' : state.lastCompleted.mode === 'focus'
		? tr(`Focus block done: ${Math.round(state.settings.focusMinutes)} minutes. Take a break.`, `Bloco de foco concluído: ${Math.round(state.settings.focusMinutes)} minutos. Faça uma pausa.`)
		: tr('Break over. Back to the lesson when you are ready.', 'Pausa encerrada. Volte para a aula quando quiser.');

	return (
		<Dialog open={open} onOpenChange={(next) => { setOpen(next); setConfirmAbandon(false); }}>
			<DialogTrigger render={<Button variant={running ? 'secondary' : 'ghost'} size={running || paused ? 'sm' : 'icon'} aria-label={tr(`Pomodoro: ${status}, ${modeLabel(state.mode)}, ${clock}`, `Pomodoro: ${status}, ${modeLabel(state.mode)}, ${clock}`)} />}>
				<Timer />{(running || paused) && <span className="font-mono tabular-nums">{clock}</span>}
			</DialogTrigger>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>Pomodoro</DialogTitle>
					<DialogDescription>{tr(`Cycle ${state.cycle + 1} · ${status}`, `Ciclo ${state.cycle + 1} · ${status}`)}</DialogDescription>
				</DialogHeader>
				<div role="group" aria-label={tr('Timer mode', 'Modo do timer')} className="grid grid-cols-3 gap-1">
					{POMODORO_MODES.map((mode) => (
						<Button key={mode} size="sm" variant={state.mode === mode ? 'secondary' : 'ghost'} aria-pressed={state.mode === mode} onClick={() => setMode(mode)}>{modeLabel(mode)}</Button>
					))}
				</div>
				<p className={cn('text-center font-mono text-6xl font-semibold tabular-nums', running && 'text-primary')} aria-label={tr(`${clock} remaining`, `${clock} restantes`)}>{clock}</p>
				<p role="status" aria-live="polite" className="text-center text-sm text-muted-foreground empty:hidden">{announcement}</p>
				<div className="flex flex-wrap justify-center gap-2">
					<Button onClick={() => (running ? pauseTimer() : startTimer())}>{running ? tr('Pause', 'Pausar') : paused ? tr('Resume', 'Retomar') : tr('Start', 'Iniciar')}</Button>
					<Button variant="outline" onClick={resetTimer}>{tr('Reset', 'Reiniciar')}</Button>
					<Button variant="ghost" onClick={skipTimer}>{tr('Skip', 'Pular')}</Button>
					{state.mode === 'focus' && state.startedAt !== null && <Button variant="ghost" onClick={() => setConfirmAbandon(true)}>{tr('Give up', 'Desistir')}</Button>}
				</div>
				{confirmAbandon && (
					<div role="alert" className="space-y-2 rounded-lg border p-3 text-sm">
						<p>{tr('Give up this focus block? It is saved as unfinished.', 'Desistir deste bloco de foco? Ele fica registrado como não concluído.')}</p>
						<div className="flex gap-2">
							<Button size="sm" variant="destructive" onClick={() => { abandonTimer(); setConfirmAbandon(false); }}>{tr('Give up', 'Desistir')}</Button>
							<Button size="sm" variant="outline" onClick={() => setConfirmAbandon(false)}>{tr('Keep focusing', 'Continuar focando')}</Button>
						</div>
					</div>
				)}
				{sync.error ? (
					<p role="status" className="text-sm">{sync.error} <Button variant="link" size="sm" disabled={sync.saving} onClick={() => void retryFocusSessions()}>{tr('Try again', 'Tentar de novo')}</Button></p>
				) : sync.pending.length > 0 ? (
					<p role="status" className="text-sm text-muted-foreground">{tr(`${sync.pending.length} session(s) waiting to sync.`, `${sync.pending.length} sessão(ões) aguardando sincronização.`)}</p>
				) : null}
				<div className="flex justify-between text-sm">
					<Link to="/progress" onClick={() => setOpen(false)} className="inline-flex min-h-11 items-center text-primary hover:underline">{tr('See progress', 'Ver progresso')}</Link>
					<Link to="/settings#focus" onClick={() => setOpen(false)} className="inline-flex min-h-11 items-center text-primary hover:underline">{tr('Adjust timers', 'Ajustar tempos')}</Link>
				</div>
			</DialogContent>
		</Dialog>
	);
}
