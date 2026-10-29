<script lang="ts">
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
		message = 'Ajustes do timer salvos.';
	}

	function discard() {
		draft = { ...$pomodoro.settings };
		message = '';
	}

	async function enableNotifications() {
		if (typeof Notification === 'undefined') {
			message = 'Este navegador não oferece notificações.';
			return;
		}
		try {
			const permission = await Notification.requestPermission();
			message = permission === 'granted' ? 'Notificações permitidas.' : 'Notificações não foram permitidas.';
		} catch {
			message = 'Não foi possível pedir permissão para notificações.';
		}
	}

	function testAlarm() {
		if (!draft.alarmEnabled) {
			message = 'Ative o alarme para ouvir o teste.';
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
			message = 'Alarme de teste reproduzido.';
		} catch {
			message = 'Não foi possível reproduzir o alarme neste dispositivo.';
		}
	}

	function restoreDefaults() {
		draft = { ...DEFAULT_POMODORO_SETTINGS };
		message = 'Padrões restaurados; salve para aplicar.';
	}
</script>

<form class="lv-pomodoro__settings" onsubmit={submit}>
	<div class="lv-pomodoro__settings-grid">
		<label>Foco (min)<input class="lvui-input" type="number" min="1" max="120" bind:value={draft.focusMinutes} /></label>
		<label>Pausa curta (min)<input class="lvui-input" type="number" min="1" max="60" bind:value={draft.shortMinutes} /></label>
		<label>Pausa longa (min)<input class="lvui-input" type="number" min="1" max="120" bind:value={draft.longMinutes} /></label>
		<label>Pausa longa a cada (blocos)<input class="lvui-input" type="number" min="2" max="12" bind:value={draft.longEvery} /></label>
		<label>Som do alarme
			<select class="lvui-select" bind:value={draft.alarmSound}>
				<option value="sine">Suave</option>
				<option value="triangle">Claro</option>
				<option value="square">Marcante</option>
			</select>
		</label>
		<label>Volume<input class="lvui-range" type="range" min="0" max="1" step="0.05" bind:value={draft.alarmVolume} /></label>
		<label>Repetições do alarme<input class="lvui-input" type="number" min="1" max="5" bind:value={draft.alarmRepeats} /></label>
	</div>
	<div class="lv-pomodoro__checks">
		<label class="lv-pomodoro__check"><input class="lvui-checkbox" type="checkbox" bind:checked={draft.autoStartNext} />Iniciar o próximo bloco automaticamente</label>
		<label class="lv-pomodoro__check"><input class="lvui-checkbox" type="checkbox" bind:checked={draft.alarmEnabled} />Tocar alarme ao terminar</label>
		<label class="lv-pomodoro__check"><input class="lvui-checkbox" type="checkbox" bind:checked={draft.notificationsEnabled} />Notificações do sistema</label>
	</div>
	<div class="lv-pomodoro__settings-actions">
		<button class="lvui-btn lvui-btn-primary" type="submit">Salvar ajustes</button>
		<button class="lvui-btn" type="button" onclick={testAlarm}>Testar alarme</button>
		<button class="lvui-btn" type="button" onclick={enableNotifications}>Permitir notificações</button>
		<button class="lvui-btn lvui-btn-ghost" type="button" onclick={restoreDefaults}>Restaurar padrões</button>
		<button class="lvui-btn lvui-btn-ghost" type="button" onclick={discard}>Descartar</button>
	</div>
	{#if message}<p class="lv-form-notice" role="status">{message}</p>{/if}
</form>
