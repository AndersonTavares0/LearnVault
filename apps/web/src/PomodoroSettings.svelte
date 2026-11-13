<script lang="ts">
	import { tr } from './i18n.svelte';
	import { DEFAULT_POMODORO_SETTINGS, type PomodoroSettings } from './pomodoro';
	import { pomodoro, saveSettings } from './pomodoro-store';

	let draft = $state<PomodoroSettings>({ ...$pomodoro.settings });
	let message = $state('');

	$effect(() => {
		draft = { ...$pomodoro.settings };
	});

	function submit(event: SubmitEvent) {
		event.preventDefault();
		saveSettings({ ...draft });
		message = tr('Timer settings saved.', 'Ajustes do timer salvos.');
	}

	function discard() {
		draft = { ...$pomodoro.settings };
		message = '';
	}

	async function enableNotifications() {
		if (typeof Notification === 'undefined') {
			message = tr('This browser does not support notifications.', 'Este navegador não oferece notificações.');
			return;
		}
		try {
			const permission = await Notification.requestPermission();
			message =
				permission === 'granted'
					? tr('Notifications enabled.', 'Notificações permitidas.')
					: tr('Notifications were not allowed.', 'Notificações não foram permitidas.');
		} catch {
			message = tr('Could not request notification permission.', 'Não foi possível pedir permissão para notificações.');
		}
	}

	function testAlarm() {
		if (!draft.alarmEnabled) {
			message = tr('Enable the alarm to hear the test.', 'Ative o alarme para ouvir o teste.');
			return;
		}
		try {
			const context = new AudioContext();
			const now = context.currentTime;
			for (let index = 0; index < draft.alarmRepeats; index += 1) {
				const start = now + index * 0.42;
				const oscillator = context.createOscillator();
				const gain = context.createGain();
				oscillator.type = draft.alarmSound;
				oscillator.frequency.setValueAtTime(660 + index * 70, start);
				gain.gain.setValueAtTime(0, start);
				gain.gain.linearRampToValueAtTime(draft.alarmVolume * 0.18, start + 0.015);
				gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.22);
				oscillator.connect(gain).connect(context.destination);
				oscillator.start(start);
				oscillator.stop(start + 0.25);
			}
			message = tr('Test alarm played.', 'Alarme de teste reproduzido.');
		} catch {
			message = tr('Could not play the alarm on this device.', 'Não foi possível reproduzir o alarme neste dispositivo.');
		}
	}

	function restoreDefaults() {
		draft = { ...DEFAULT_POMODORO_SETTINGS };
		message = tr('Defaults restored; save to apply.', 'Padrões restaurados; salve para aplicar.');
	}
</script>

<form class="lv-pomodoro__settings" onsubmit={submit}>
	<div class="lv-pomodoro__settings-grid">
		<label>{tr('Focus (min)', 'Foco (min)')}<input class="lvui-input" type="number" min="1" max="120" bind:value={draft.focusMinutes} /></label>
		<label>{tr('Short break (min)', 'Pausa curta (min)')}<input class="lvui-input" type="number" min="1" max="60" bind:value={draft.shortMinutes} /></label>
		<label>{tr('Long break (min)', 'Pausa longa (min)')}<input class="lvui-input" type="number" min="1" max="120" bind:value={draft.longMinutes} /></label>
		<label>{tr('Long break every (blocks)', 'Pausa longa a cada (blocos)')}<input class="lvui-input" type="number" min="2" max="12" bind:value={draft.longEvery} /></label>
		<label>{tr('Alarm sound', 'Som do alarme')}
			<select class="lvui-select" bind:value={draft.alarmSound}>
				<option value="sine">{tr('Soft', 'Suave')}</option>
				<option value="triangle">{tr('Clear', 'Claro')}</option>
				<option value="square">{tr('Bold', 'Marcante')}</option>
			</select>
		</label>
		<label>{tr('Volume', 'Volume')}<input class="lvui-range" type="range" min="0" max="1" step="0.05" bind:value={draft.alarmVolume} /></label>
		<label>{tr('Alarm repeats', 'Repetições do alarme')}<input class="lvui-input" type="number" min="1" max="5" bind:value={draft.alarmRepeats} /></label>
	</div>
	<div class="lv-pomodoro__checks">
		<label class="lv-pomodoro__check"><input class="lvui-checkbox" type="checkbox" bind:checked={draft.autoStartNext} />{tr('Start the next block automatically', 'Iniciar o próximo bloco automaticamente')}</label>
		<label class="lv-pomodoro__check"><input class="lvui-checkbox" type="checkbox" bind:checked={draft.alarmEnabled} />{tr('Play alarm when finished', 'Tocar alarme ao terminar')}</label>
		<label class="lv-pomodoro__check"><input class="lvui-checkbox" type="checkbox" bind:checked={draft.notificationsEnabled} />{tr('System notifications', 'Notificações do sistema')}</label>
	</div>
	<div class="lv-pomodoro__settings-actions">
		<button class="lvui-btn lvui-btn-primary" type="submit">{tr('Save settings', 'Salvar ajustes')}</button>
		<button class="lvui-btn" type="button" onclick={testAlarm}>{tr('Test alarm', 'Testar alarme')}</button>
		<button class="lvui-btn" type="button" onclick={enableNotifications}>{tr('Enable notifications', 'Permitir notificações')}</button>
		<button class="lvui-btn lvui-btn-ghost" type="button" onclick={restoreDefaults}>{tr('Restore defaults', 'Restaurar padrões')}</button>
		<button class="lvui-btn lvui-btn-ghost" type="button" onclick={discard}>{tr('Discard', 'Descartar')}</button>
	</div>
	{#if message}<p class="lv-form-notice" role="status">{message}</p>{/if}
</form>
