import { Bell, Check, Clock, Volume2 } from 'lucide-react';
import { useState, type FormEvent } from 'react';
import { SwitchCard } from './switch-card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useStore } from '@/hooks/use-store';
import { tr } from '@/lib/i18n';
import { cn } from '@/lib/utils';
import { DEFAULT_POMODORO_SETTINGS, playPomodoroAlarm, type AlarmSound, type PomodoroSettings } from '@/lib/pomodoro';
import { pomodoro, saveSettings } from '@/lib/pomodoro-store';

const numberFields: { key: 'focusMinutes' | 'shortMinutes' | 'longMinutes' | 'longEvery'; label: () => string; min: number; max: number }[] = [
	{ key: 'focusMinutes', label: () => tr('Focus (minutes)', 'Foco (minutos)'), min: 1, max: 120 },
	{ key: 'shortMinutes', label: () => tr('Short break (minutes)', 'Pausa curta (minutos)'), min: 1, max: 60 },
	{ key: 'longMinutes', label: () => tr('Long break (minutes)', 'Pausa longa (minutos)'), min: 1, max: 120 },
	{ key: 'longEvery', label: () => tr('Long break every (blocks)', 'Pausa longa a cada (blocos)'), min: 2, max: 12 }
];

const PRESETS = [
	{ name: () => tr('Classic', 'Clássico'), focusMinutes: 25, shortMinutes: 5, longMinutes: 15 },
	{ name: () => tr('Deep', 'Profundo'), focusMinutes: 50, shortMinutes: 10, longMinutes: 30 },
	{ name: () => tr('Light', 'Leve'), focusMinutes: 15, shortMinutes: 3, longMinutes: 10 }
];

