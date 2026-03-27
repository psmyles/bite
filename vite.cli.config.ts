// Standalone CLI build — outputs a single self-contained CJS bundle that pkg
// can compile into a native exe.  Svelte and renderer code are not included.
import { defineConfig } from 'vite'
import path from 'node:path'

export default defineConfig({
  build: {
    lib: {
      entry: { 'cli-bundle': path.resolve(__dirname, 'src/cli/index.ts') },
      formats: ['cjs'],
    },
    outDir: 'dist-cli',
    rollupOptions: {
      // Keep all node: builtins external — pkg provides them at runtime
      external: (id: string) => id.startsWith('node:') || id === 'electron',
    },
    minify: false,
    target: 'node20',
    ssr: true,        // prevents browser-specific transforms
    emptyOutDir: true,
  },
})
