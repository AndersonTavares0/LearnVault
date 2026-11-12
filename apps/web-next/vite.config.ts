import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

// Mirrors apps/web/vite.config.ts: same transport switch, shell and asset layout. Output goes to
// local dist folders until the cutover (#340) points it at the server and desktop paths.
export default defineConfig(({ mode }) => {
	const desktop = mode === 'desktop' || process.env.VITE_TRANSPORT === 'desktop';
	return {
		plugins: [
			react(),
			tailwindcss(),
			{
				name: 'learnvault-server-entry',
				enforce: 'post',
				generateBundle(_options, bundle) {
					const shell = Object.values(bundle).find((entry) => entry.type === 'asset' && entry.fileName.endsWith('app.html'));
					if (shell?.type === 'asset' && typeof shell.source === 'string') this.emitFile({ type: 'asset', fileName: '200.html', source: shell.source });
				}
			},
			{
				name: 'learnvault-dev-shell',
				apply: 'serve',
				configureServer(server) {
					server.middlewares.use((request, _response, next) => {
						if (request.url?.split('?')[0] === '/') request.url = '/src/app.html';
						next();
					});
				}
			}
		],
		resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
		define: { __LEARNVAULT_TRANSPORT__: JSON.stringify(desktop ? 'desktop' : 'http') },
		server: {
			port: 5174,
			proxy: { '/api': { target: process.env.LEARNVAULT_API_TARGET ?? 'http://127.0.0.1:8090' } }
		},
		build: {
			outDir: desktop ? '../../crates/desktop/frontend-dist' : '../../crates/server/web/build',
			assetsDir: '_app/immutable',
			// The server CSP only allows fonts from 'self'; inlined `data:` fonts would be blocked.
			assetsInlineLimit: 0,
			emptyOutDir: true,
			// The desktop webview is current and the polyfill would add a network `fetch` to the offline bundle.
			modulePreload: desktop ? { polyfill: false } : {},
			rollupOptions: { input: { '200': 'src/app.html' } }
		},
		test: { environment: 'jsdom' }
	};
});
