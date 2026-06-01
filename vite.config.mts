import { defineConfig } from 'vite';
import path from 'node:path';
import { readFileSync } from 'node:fs';
import electron from 'vite-plugin-electron/simple';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const readPkg = (p: string) => JSON.parse(readFileSync(new URL(p, import.meta.url), 'utf-8')) as Record<string, string>;
const { version } = readPkg('./package.json');
const svelteVersion = readPkg('./node_modules/svelte/package.json').version;
const xyflowVersion = readPkg('./node_modules/@xyflow/svelte/package.json').version;

// https://vitejs.dev/config/
export default defineConfig({
  build: {
    rollupOptions: {
      input: {
        main: './index.html',
        showcase: './ui-showcase.html',
      },
    },
  },
  define: {
    __APP_VERSION__: JSON.stringify(version),
    __SVELTE_VERSION__: JSON.stringify(svelteVersion),
    __XYFLOW_VERSION__: JSON.stringify(xyflowVersion),
  },
  plugins: [
    svelte(),
    electron({
      main: {
        // Multiple entries: Electron main process + standalone CLI
        entry: {
          main: 'electron/main.ts',
          cli: 'src/cli/index.ts',
        },
        vite: {},
      },
      preload: {
        // Shortcut of `build.rollupOptions.input`.
        // Preload scripts may contain Web assets, so use the `build.rollupOptions.input` instead `build.lib.entry`.
        input: path.join(__dirname, 'electron/preload.ts'),
      },
      // Ployfill the Electron and Node.js API for Renderer process.
      // If you want use Node.js in Renderer process, the `nodeIntegration` needs to be enabled in the Main process.
      // See 👉 https://github.com/electron-vite/vite-plugin-electron-renderer
      renderer:
        process.env.NODE_ENV === 'test'
          ? // https://github.com/electron-vite/vite-plugin-electron-renderer/issues/78#issuecomment-2053600808
            undefined
          : {},
    }),
  ],
});
