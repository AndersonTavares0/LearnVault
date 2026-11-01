export const POMODORO_SETTINGS_KEY = 'learnvault-pomodoro-settings';
export const FOCUS_SESSIONS_KEY = 'learnvault-focus-sessions';
export const POMODORO_TIMER_KEY = 'learnvault-pomodoro-timer';

export const POMODORO_MODES = ['focus', 'short', 'long'] as const;
export type PomodoroMode = (typeof POMODORO_MODES)[number];
export type AlarmSound = 'sine' | 'triangle' | 'square';

export type PomodoroSettings = {
	focusMinutes: number;
	shortMinutes: number;
	longMinutes: number;
	longEvery: number;
	autoStartNext: boolean;
	alarmEnabled: boolean;
	alarmSound: AlarmSound;
	alarmVolume: number;
	alarmRepeats: number;
	notificationsEnabled: boolean;
};

export const DEFAULT_POMODORO_SETTINGS: PomodoroSettings = {
	focusMinutes: 25,
	shortMinutes: 5,
	longMinutes: 15,
	longEvery: 4,
	autoStartNext: false,
	alarmEnabled: true,
	alarmSound: 'sine',
	alarmVolume: 0.5,
	alarmRepeats: 2,
	notificationsEnabled: true
};

export type FocusSession = {
	id: string;
	kind: PomodoroMode;
	plannedSeconds: number;
	actualSeconds: number;
	completed: boolean;
	startedAt: number;
	endedAt: number;
};

export type FocusDay = {
	day: string;
	minutes: number;
	sessions: number;
	completed: number;
};

export type FocusSummary = {
	todayMinutes: number;
	weekMinutes: number;
	totalMinutes: number;
	weekSessions: number;
	totalSessions: number;
	completionRate: number;
	bestDay: FocusDay | null;
	days: FocusDay[];
};

function finiteInteger(value: unknown, min: number, max: number, fallback: number): number {
	const parsed = typeof value === 'number' ? value : Number(value);
	if (!Number.isInteger(parsed) || parsed < min || parsed > max) return fallback;
	return parsed;
}

function finiteNumber(value: unknown, min: number, max: number, fallback: number): number {
	const parsed = typeof value === 'number' ? value : Number(value);
	if (!Number.isFinite(parsed)) return fallback;
	return Math.min(max, Math.max(min, parsed));
}

function alarmSound(value: unknown): AlarmSound {
	return value === 'triangle' || value === 'square' || value === 'sine' ? value : DEFAULT_POMODORO_SETTINGS.alarmSound;
}

export function parsePomodoroSettings(value: unknown): PomodoroSettings {
	if (!value || typeof value !== 'object') return { ...DEFAULT_POMODORO_SETTINGS };
	const source = value as Partial<Record<keyof PomodoroSettings, unknown>>;
	return {
		focusMinutes: finiteInteger(source.focusMinutes, 1, 120, DEFAULT_POMODORO_SETTINGS.focusMinutes),
		shortMinutes: finiteInteger(source.shortMinutes, 1, 60, DEFAULT_POMODORO_SETTINGS.shortMinutes),
		longMinutes: finiteInteger(source.longMinutes, 1, 120, DEFAULT_POMODORO_SETTINGS.longMinutes),
		longEvery: finiteInteger(source.longEvery, 2, 12, DEFAULT_POMODORO_SETTINGS.longEvery),
		autoStartNext: source.autoStartNext === true,
		alarmEnabled: source.alarmEnabled !== false,
		alarmSound: alarmSound(source.alarmSound),
		alarmVolume: finiteNumber(source.alarmVolume, 0, 1, DEFAULT_POMODORO_SETTINGS.alarmVolume),
		alarmRepeats: finiteInteger(source.alarmRepeats, 1, 5, DEFAULT_POMODORO_SETTINGS.alarmRepeats),
		notificationsEnabled: source.notificationsEnabled !== false
	};
}

export function readPomodoroSettings(storage: Pick<Storage, 'getItem'> | null | undefined): PomodoroSettings {
	try {
		const raw = storage?.getItem(POMODORO_SETTINGS_KEY);
		return raw ? parsePomodoroSettings(JSON.parse(raw)) : { ...DEFAULT_POMODORO_SETTINGS };
	} catch {
		return { ...DEFAULT_POMODORO_SETTINGS };
	}
}

export function persistPomodoroSettings(storage: Pick<Storage, 'setItem'> | null | undefined, settings: PomodoroSettings): boolean {
	if (!storage) return false;
	try {
		storage.setItem(POMODORO_SETTINGS_KEY, JSON.stringify(settings));
		return true;
	} catch {
		return false;
	}
}

export function modeSeconds(mode: PomodoroMode, settings: PomodoroSettings): number {
	if (mode === 'short') return settings.shortMinutes * 60;
	if (mode === 'long') return settings.longMinutes * 60;
	return settings.focusMinutes * 60;
}