/** Pomodoro durations and alarm; stored in this browser like the rest of the timer. */
export function FocusSettings() {
	const state = useStore(pomodoro);
	const [draft, setDraft] = useState<PomodoroSettings>(state.settings);
	const [message, setMessage] = useState('');
	const set = <K extends keyof PomodoroSettings>(key: K, value: PomodoroSettings[K]) => setDraft({ ...draft, [key]: value });

	const submit = (event: FormEvent) => {
		event.preventDefault();
		saveSettings({ ...draft });
		setMessage(tr('Timer settings saved.', 'Ajustes do timer salvos.'));
	};
	const askNotifications = async () => {
		if (typeof Notification === 'undefined') return setMessage(tr('This browser does not support notifications.', 'Este navegador não oferece notificações.'));
		const permission = await Notification.requestPermission().catch(() => 'denied');
		setMessage(permission === 'granted' ? tr('Notifications allowed.', 'Notificações permitidas.') : tr('Notifications were not allowed.', 'Notificações não foram permitidas.'));
	};

	return (
		<form onSubmit={submit} className="space-y-6">
			<fieldset className="space-y-2.5">
				<legend className="text-sm font-semibold">{tr('Preset', 'Predefinição')}</legend>
				<div className="grid gap-2.5 sm:grid-cols-3">
					{PRESETS.map((preset) => {
						const active = draft.focusMinutes === preset.focusMinutes && draft.shortMinutes === preset.shortMinutes && draft.longMinutes === preset.longMinutes;
						return (
							<button
								key={preset.name()}
								type="button"
								aria-pressed={active}
								onClick={() => setDraft({ ...draft, focusMinutes: preset.focusMinutes, shortMinutes: preset.shortMinutes, longMinutes: preset.longMinutes })}
								className={cn(
									'flex min-h-14 flex-col items-start justify-center gap-0.5 rounded-2xl border px-4 py-3 text-left transition-colors duration-150 focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none',
									active ? 'border-primary bg-brand-soft text-primary ring-1 ring-primary/20' : 'border-border bg-background hover:bg-muted/60'
								)}
							>
								<span className="flex w-full items-center justify-between text-sm font-semibold">
									<span>{preset.name()}</span>
									{active && <Check className="size-4 text-primary" aria-hidden="true" />}
								</span>
								<span className={cn('text-xs tabular-nums', active ? 'text-primary/85' : 'text-muted-foreground')}>
									{tr(`${preset.focusMinutes} min focus · ${preset.shortMinutes}m break`, `${preset.focusMinutes} min foco · ${preset.shortMinutes}m pausa`)}
								</span>
							</button>
						);
					})}
				</div>
			</fieldset>

			<div className="grid gap-5 lg:grid-cols-2">
				<div className="space-y-4 rounded-2xl border bg-panel/45 p-4 sm:p-5">
					<div className="flex items-center gap-2 text-sm font-semibold">
						<Clock className="size-4 text-primary" aria-hidden="true" />
						<span>{tr('Durations', 'Durações')}</span>
					</div>
					<div className="grid gap-3.5 sm:grid-cols-2">
						{numberFields.map((field) => (
							<div key={field.key} className="space-y-1.5">
								<Label htmlFor={`timer-${field.key}`}>{field.label()}</Label>
								<Input id={`timer-${field.key}`} type="number" inputMode="numeric" min={field.min} max={field.max} required value={draft[field.key]} onChange={(event) => set(field.key, Number(event.target.value))} />
							</div>
						))}
					</div>
				</div>

				<div className="space-y-4 rounded-2xl border bg-panel/45 p-4 sm:p-5">
					<div className="flex items-center gap-2 text-sm font-semibold">
						<Volume2 className="size-4 text-primary" aria-hidden="true" />
						<span>{tr('Alarm and sound', 'Alarme e som')}</span>
					</div>
					<div className="grid gap-3.5 sm:grid-cols-2">
						<div className="space-y-1.5">
							<Label htmlFor="timer-alarmSound">{tr('Alarm sound', 'Som do alarme')}</Label>
							<select id="timer-alarmSound" value={draft.alarmSound} onChange={(event) => set('alarmSound', event.target.value as AlarmSound)} className="h-11 w-full rounded-lg border border-input bg-background px-3 text-sm">
								<option value="sine">{tr('Soft', 'Suave')}</option>
								<option value="triangle">{tr('Clear', 'Claro')}</option>
								<option value="square">{tr('Bold', 'Marcante')}</option>
							</select>
						</div>
						<div className="space-y-1.5">
							<Label htmlFor="timer-alarmRepeats">{tr('Alarm repeats', 'Repetições do alarme')}</Label>
							<Input id="timer-alarmRepeats" type="number" inputMode="numeric" min={1} max={5} required value={draft.alarmRepeats} onChange={(event) => set('alarmRepeats', Number(event.target.value))} />
						</div>
					</div>
					<div className="space-y-1.5">
						<div className="flex items-center justify-between gap-2">
							<Label htmlFor="timer-volume">{tr('Alarm volume', 'Volume do alarme')}</Label>
							<span className="text-xs font-medium text-muted-foreground tabular-nums">{Math.round(draft.alarmVolume * 100)}%</span>
						</div>
						<input id="timer-volume" type="range" min={0} max={1} step={0.05} value={draft.alarmVolume} onChange={(event) => set('alarmVolume', Number(event.target.value))} className="h-8 w-full cursor-pointer accent-[var(--primary)]" />
					</div>
					<div className="flex flex-wrap gap-2 pt-1">
						<Button type="button" size="sm" variant="outline" onClick={() => playPomodoroAlarm(draft.alarmSound, draft.alarmVolume, 1)}>{tr('Test alarm', 'Testar alarme')}</Button>
						<Button type="button" size="sm" variant="outline" onClick={() => void askNotifications()}><Bell className="size-3.5" aria-hidden="true" />{tr('Allow notifications', 'Permitir notificações')}</Button>
					</div>
				</div>
			</div>

			<div className="space-y-2.5">
				<p className="text-sm font-semibold">{tr('Automation', 'Automação')}</p>
				<div className="grid gap-2.5 sm:grid-cols-2">
					{([
						['autoStartNext', tr('Start the next block automatically', 'Iniciar o próximo bloco automaticamente')],
						['pauseVideoOnRoundEnd', tr('Pause the lesson when a focus block ends', 'Pausar a aula quando um bloco de foco termina')],
						['alarmEnabled', tr('Play the alarm when a block ends', 'Tocar o alarme ao fim do bloco')],
						['notificationsEnabled', tr('Show a system notification', 'Mostrar notificação do sistema')]
					] as const).map(([key, label]) => (
						<SwitchCard key={key} label={label} checked={draft[key]} onChange={(checked) => set(key, checked)} />
					))}
				</div>
			</div>

			<div className="flex flex-wrap items-center justify-between gap-3 border-t pt-4">
				<Button type="button" variant="ghost" onClick={() => { setDraft({ ...DEFAULT_POMODORO_SETTINGS }); setMessage(tr('Defaults restored; save to apply.', 'Padrões restaurados; salve para aplicar.')); }}>{tr('Restore defaults', 'Restaurar padrões')}</Button>
				<div className="flex flex-wrap items-center gap-3">
					<p role="status" aria-live="polite" className="text-sm text-muted-foreground empty:hidden">{message}</p>
					<Button type="submit">{tr('Save timer settings', 'Salvar ajustes do timer')}</Button>
				</div>
			</div>
		</form>
	);
}
