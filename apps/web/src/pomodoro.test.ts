import { describe, expect, it } from 'vitest';
import {
	DEFAULT_POMODORO_SETTINGS,
	FOCUS_SESSIONS_KEY,
	POMODORO_SETTINGS_KEY,
	buildFocusSummary,
	localDayKey,
	modeLabel,
	modeSeconds,
	parsePomodoroSettings,
	persistFocusSessions,
	persistPomodoroSettings,
	readFocusSessions,
	readPomodoroSettings,
	recordFocusSession,
	weekDayKeys,
	type FocusSession
} from './pomodoro';

function storage(initial: Record<string, string> = {}) {
	const values = new Map(Object.entries(initial));
	return {
		getItem: (key: string) => values.get(key) ?? null,
		setItem: (key: string, value: string) => void values.set(key, value)
	};
}

function session(startedAt: Date, actualSeconds = 1500, completed = true): FocusSession {
	return {
		id: `s-${startedAt.toISOString()}-${actualSeconds}`,
		kind: 'focus',
		plannedSeconds: 1500,
		actualSeconds,
		completed,
		startedAt: Math.floor(startedAt.getTime() / 1000),
		endedAt: Math.floor(startedAt.getTime() / 1000) + actualSeconds
	};
}

describe('parsePomodoroSettings', () => {
	it('returns the defaults for missing or malformed values', () => {
		expect(parsePomodoroSettings(null)).toEqual(DEFAULT_POMODORO_SETTINGS);
		expect(parsePomodoroSettings({ focusMinutes: 0, shortMinutes: 999, longMinutes: -1 })).toMatchObject({
			focusMinutes: DEFAULT_POMODORO_SETTINGS.focusMinutes,
			shortMinutes: DEFAULT_POMODORO_SETTINGS.shortMinutes,
			longMinutes: DEFAULT_POMODORO_SETTINGS.longMinutes
		});
	});

	it('keeps valid values and clamps the volume', () => {
		const parsed = parsePomodoroSettings({
			focusMinutes: 50,
			shortMinutes: 10,
			longMinutes: 30,
			longEvery: 6,
			autoStartNext: true,
			alarmEnabled: false,
			alarmSound: 'square',
			alarmVolume: 2,
			alarmRepeats: 4,
			notificationsEnabled: false
		});
		expect(parsed).toMatchObject({
			focusMinutes: 50,
			shortMinutes: 10,
			longMinutes: 30,
			longEvery: 6,
			autoStartNext: true,
			alarmEnabled: false,
			alarmSound: 'square',
			alarmVolume: 1,
			alarmRepeats: 4,
			notificationsEnabled: false
		});
	});
});

describe('pomodoro modes', () => {
	it('returns the configured duration in seconds', () => {
		expect(modeSeconds('focus', DEFAULT_POMODORO_SETTINGS)).toBe(25 * 60);
		expect(modeSeconds('short', DEFAULT_POMODORO_SETTINGS)).toBe(5 * 60);
		expect(modeSeconds('long', DEFAULT_POMODORO_SETTINGS)).toBe(15 * 60);
	});

	it('labels each mode in pt-BR', () => {
		expect(modeLabel('focus')).toBe('Foco');
		expect(modeLabel('short')).toBe('Pausa curta');
		expect(modeLabel('long')).toBe('Pausa longa');
	});
});

describe('week helpers', () => {
	it('starts the week on Monday', () => {
		const monday = new Date(2026, 9, 26, 12);
		expect(weekDayKeys(monday)).toEqual([
			'2026-10-26',
			'2026-10-27',
			'2026-10-28',
			'2026-10-29',
			'2026-10-30',
			'2026-10-31',
			'2026-11-01'
		]);
		expect(localDayKey(monday)).toBe('2026-10-26');
	});
});

describe('buildFocusSummary', () => {
	it('summarizes only the current week and keeps all-time totals', () => {
		const now = new Date(2026, 9, 28, 18);
		const sessions = [
			session(new Date(2026, 9, 26, 9), 1500),
			session(new Date(2026, 9, 26, 14), 600),
			session(new Date(2026, 9, 28, 10), 1200),
			session(new Date(2026, 9, 20, 10), 1500)
		];
		const summary = buildFocusSummary(sessions, now);
		expect(summary.weekMinutes).toBe(55);
		expect(summary.weekSessions).toBe(3);
		expect(summary.totalSessions).toBe(4);
		expect(summary.todayMinutes).toBe(20);
		expect(summary.completionRate).toBe(100);
		expect(summary.bestDay?.day).toBe('2026-10-26');
		expect(summary.bestDay?.minutes).toBe(35);
	});

	it('returns zeroed values without focus sessions', () => {
		const summary = buildFocusSummary([], new Date(2026, 9, 28, 18));
		expect(summary.weekMinutes).toBe(0);
		expect(summary.weekSessions).toBe(0);
		expect(summary.totalSessions).toBe(0);
		expect(summary.bestDay).toBeNull();
	});
});

describe('focus session persistence', () => {
	it('round-trips sessions and records new ones', () => {
		const store = storage();
		const first = session(new Date(2026, 9, 26, 9));
		expect(recordFocusSession(store, first)).toBe(true);
		const second = session(new Date(2026, 9, 26, 10), 300, false);
		expect(recordFocusSession(store, second)).toBe(true);
		expect(readFocusSessions(store)).toHaveLength(2);
		expect(JSON.parse(store.getItem(FOCUS_SESSIONS_KEY) ?? '[]')).toHaveLength(2);
	});

	it('ignores malformed session entries', () => {
		const store = storage({ [FOCUS_SESSIONS_KEY]: JSON.stringify([{ id: 'x' }, null, { id: 'ok', kind: 'focus', plannedSeconds: 60, actualSeconds: 60, completed: true, startedAt: 1, endedAt: 2 }]) });
		expect(readFocusSessions(store).map((entry) => entry.id)).toEqual(['ok']);
	});

	it('persists settings under the expected key', () => {
		const store = storage();
		expect(persistPomodoroSettings(store, { ...DEFAULT_POMODORO_SETTINGS, focusMinutes: 40 })).toBe(true);
		expect(readPomodoroSettings(store).focusMinutes).toBe(40);
		expect(store.getItem(POMODORO_SETTINGS_KEY)).toContain('"focusMinutes":40');
	});

	it('drops the oldest sessions when the cap is exceeded', () => {
		const store = storage();
		const entries = Array.from({ length: 501 }, (_, index) => session(new Date(2026, 0, 1, 0, 0, index), 60));
		expect(persistFocusSessions(store, entries)).toBe(true);
		expect(readFocusSessions(store)).toHaveLength(500);
	});
});
