import { get, writable } from 'svelte/store';
import {
	DEFAULT_POMODORO_SETTINGS,
	POMODORO_TIMER_KEY,
	modeSeconds,
	persistPomodoroSettings,
	playPomodoroAlarm,
	readPomodoroSettings,
	type PomodoroMode,
	type PomodoroSettings
} from './pomodoro';
import { FocusSessionConflict, InvalidFocusSession, transport, type FocusSessionInput, type FocusSummary, type RewardSummary } from './transport';
import { persistFocusOutbox, persistRejectedFocusSessions, readFocusOutbox, readRejectedFocusSessions, type RejectedFocusSession } from './focus-outbox';

export type PomodoroStatus = 'idle' | 'running' | 'paused';

type StoredTimer = {
	mode: PomodoroMode;
	status: PomodoroStatus;
	remainingSeconds: number;
	totalSeconds: number;
	cycle: number;
	startedAt: number | null;
	endAt: number | null;
};

export type PomodoroState = StoredTimer & {
	settings: PomodoroSettings;
	lastCompleted: { mode: PomodoroMode; at: number } | null;
};

export type FocusSyncState = { pending: FocusSessionInput[]; rejected: RejectedFocusSession[]; saving: boolean; error: string | null; savedAt: number | null };

function storage(): Storage | null {
	try {
		return window.localStorage;
	} catch {
		return null;
	}
}

function modeTotal(mode: PomodoroMode, settings: PomodoroSettings): number {
	return modeSeconds(mode, settings);
}

function readTimer(settings: PomodoroSettings): StoredTimer {
	const fallback: StoredTimer = {
		mode: 'focus',
		status: 'idle',
		remainingSeconds: modeTotal('focus', settings),
		totalSeconds: modeTotal('focus', settings),
		cycle: 0,
		startedAt: null,
		endAt: null
	};
	try {
		const raw = storage()?.getItem(POMODORO_TIMER_KEY);
		if (!raw) return fallback;
		const value: unknown = JSON.parse(raw);
		if (!value || typeof value !== 'object') return fallback;
		const timer = value as Partial<StoredTimer>;
		const mode = timer.mode === 'short' || timer.mode === 'long' ? timer.mode : 'focus';
		const status = timer.status === 'running' || timer.status === 'paused' ? timer.status : 'idle';
		const total = typeof timer.totalSeconds === 'number' && timer.totalSeconds > 0 ? timer.totalSeconds : modeTotal(mode, settings);
		const remaining = typeof timer.remainingSeconds === 'number' && timer.remainingSeconds >= 0 ? Math.min(timer.remainingSeconds, total) : total;
		return {
			mode,
			status,
			remainingSeconds: remaining,
			totalSeconds: total,
			cycle: typeof timer.cycle === 'number' && timer.cycle >= 0 ? Math.floor(timer.cycle) : 0,
			startedAt: typeof timer.startedAt === 'number' ? timer.startedAt : null,
			endAt: typeof timer.endAt === 'number' ? timer.endAt : null
		};
	} catch {
		return fallback;
	}
}

const initialSettings = readPomodoroSettings(storage());
const stored = readTimer(initialSettings);

export const pomodoro = writable<PomodoroState>({
	...stored,
	settings: initialSettings,
	lastCompleted: null
});
export const focusSync = writable<FocusSyncState>({ pending: [], rejected: [], saving: false, error: null, savedAt: null });
export const persistedFocusSummary = writable<FocusSummary | null>(null);
export const persistedRewardSummary = writable<RewardSummary | null>(null);
let focusOutboxScope: string | null = null;
let focusScopeRevision = 0;
let activeRetry: Promise<void> | null = null;

export function setFocusOutboxScope(scope: string | null): void {
	if (focusOutboxScope === scope) return;
	focusOutboxScope = scope;
	focusScopeRevision += 1;
	const pending = scope ? readFocusOutbox(storage(), scope) : [];
	const rejected = scope ? readRejectedFocusSessions(storage(), scope) : [];
	focusSync.set({ pending, rejected, saving: false, error: null, savedAt: null });
	persistedFocusSummary.set(null);
	persistedRewardSummary.set(null);
	if (pending.length > 0) void retryFocusSessions();
}

export async function refreshFocusData(): Promise<void> {
	const scope = focusOutboxScope;
	const revision = focusScopeRevision;
	const [focus, rewards] = await Promise.allSettled([transport.focusSummary(), transport.rewardSummary()]);
	if (scope !== focusOutboxScope || revision !== focusScopeRevision) return;
	if (focus.status === 'fulfilled') persistedFocusSummary.set(focus.value);
	if (rewards.status === 'fulfilled') persistedRewardSummary.set(rewards.value);
}

let ticker: ReturnType<typeof setInterval> | null = null;

