<script lang="ts">
 import { onDestroy } from 'svelte';
 import { formatDuration } from './player';
 import { transport, type Pomodoro } from './transport';
 let pomodoroState: Pomodoro = $state({ status: 'idle', durationSeconds: 1500, elapsedSeconds: 0, updatedAt: 0 });
 let busy = $state(false); let error = $state(''); let timer: ReturnType<typeof setInterval> | undefined;
 let remaining = $derived(Math.max(0, pomodoroState.durationSeconds - pomodoroState.elapsedSeconds));
 async function load() { try { pomodoroState = await transport.pomodoro(); } catch (cause) { error = cause instanceof Error ? cause.message : 'Pomodoro indisponível.'; } }
 async function start() { busy = true; error = ''; try { pomodoroState = await transport.startPomodoro(pomodoroState.durationSeconds); startTicker(); } catch (cause) { error = cause instanceof Error ? cause.message : 'Não foi possível iniciar.'; } finally { busy = false; } }
 async function pause() { busy = true; error = ''; try { pomodoroState = await transport.pausePomodoro(pomodoroState.elapsedSeconds); stopTicker(); } catch (cause) { error = cause instanceof Error ? cause.message : 'Não foi possível pausar.'; } finally { busy = false; } }
 async function reset() { busy = true; error = ''; try { pomodoroState = await transport.resetPomodoro(); stopTicker(); } catch (cause) { error = cause instanceof Error ? cause.message : 'Não foi possível reiniciar.'; } finally { busy = false; } }
 function startTicker() { stopTicker(); timer = setInterval(() => { if (pomodoroState.status === 'running' && pomodoroState.elapsedSeconds < pomodoroState.durationSeconds) pomodoroState = { ...pomodoroState, elapsedSeconds: pomodoroState.elapsedSeconds + 1 }; }, 1000); }
 function stopTicker() { if (timer) clearInterval(timer); timer = undefined; }
 load(); onDestroy(stopTicker);
</script>
<section class="pomodoro" aria-labelledby="pomodoro-heading"><div><h2 id="pomodoro-heading">Pomodoro</h2><p>{pomodoroState.status === 'running' ? 'Sessão em andamento' : pomodoroState.status === 'paused' ? 'Sessão pausada' : 'Pronto para estudar'}</p></div><strong aria-live="polite">{formatDuration(remaining)}</strong><div class="pomodoro-actions">{#if pomodoroState.status === 'running'}<button type="button" class="text-button" onclick={pause} disabled={busy}>Pausar</button>{:else}<button type="button" class="primary" onclick={start} disabled={busy}>{pomodoroState.status === 'paused' ? 'Retomar' : 'Iniciar'}</button>{/if}{#if pomodoroState.status !== 'idle'}<button type="button" class="text-button" onclick={reset} disabled={busy}>Reiniciar</button>{/if}</div>{#if error}<p class="error" role="alert">{error}</p>{/if}</section>
