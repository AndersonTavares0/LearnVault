import adapter from '@sveltejs/adapter-static';

export default {
	kit: {
		files: {
			routes: 'web/src/routes',
			appTemplate: 'web/src/app.html'
		},
		adapter: adapter({
			fallback: '200.html',
			pages: 'cmd/learnvault/web/build',
			assets: 'cmd/learnvault/web/build'
		})
	}
};