function persist(state: PomodoroState): void {
	try {
		const timer: StoredTimer = {
			mode: state.mode,
			status: state.status,
			remainingSeconds: state.remainingSeconds,
			totalSeconds: state.totalSeconds,
			cycle: state.cycle,
			startedAt: state.startedAt,
			endAt: state.endAt
		};
		storage()?.setItem(POMODORO_TIMER_KEY, JSON.stringify(timer));
	} catch {
		// A timer continua válido em memória quando o armazenamento está bloqueado.
	}
}

function ensureTicker(): void {
	if (ticker !== null) return;
	ticker = setInterval(tick, 250);
}

function stopTickerIfIdle(): void {
	if (ticker === null) return;
	if (get(pomodoro).status !== 'running') {
		clearInterval(ticker);
		ticker = null;
	}
}

function setState(next: PomodoroState, persistState = true): void {
	pomodoro.set(next);
	if (persistState) persist(next);
	stopTickerIfIdle();
}

function nextMode(state: PomodoroState): { mode: PomodoroMode; cycle: number } {
	if (state.mode !== 'focus') return { mode: 'focus', cycle: state.cycle };
	const cycle = state.cycle + 1;
	return { mode: cycle % state.settings.longEvery === 0 ? 'long' : 'short', cycle };
}

function queueFocusSession(session: FocusSessionInput): void {
	focusSync.update((current) => {
		if (current.pending.some((entry) => entry.id === session.id)) return current;
		const pending = [...current.pending, session];
		if (focusOutboxScope) persistFocusOutbox(storage(), focusOutboxScope, pending);
		return { ...current, pending, error: null };
	});
	void retryFocusSessions();
}

export function retryFocusSessions(): Promise<void> {
	if (activeRetry) return activeRetry;
	const scope = focusOutboxScope;
	const revision = focusScopeRevision;
	if (!scope) return Promise.resolve();
	focusSync.update((current) => ({ ...current, saving: true, error: null }));
	const task = (async () => {
		while (scope === focusOutboxScope && revision === focusScopeRevision && get(focusSync).pending.length > 0) {
			const session = get(focusSync).pending[0]!;
			try {
				await transport.recordFocusSession(session);
				if (scope !== focusOutboxScope || revision !== focusScopeRevision) break;
				focusSync.update((current) => {
					const pending = current.pending.filter((entry) => entry.id !== session.id);
					persistFocusOutbox(storage(), scope, pending);
					return { ...current, pending, savedAt: Date.now() };
				});
				void refreshFocusData();
			} catch (cause) {
				if (scope !== focusOutboxScope || revision !== focusScopeRevision) break;
				const current = get(focusSync);
				if (cause instanceof FocusSessionConflict || cause instanceof InvalidFocusSession) {
					const reason = cause.message;
					const rejected = [...current.rejected, { session, reason }];
					const durable = persistRejectedFocusSessions(storage(), scope, rejected);
					if (!durable) {
						focusSync.update((state) => ({ ...state, error: `${reason} Não foi possível preservar a rejeição no armazenamento; a sessão continua pendente nesta tela.` }));
						break;
					}
					const pending = current.pending.filter((entry) => entry.id !== session.id);
					persistFocusOutbox(storage(), scope, pending);
					focusSync.update((state) => ({ ...state, pending, rejected, error: null }));
					continue;
				}
				const durable = persistFocusOutbox(storage(), scope, current.pending);
				focusSync.update((state) => ({ ...state, error: durable ? (cause instanceof Error ? cause.message : 'Não foi possível salvar a sessão de foco.') : 'Não foi possível sincronizar; esta sessão ainda está somente nesta tela.' }));
				break;
			}
		}
		if (scope === focusOutboxScope && revision === focusScopeRevision) {
			focusSync.update((current) => ({ ...current, saving: false }));
		}
	})();
	activeRetry = task.finally(() => {
		activeRetry = null;
		if ((scope !== focusOutboxScope || revision !== focusScopeRevision) && focusOutboxScope && get(focusSync).pending.length > 0) void retryFocusSessions();
	});
	return activeRetry;
}

