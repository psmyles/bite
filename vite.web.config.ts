import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// Web-only build — no Electron plugin, no main/preload targets.
// Node definitions are bundled statically via import.meta.glob in browserIpc.ts.
//
// GitHub Pages base URL:
//   '/'          → user/org site  (https://username.github.io/)
//   '/imgplex/'  → project site   (https://username.github.io/imgplex/)
export default defineConfig({
  plugins: [svelte()],
  base: '/imgplex/',
  build: {
    outDir: 'dist-web',
    emptyOutDir: true,
  },
})
