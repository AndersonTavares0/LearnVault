import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';
import { copyFileSync } from 'node:fs';
import { join } from 'node:path';

export default defineConfig({
	plugins: [svelte(), {
		name: 'learnvault-server-entry',
		closeBundle() {
			const output = join(process.cwd(), '../../services/go/cmd/learnvault/web/build');
			copyFileSync(join(output, 'src/app.html'), join(output, '200.html'));
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