function createFocusSessionId(): string {
	if (typeof crypto === 'undefined') throw new Error('Não há gerador criptográfico disponível para identificar a sessão de foco.');
	if (typeof crypto.randomUUID === 'function') return crypto.randomUUID();
	if (typeof crypto.getRandomValues !== 'function') throw new Error('Não há gerador criptográfico disponível para identificar a sessão de foco.');
	const bytes = crypto.getRandomValues(new Uint8Array(16));
	bytes[6] = (bytes[6]! & 0x0f) | 0x40;
	bytes[8] = (bytes[8]! & 0x3f) | 0x80;
	const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
	return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function recordSession(state: PomodoroState, completed: boolean): FocusSessionInput | null {
	if (state.mode !== 'focus' || state.startedAt === null) return null;
	const endedAt = Math.max(0, Math.floor(Date.now() / 1000));
	const elapsed = Math.max(0, Math.min(state.totalSeconds, state.totalSeconds - state.remainingSeconds));
	if (!completed && elapsed < 1) return null;
	const actualSeconds = completed ? state.totalSeconds : elapsed;
	const session: FocusSessionInput = {
		id: createFocusSessionId(),
		kind: 'focus',
		plannedSeconds: state.totalSeconds,
		actualSeconds,
		completed,
		startedAt: state.startedAt,
		endedAt
	};
	queueFocusSession(session);
	return session;
}

function complete(): void {
	const state = get(pomodoro);
	if (state.status !== 'running') return;
	recordSession(state, true);

	// Notificação sonora e de navegador (opt-in)
	if (state.settings.alarmEnabled) {
		playPomodoroAlarm(state.settings.alarmSound, state.settings.alarmVolume, state.settings.alarmRepeats);
	}
	if (state.settings.notificationsEnabled && typeof Notification !== 'undefined' && Notification.permission === 'granted') {
		try {
			const title = state.mode === 'focus' ? 'Ciclo de foco concluído! 🎉' : 'Pausa finalizada! Pronto para focar?';
			const body = state.mode === 'focus' ? 'Uma árvore saudável foi cultivada na sua floresta.' : 'Hora de retomar seus estudos.';
			new Notification(title, { body });
		} catch {
			// Ignora falha de notificação do navegador
		}
	}

	const next = nextMode(state);
	const total = modeTotal(next.mode, state.settings);
	const running = state.settings.autoStartNext;
	setState({
		...state,
		mode: next.mode,
		cycle: next.cycle,
		status: running ? 'running' : 'idle',
		remainingSeconds: total,
		totalSeconds: total,
		startedAt: running ? Math.floor(Date.now() / 1000) : null,
		endAt: running ? Date.now() + total * 1000 : null,
		lastCompleted: { mode: state.mode, at: Date.now() }
	});
	if (running) ensureTicker();
}

export function tick(): void {
	const state = get(pomodoro);
	if (state.status !== 'running' || state.endAt === null) return;
	const remaining = Math.max(0, Math.ceil((state.endAt - Date.now()) / 1000));
	if (remaining <= 0) {
		complete();
		return;
	}
	if (remaining !== state.remainingSeconds) {
		setState({ ...state, remainingSeconds: remaining }, false);
	}
}

export function startTimer(): void {
	const state = get(pomodoro);
	if (state.status === 'running') return;
	const remaining = state.remainingSeconds > 0 ? state.remainingSeconds : state.totalSeconds;
	const startedAt = state.startedAt ?? Math.floor(Date.now() / 1000);
	const endAt = Date.now() + remaining * 1000;
	setState({ ...state, status: 'running', remainingSeconds: remaining, startedAt, endAt });
	ensureTicker();
}

export function pauseTimer(): void {
	const state = get(pomodoro);
	if (state.status !== 'running' || state.endAt === null) return;
	const remaining = Math.max(0, Math.ceil((state.endAt - Date.now()) / 1000));
	setState({ ...state, status: 'paused', remainingSeconds: remaining, endAt: null });
}

export function resetTimer(): void {
	const state = get(pomodoro);
	const total = modeTotal(state.mode, state.settings);
	setState({ ...state, status: 'idle', remainingSeconds: total, totalSeconds: total, startedAt: null, endAt: null });
}

/**
 * Abandono explícito da sessão de foco ativa.
 *
 * Apenas esta ação deliberada de desistência gera uma sessão incompleta (árvore seca).
 * Interrupções de rede, fechamento de aba, reinício ou pausas preservam a tentativa.
 */
export function abandonTimer(): void {
	const state = get(pomodoro);
	if (state.mode === 'focus' && state.startedAt !== null) {
		recordSession(state, false);
	}
	const total = modeTotal(state.mode, state.settings);
	setState({ ...state, status: 'idle', remainingSeconds: total, totalSeconds: total, startedAt: null, endAt: null });
}

export function skipTimer(): void {
	const state = get(pomodoro);
	recordSession(state, false);
	const next = nextMode(state);
	const total = modeTotal(next.mode, state.settings);
	setState({ ...state, mode: next.mode, cycle: next.cycle, status: 'idle', remainingSeconds: total, totalSeconds: total, startedAt: null, endAt: null });
}

export function setMode(mode: PomodoroMode): void {
	const state = get(pomodoro);
	if (state.status === 'running') recordSession(state, false);
	const total = modeTotal(mode, state.settings);
	setState({ ...state, mode, status: 'idle', remainingSeconds: total, totalSeconds: total, startedAt: null, endAt: null });
}

export function saveSettings(next: PomodoroSettings): void {
	persistPomodoroSettings(storage(), next);
	const state = get(pomodoro);
	const total = modeTotal(state.mode, next);
	const running = state.status === 'running' && state.endAt !== null;
	const remaining = running ? Math.min(state.remainingSeconds, total) : total;
	setState({
		...state,
		settings: next,
		status: running ? 'running' : 'idle',
		remainingSeconds: remaining,
		totalSeconds: total,
		startedAt: running ? state.startedAt : null,
		endAt: running ? Date.now() + remaining * 1000 : null
	});
}

if (stored.status === 'running') {
	ensureTicker();
	tick();
}

export { DEFAULT_POMODORO_SETTINGS };
