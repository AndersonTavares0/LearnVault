import { useCallback, useState } from 'react';
import { playerPreferencesScope, readPlayerPreferences, writePlayerPreferences, type PlayerPreferences } from '@/lib/player-preferences';
import { transportKind } from '@/lib/transport';
import { safeStorage } from '@/app/preferences';

/** Speed, auto-advance and per-lesson media/subtitle choices, stored per user in this browser. */
export function usePlayerPreferences(userId: string | null | undefined) {
	const scope = playerPreferencesScope(transportKind, userId);
	const [preferences, setPreferences] = useState(() => readPlayerPreferences(safeStorage(), scope));
	const save = useCallback(
		(next: PlayerPreferences) => {
			setPreferences(next);
			writePlayerPreferences(safeStorage(), scope, next);
		},
		[scope]
	);
	return [preferences, save] as const;
}
