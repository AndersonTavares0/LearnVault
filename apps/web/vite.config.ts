import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [svelte(), {
		name: 'learnvault-server-entry',
		enforce: 'post',
		generateBundle(_options, bundle) {
			const shell = Object.values(bundle).find((entry) => entry.type === 'asset' && entry.fileName.endsWith('app.html'));
			if (shell?.type === 'asset' && typeof shell.source === 'string') this.emitFile({ type: 'asset', fileName: '200.html', source: shell.source });
		}
	}],
	define: { __LEARNVAULT_TRANSPORT__: JSON.stringify(process.env.VITE_TRANSPORT === 'desktop' ? 'desktop' : 'http') },
	build: {
		outDir: '../../services/go/cmd/learnvault/web/build',
		assetsDir: '_app/immutable',
		emptyOutDir: true,
		rollupOptions: { input: { '200': 'src/app.html' } }
	}
});
