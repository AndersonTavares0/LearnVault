import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		sveltekit({
			files: {
				routes: 'web/src/routes',
				appTemplate: 'web/src/app.html'
			},
			adapter: adapter({
				fallback: '200.html',
				pages: 'services/go/cmd/learnvault/web/build',
				assets: 'services/go/cmd/learnvault/web/build'
			})
		})
	]
});
