import { useState, type FormEvent } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useStore } from '@/hooks/use-store';
import { tr } from '@/lib/i18n';
import { DEFAULT_POMODORO_SETTINGS, playPomodoroAlarm, type AlarmSound, type PomodoroSettings } from '@/lib/pomodoro';
import { pomodoro, saveSettings } from '@/lib/pomodoro-store';

const numberFields: { key: 'focusMinutes' | 'shortMinutes' | 'longMinutes' | 'longEvery' | 'alarmRepeats'; label: () => string; min: number; max: number }[] = [
	{ key: 'focusMinutes', label: () => tr('Focus (minutes)', 'Foco (minutos)'), min: 1, max: 120 },
	{ key: 'shortMinutes', label: () => tr('Short break (minutes)', 'Pausa curta (minutos)'), min: 1, max: 60 },
	{ key: 'longMinutes', label: () => tr('Long break (minutes)', 'Pausa longa (minutos)'), min: 1, max: 120 },
	{ key: 'longEvery', label: () => tr('Long break every (blocks)', 'Pausa longa a cada (blocos)'), min: 2, max: 12 },
	{ key: 'alarmRepeats', label: () => tr('Alarm repeats', 'Repetições do alarme'), min: 1, max: 5 }
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
		<form onSubmit={submit} className="space-y-4">
			<fieldset className="space-y-2">
				<legend className="text-sm font-medium">{tr('Preset', 'Predefinição')}</legend>
				<div className="flex flex-wrap gap-2">
					{PRESETS.map((preset) => {
						const active = draft.focusMinutes === preset.focusMinutes && draft.shortMinutes === preset.shortMinutes && draft.longMinutes === preset.longMinutes;
						return (
							<Button key={preset.name()} type="button" variant={active ? 'default' : 'outline'} aria-pressed={active} onClick={() => setDraft({ ...draft, focusMinutes: preset.focusMinutes, shortMinutes: preset.shortMinutes, longMinutes: preset.longMinutes })}>
								{preset.name()} · {tr(`${preset.focusMinutes} min focus, ${preset.shortMinutes} break`, `${preset.focusMinutes} min foco, ${preset.shortMinutes} pausa`)}
							</Button>
						);
					})}
				</div>
			</fieldset>
			<div className="grid max-w-2xl gap-4 sm:grid-cols-2">
				{numberFields.map((field) => (
					<div key={field.key} className="space-y-2">
						<Label htmlFor={`timer-${field.key}`}>{field.label()}</Label>
						<Input id={`timer-${field.key}`} type="number" inputMode="numeric" min={field.min} max={field.max} required value={draft[field.key]} onChange={(event) => set(field.key, Number(event.target.value))} />
					</div>
				))}
				<div className="space-y-2">
					<Label htmlFor="timer-sound">{tr('Alarm sound', 'Som do alarme')}</Label>
					<select id="timer-sound" value={draft.alarmSound} onChange={(event) => set('alarmSound', event.target.value as AlarmSound)} className="h-11 w-full rounded-lg border border-input bg-background px-3 text-sm">
						<option value="sine">{tr('Soft', 'Suave')}</option>
						<option value="triangle">{tr('Clear', 'Claro')}</option>
						<option value="square">{tr('Bold', 'Marcante')}</option>
					</select>
				</div>
				<div className="space-y-2">
					<Label htmlFor="timer-volume">{tr('Alarm volume', 'Volume do alarme')}</Label>
					<input id="timer-volume" type="range" min={0} max={1} step={0.05} value={draft.alarmVolume} onChange={(event) => set('alarmVolume', Number(event.target.value))} className="h-11 w-full accent-[var(--primary)]" />
				</div>
			</div>
			{([['autoStartNext', tr('Start the next block automatically', 'Iniciar o próximo bloco automaticamente')], ['pauseVideoOnRoundEnd', tr('Pause the lesson when a focus block ends', 'Pausar a aula quando um bloco de foco termina')], ['alarmEnabled', tr('Play the alarm when a block ends', 'Tocar o alarme ao fim do bloco')], ['notificationsEnabled', tr('Show a system notification', 'Mostrar notificação do sistema')]] as const).map(([key, label]) => (
				<label key={key} className="flex min-h-11 items-center gap-3 text-sm">
					<input type="checkbox" className="size-5 accent-[var(--primary)]" checked={draft[key]} onChange={(event) => set(key, event.target.checked)} />
					{label}
				</label>
			))}
			<div className="flex flex-wrap gap-2">
				<Button type="submit">{tr('Save timer settings', 'Salvar ajustes do timer')}</Button>
				<Button type="button" variant="outline" onClick={() => playPomodoroAlarm(draft.alarmSound, draft.alarmVolume, 1)}>{tr('Test alarm', 'Testar alarme')}</Button>
				<Button type="button" variant="outline" onClick={() => void askNotifications()}>{tr('Allow notifications', 'Permitir notificações')}</Button>
				<Button type="button" variant="ghost" onClick={() => { setDraft({ ...DEFAULT_POMODORO_SETTINGS }); setMessage(tr('Defaults restored; save to apply.', 'Padrões restaurados; salve para aplicar.')); }}>{tr('Restore defaults', 'Restaurar padrões')}</Button>
			</div>
			<p role="status" aria-live="polite" className="text-sm text-muted-foreground empty:hidden">{message}</p>
		</form>
	);
}
