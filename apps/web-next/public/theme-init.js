// Runs before the first paint so a learner who chose the dark theme never sees a white flash.
// It only sets data-theme and color-scheme; the full appearance is applied by the app (src/lib/appearance.ts).
// A separate file, not an inline script, because the Content-Security-Policy allows only 'self' scripts.
(function () {
	try {
		var raw = window.localStorage.getItem('learnvault-appearance');
		var theme = raw ? JSON.parse(raw).theme : null;
		var dark = theme === 'dark' || (theme === 'system' && window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches);
		var root = document.documentElement;
		root.setAttribute('data-theme', dark ? 'dark' : 'light');
		root.style.colorScheme = dark ? 'dark' : 'light';
	} catch {
		// Unreadable storage: the light default stays.
	}
})();
