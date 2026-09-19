import { defineConfig } from 'vite';
import path from 'node:path';

export default defineConfig({
  build: {
    lib: {
      entry: path.resolve(__dirname, 'benchmark-node-import.ts'),
      formats: ['es'],
      fileName: () => 'node-import.mjs',
    },
    outDir: path.resolve(__dirname, 'out/benchmark-tools'),
    emptyOutDir: true,
    rollupOptions: { external: (id: string) => id.startsWith('node:') || id === 'electron' },
    target: 'node20',
    minify: false,
    ssr: true,
  },
});
