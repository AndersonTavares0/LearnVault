import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { resolveSession } from '@/lib/session';
import { studySyncSupported } from '@/lib/player';
import { STUDY_NOT_AVAILABLE, transport, transportKind, type MissionId, type NewPdfAnnotation, type PdfAnnotationColor, type StudyFavorites, type StudyProgress } from '@/lib/transport';

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

/** Marks a lesson done or not and patches the cached history, so progress updates at once. */
export function useSetLessonCompleted() {
	const client = useQueryClient();
	return useMutation({
		mutationFn: ({ mediaId, completed }: { mediaId: string; completed: boolean }) => transport.setCompleted(mediaId, completed),
		onSuccess: (updated) => {
			client.setQueryData<StudyProgress[]>(keys.recent, (items = []) => [updated, ...items.filter((item) => item.mediaId !== updated.mediaId)]);
			void client.invalidateQueries({ queryKey: keys.rewards });
		}
	});
}

export function useToggleLessonFavorite() {
	const client = useQueryClient();
	return useMutation({
		mutationFn: ({ mediaId, favorite }: { mediaId: string; favorite: boolean }) => transport.setLessonFavorite(mediaId, favorite),
		onSuccess: () => client.invalidateQueries({ queryKey: keys.favorites })
	});
}

/** Removes a course from the registry only; files on disk are never touched. */
export function useRemoveCourse() {
	const client = useQueryClient();
	return useMutation({
		mutationFn: (courseId: string) => transport.removeCourse(courseId),
		onSuccess: () => client.invalidateQueries({ queryKey: keys.catalog })
	});
}

export function useResetCourseOrder() {
	const client = useQueryClient();
	return useMutation({
		mutationFn: (courseId: string) => transport.resetCourseOrder(courseId),
		onSuccess: () => client.invalidateQueries({ queryKey: keys.catalog })
	});
}

export function useNotes(enabled = true) {
	return useQuery({ queryKey: ['notes'], enabled: enabled && studySyncSupported, queryFn: () => transport.listNotes() });
}

export function useFocusSummary(enabled = true) {
	return useQuery({ queryKey: ['focus-summary'], enabled: enabled && studySyncSupported, queryFn: () => transport.focusSummary() });
}

export function useClaimMission() {
	const client = useQueryClient();
	return useMutation({ mutationFn: (missionId: MissionId) => transport.claimDailyMission(missionId), onSuccess: () => client.invalidateQueries({ queryKey: keys.rewards }) });
}

export function useSetWeeklyGoal() {
	const client = useQueryClient();
	return useMutation({ mutationFn: (goal: number) => transport.setWeeklyGoal(goal), onSuccess: () => client.invalidateQueries({ queryKey: keys.rewards }) });
}

/** Desktop opens the native image picker in Rust (no file crosses IPC); the server uploads the chosen file. */
export function useCourseCover() {
	const client = useQueryClient();
	const refresh = () => client.invalidateQueries({ queryKey: keys.catalog });
	return {
		change: useMutation({ mutationFn: ({ courseId, file }: { courseId: string; file?: File }) => transport.importCourseCover(courseId, file), onSuccess: refresh }),
		remove: useMutation({ mutationFn: (courseId: string) => transport.removeCourseCover(courseId), onSuccess: refresh })
	};
}

/** The reader's marks on one PDF; they are per owner, so the key includes the media. */
export function usePdfAnnotations(mediaId: string | null) {
	return useQuery({ queryKey: ['pdf-annotations', mediaId], enabled: Boolean(mediaId) && studySyncSupported, queryFn: () => transport.listPdfAnnotations(mediaId!) });
}

export function usePdfAnnotationMutations(mediaId: string | null) {
	const client = useQueryClient();
	const refresh = () => client.invalidateQueries({ queryKey: ['pdf-annotations', mediaId] });
	return {
		create: useMutation({ mutationFn: (input: NewPdfAnnotation) => transport.createPdfAnnotation(mediaId!, input), onSuccess: refresh }),
		update: useMutation({ mutationFn: ({ annotationId, ...change }: { annotationId: string; color?: PdfAnnotationColor; content?: string }) => transport.updatePdfAnnotation(annotationId, change), onSuccess: refresh }),
		remove: useMutation({ mutationFn: (annotationId: string) => transport.deletePdfAnnotation(annotationId), onSuccess: refresh })
	};
}
