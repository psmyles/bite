import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Web-only build - no Electron plugin, no main/preload targets.
// Node definitions are bundled statically via import.meta.glob in browserIpc.ts.
//
// GitHub Pages base URL:
//   '/'          -> user/org site  (https://username.github.io/)
//   '/bite/'  -> project site   (https://username.github.io/bite/)
export default defineConfig({
  plugins: [svelte()],
  base: '/bite/',
  build: {
    outDir: 'dist-web',
    emptyOutDir: true,
  },
});