export function playPomodoroAlarm(sound: AlarmSound, volume: number, repeats: number): void {
	if (typeof window === 'undefined' || typeof AudioContext === 'undefined') return;
	try {
		const context = new AudioContext();
		const now = context.currentTime;
		for (let index = 0; index < repeats; index += 1) {
			const start = now + index * 0.42;
			const oscillator = context.createOscillator();
			const gain = context.createGain();
			oscillator.type = sound;
			oscillator.frequency.setValueAtTime(660 + index * 70, start);
			gain.gain.setValueAtTime(0, start);
			gain.gain.linearRampToValueAtTime(volume * 0.18, start + 0.015);
			gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.22);
			oscillator.connect(gain).connect(context.destination);
			oscillator.start(start);
			oscillator.stop(start + 0.25);
		}
	} catch {
		// Falha de áudio segura
	}
}

export function modeLabel(mode: PomodoroMode): string {
	if (mode === 'short') return 'Pausa curta';
	if (mode === 'long') return 'Pausa longa';
	return 'Foco';
}

export function formatClock(totalSeconds: number): string {
	const seconds = Math.max(0, Math.round(totalSeconds));
	return `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
}

export function localDayKey(date: Date): string {
	const year = date.getFullYear();
	const month = String(date.getMonth() + 1).padStart(2, '0');
	const day = String(date.getDate()).padStart(2, '0');
	return `${year}-${month}-${day}`;
}

export function weekDayKeys(now = new Date()): string[] {
	const start = new Date(now.getFullYear(), now.getMonth(), now.getDate());
	const sinceMonday = (start.getDay() + 6) % 7;
	start.setDate(start.getDate() - sinceMonday);
	return Array.from({ length: 7 }, (_, index) => {
		const day = new Date(start);
		day.setDate(start.getDate() + index);
		return localDayKey(day);
	});
}

export function readFocusSessions(storage: Pick<Storage, 'getItem'> | null | undefined): FocusSession[] {
	try {
		const raw = storage?.getItem(FOCUS_SESSIONS_KEY);
		if (!raw) return [];
		const parsed: unknown = JSON.parse(raw);
		if (!Array.isArray(parsed)) return [];
		return parsed.filter((entry): entry is FocusSession => {
			if (!entry || typeof entry !== 'object') return false;
			const session = entry as Partial<FocusSession>;
			return (
				typeof session.id === 'string' &&
				(session.kind === 'focus' || session.kind === 'short' || session.kind === 'long') &&
				typeof session.plannedSeconds === 'number' &&
				typeof session.actualSeconds === 'number' &&
				typeof session.completed === 'boolean' &&
				typeof session.startedAt === 'number' &&
				typeof session.endedAt === 'number'
			);
		});
	} catch {
		return [];
	}
}

export function persistFocusSessions(storage: Pick<Storage, 'setItem'> | null | undefined, sessions: FocusSession[]): boolean {
	if (!storage) return false;
	try {
		storage.setItem(FOCUS_SESSIONS_KEY, JSON.stringify(sessions.slice(-500)));
		return true;
	} catch {
		return false;
	}
}

export function recordFocusSession(
	storage: Pick<Storage, 'getItem' | 'setItem'> | null | undefined,
	session: FocusSession
): boolean {
	const sessions = [...readFocusSessions(storage), session].slice(-500);
	return persistFocusSessions(storage, sessions);
}

export function buildFocusSummary(sessions: FocusSession[], now = new Date()): FocusSummary {
	const week = weekDayKeys(now);
	const weekSet = new Set(week);
	const byDay = new Map<string, FocusDay>(week.map((day) => [day, { day, minutes: 0, sessions: 0, completed: 0 }]));
	const today = localDayKey(now);
	for (const session of sessions) {
		const day = localDayKey(new Date(session.endedAt * 1000));
		const entry = byDay.get(day);
		if (!entry || !weekSet.has(day)) continue;
		entry.sessions += 1;
		entry.minutes += Math.round(session.actualSeconds / 60);
		if (session.completed) entry.completed += 1;
	}
	const days = week.map((day) => byDay.get(day)!);
	const focusSessions = sessions.filter((session) => session.kind === 'focus');
	const completedFocus = focusSessions.filter((session) => session.completed).length;
	const bestDay = [...days].sort((a, b) => b.minutes - a.minutes || a.day.localeCompare(b.day))[0] ?? null;
	return {
		todayMinutes: byDay.get(today)?.minutes ?? 0,
		weekMinutes: days.reduce((total, entry) => total + entry.minutes, 0),
		totalMinutes: Math.round(focusSessions.reduce((total, session) => total + session.actualSeconds, 0) / 60),
		weekSessions: days.reduce((total, entry) => total + entry.sessions, 0),
		totalSessions: completedFocus,
		completionRate: focusSessions.length > 0 ? Math.round((completedFocus / focusSessions.length) * 100) : 0,
		bestDay: bestDay && bestDay.minutes > 0 ? bestDay : null,
		days
	};
}
