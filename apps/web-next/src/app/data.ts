import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { resolveSession } from '@/lib/session';
import { studySyncSupported } from '@/lib/player';
import { STUDY_NOT_AVAILABLE, transport, transportKind, type StudyFavorites, type StudyProgress } from '@/lib/transport';

/** Query keys shared by every screen; invalidating one refreshes all its readers. */
export const keys = {
	session: ['session'] as const,
	catalog: ['catalog'] as const,
	recent: ['recent'] as const,
	favorites: ['favorites'] as const,
	rewards: ['rewards'] as const
};

export function useSession() {
	return useQuery({ queryKey: keys.session, queryFn: () => resolveSession(() => transport.me(), transportKind), staleTime: Infinity, retry: false });
}

export function useCatalog(enabled = true) {
	return useQuery({ queryKey: keys.catalog, queryFn: () => transport.catalog(), enabled });
}

// The history covers the whole user: 100 items find the last lesson of any course.
const RECENT_LIMIT = 100;

export function useRecent(enabled = true) {
	return useQuery({
		queryKey: keys.recent,
		enabled: enabled && studySyncSupported,
		queryFn: async (): Promise<StudyProgress[]> => {
			try {
				return (await transport.recentStudy(RECENT_LIMIT)).items;
			} catch (cause) {
				// Study sync can be off on the server; the library still works without it.
				if ((cause as { code?: string } | null)?.code === STUDY_NOT_AVAILABLE) return [];
				throw cause;
			}
		}
	});
}

export function useFavorites(enabled = true) {
	return useQuery({
		queryKey: keys.favorites,
		enabled: enabled && studySyncSupported,
		queryFn: async (): Promise<StudyFavorites> => {
			try {
				return await transport.listFavorites();
			} catch {
				return { courseIds: [], mediaIds: [] };
			}
		}
	});
}

/** Streak, weekly goal and missions in one call (the study summary is part of it). */
export function useRewards(enabled = true) {
	return useQuery({ queryKey: keys.rewards, enabled: enabled && studySyncSupported, queryFn: () => transport.rewardSummary() });
}

export function useToggleCourseFavorite() {
	const client = useQueryClient();
	return useMutation({
		mutationFn: ({ courseId, favorite }: { courseId: string; favorite: boolean }) => transport.setCourseFavorite(courseId, favorite),
		onSuccess: () => client.invalidateQueries({ queryKey: keys.favorites })
	});
}

/** Reindexes the library and refreshes everything that depends on the catalog. */
export function useRescan() {
	const client = useQueryClient();
	return useMutation({
		mutationFn: () => transport.rescan(),
		onSuccess: () => Promise.all([client.invalidateQueries({ queryKey: keys.catalog }), client.invalidateQueries({ queryKey: keys.recent })])
	});
}

/** Desktop opens the native folder picker; the server registers the path an admin typed. */
export function useImportCourse() {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (path?: string) => {
			if (transportKind === 'desktop') await transport.importCourse();
			else await transport.importCoursePath(path?.trim() ?? '');
			await transport.rescan();
		},
		onSuccess: () => Promise.all([client.invalidateQueries({ queryKey: keys.catalog }), client.invalidateQueries({ queryKey: keys.recent })])
	});
}

export function useSignIn() {
	const client = useQueryClient();
	return useMutation({
		mutationFn: ({ username, password }: { username: string; password: string }) => transport.login(username, password),
		onSuccess: (user) => client.setQueryData(keys.session, { user, error: '' })
	});
}

/** Signs out and drops every cached query so the next user starts clean (docs/design/security.md). */
export function useSignOut() {
	const client = useQueryClient();
	return useMutation({
		mutationFn: () => transport.logout(),
		onSettled: () => {
			client.clear();
			client.setQueryData(keys.session, { user: null, error: '' });
		}
	});
}
