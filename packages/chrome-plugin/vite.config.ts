import { crx } from '@crxjs/vite-plugin';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';
import sveltePreprocess from 'svelte-preprocess';
import { defineConfig, loadEnv, type PluginOption } from 'vite';
import manifest from './src/manifest';

export default defineConfig(({ mode }) => {
	const env = loadEnv(mode, process.cwd(), '');

	const browser = env.TARGET_BROWSER ?? 'chrome';

	if (browser !== 'chrome' && browser !== 'firefox') {
		throw new Error('UNSUPPORTED BROWSER TYPE');
	}

	console.log(`Building for ${browser}`);

	const production = mode === 'production';

	return {
		build: {
			minify: false,
			outDir: 'build',
			rollupOptions: {
				output: {
					chunkFileNames: 'assets/chunk-[hash].js',
				},
			},
		},
		plugins: [
			tailwindcss(),
			crx({ manifest, browser }) as unknown as PluginOption,
			svelte({
				compilerOptions: {
					dev: !production,
				},
				preprocess: sveltePreprocess(),
			}),
		],
		resolve: {
			alias: {
				'@': path.resolve(__dirname, 'src'),
			},
		},
		legacy: {
			skipWebSocketTokenCheck: true,
		},
	};
});
