export type ScreenName = 'library' | 'course' | 'player' | 'notes' | 'settings' | 'login' | 'analytics' | 'pomodoro' | 'discovery' | 'materials' | 'catalog-flow';

export type ScreenContract = {
	board: string;
	route: string;
	visualSource: string;
	dataSource: string;
	limitations: readonly string[];
};

/** Product screens only; boards 07, 11 and 14 are decision documents, not routes. */
export const graphiteScreens: Readonly<Record<ScreenName, ScreenContract>> = {
	library: { board: '01', route: '#/home', visualSource: '01-library.html', dataSource: 'catalog, recentStudy, course covers', limitations: ['No lesson duration or completion percentage is exposed by the current study API.'] },
	course: { board: '02', route: '#/course/:courseId', visualSource: '02-course.html', dataSource: 'catalog tree and recentStudy', limitations: ['Do not infer course completion percentage or lesson duration.'] },
	player: { board: '03', route: '#/player/:courseId/:lessonPath', visualSource: '03-player.html', dataSource: 'authorized media, progress, subtitles, materials', limitations: ['Use browser media controls where custom controls would misrepresent native playback.'] },
	notes: { board: '04', route: '#/notes', visualSource: '04-notes.html', dataSource: 'per-owner study notes', limitations: ['Demo note text and counts are not real user information.'] },
	settings: { board: '05', route: '#/settings', visualSource: '05-settings.html', dataSource: 'theme, authenticated account, update capability', limitations: ['Do not show unavailable palettes as selectable.'] },
	login: { board: '06', route: '#/login', visualSource: '06-login.html', dataSource: 'HTTP authentication', limitations: ['Desktop intentionally has no login route.'] },
	analytics: { board: '09', route: '#/analytics', visualSource: '09-analytics.html', dataSource: 'owner-scoped study aggregates', limitations: ['No historical time series or per-course metrics without a contract.'] },
	pomodoro: { board: '08', route: 'global panel', visualSource: '08-pomodoro.html', dataSource: 'persisted owner-scoped timer state', limitations: ['Reference values are illustrative, not initial user state.'] },
	discovery: { board: '12', route: 'import preview', visualSource: '12-discovery.html', dataSource: 'read-only discovery + explainable interpretation', limitations: ['Preview does not write catalog state.'] },
	materials: { board: '13', route: 'course/player material panel', visualSource: '13-materials.html', dataSource: 'allowlisted catalog materials', limitations: ['Only authorized content types can be previewed or downloaded.'] },
	'catalog-flow': { board: '15', route: 'admin import flow', visualSource: '15-catalog-flow.html', dataSource: 'desktop selector or validated server path', limitations: ['Import requires explicit confirmation and never mutates the source folder.'] }
};
