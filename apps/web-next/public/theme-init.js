// Runs before the first paint so a learner never sees a white flash on the dark default or selected theme.
// It only sets data-theme and color-scheme; the full appearance is applied by the app (src/lib/appearance.ts).
// A separate file, not an inline script, because the Content-Security-Policy allows only 'self' scripts.
(function () {
	var root = document.documentElement;
	var dark = true;
	try {
		var raw = window.localStorage.getItem('learnvault-appearance');
		var parsed = raw ? JSON.parse(raw) : null;
		var theme = parsed && typeof parsed.theme === 'string' ? parsed.theme : 'dark';
		dark = theme === 'dark' || (theme === 'system' && window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches);
	} catch {
		// Unreadable storage: keep the dark default.
	}
	root.setAttribute('data-theme', dark ? 'dark' : 'light');
	root.style.colorScheme = dark ? 'dark' : 'light';
})();
