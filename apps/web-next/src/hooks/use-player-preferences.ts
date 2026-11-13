import { useQuery } from '@tanstack/react-query';
import { useCallback, useMemo, useState } from 'react';
import { safeStorage } from '@/app/preferences';
import { clampSpeed } from '@/lib/player';
import {
	normalizeSubtitleLanguage,
	PLAYBACK_PREFERENCES_KEY,
	playerPreferencesScope,
	readPlayerPreferences,
	writePlayerPreferences,
	type PlayerPreferences
} from '@/lib/player-preferences';
import { transport, transportKind } from '@/lib/transport';

/**
 * Speed, subtitle language, auto-advance and per-lesson media/subtitle choices.
 *
 * The server keeps the user's speed and subtitle language across devices; the browser copy
 * is the immediate source and the offline fallback. The desktop build has no account, so it
 * keeps everything in its webview storage.
 */
export function usePlayerPreferences(userId: string | null | undefined) {
	const scope = playerPreferencesScope(transportKind, userId);
	const [stored, setStored] = useState(() => readPlayerPreferences(safeStorage(), scope));
	// Once the learner changes something here, the server copy must not overwrite it.
	const [changed, setChanged] = useState(false);
	const remote = useQuery({
		queryKey: PLAYBACK_PREFERENCES_KEY,
		enabled: Boolean(transport.getPlaybackPreferences),
		queryFn: () => transport.getPlaybackPreferences!(),
		staleTime: Infinity,
		retry: false
	});

	const preferences = useMemo<PlayerPreferences>(() => {
		const saved = remote.data;
		if (changed || !saved) return stored;
		return {
			...stored,
			videoSpeed: clampSpeed(saved.videoSpeed),
			audioSpeed: clampSpeed(saved.audioSpeed),
			subtitleLanguage: normalizeSubtitleLanguage(saved.subtitleLanguage)
		};
	}, [stored, changed, remote.data]);

	const save = useCallback(
		(next: PlayerPreferences) => {
			setChanged(true);
			setStored(next);
			writePlayerPreferences(safeStorage(), scope, next);
			// A failed write keeps the local value; the next change tries again.
			void transport.savePlaybackPreferences?.({
				videoSpeed: next.videoSpeed,
				audioSpeed: next.audioSpeed,
				subtitleLanguage: next.subtitleLanguage
			}).catch(() => undefined);
		},
		[scope]
	);
	return [preferences, save] as const;
}
