import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig(({ mode }) => {
	const desktop = mode === 'desktop' || process.env.VITE_TRANSPORT === 'desktop';
	return {
	plugins: [svelte(), {
		name: 'learnvault-server-entry',
		enforce: 'post',
		generateBundle(_options, bundle) {
			const shell = Object.values(bundle).find((entry) => entry.type === 'asset' && entry.fileName.endsWith('app.html'));
			if (shell?.type === 'asset' && typeof shell.source === 'string') this.emitFile({ type: 'asset', fileName: '200.html', source: shell.source });
		}
	}, {
		// O build usa `src/app.html` como entrada; em desenvolvimento, o mesmo shell
		// é servido na raiz para a SPA abrir em `http://localhost:5173/`.
		name: 'learnvault-dev-shell',
		apply: 'serve',
		configureServer(server) {
			server.middlewares.use((request, _response, next) => {
				if (request.url?.split('?')[0] === '/') request.url = '/src/app.html';
				next();
			});
		}
	}],
	define: { __LEARNVAULT_TRANSPORT__: JSON.stringify(desktop ? 'desktop' : 'http') },
	// O servidor de desenvolvimento da SPA encaminha `/api` ao backend local; o build
	// continua gerando um bundle com URLs relativas, sem depender deste proxy.
	server: {
		proxy: {
			'/api': { target: process.env.LEARNVAULT_API_TARGET ?? 'http://127.0.0.1:8090' }
		}
	},
	build: {
		// O servidor Rust consome a saída canônica; o aplicativo Tauri embute o shell
		// a partir de `crates/desktop/frontend-dist`.
		outDir: desktop ? '../../crates/desktop/frontend-dist' : '../../crates/server/web/build',
		assetsDir: '_app/immutable',
		emptyOutDir: true,
		// O webview do desktop é sempre atual e não precisa do polyfill de
		// `modulepreload`, que injeta um `fetch` de rede no bundle offline.
		modulePreload: desktop ? { polyfill: false } : {},
		rollupOptions: { input: { '200': 'src/app.html' } }
	}
	};
});
