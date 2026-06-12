import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  test: {
    environment: 'node',
    pool: 'forks',
    include: ['src/tests/**/*.test.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'json-summary'],
      reportsDirectory: './coverage',
      // Tests run in a Node environment by design, so .svelte components and the
      // Electron/browser bootstrap layers aren't exercised here — excluding them
      // keeps the number focused on the pure logic the suite actually covers.
      include: ['src/**/*.ts'],
      exclude: [
        'src/tests/**',
        'src/**/*.svelte',
        'src/**/*.svelte.ts',
        'src/renderer/browserIpc.ts',
        'src/renderer/showcase/**',
        'src/**/*.d.ts',
      ],
      // Floor set just under the current baseline (~44%). Ratchet up as more
      // pipeline/CLI/component tests land.
      thresholds: {
        statements: 43,
        branches: 42,
        functions: 46,
        lines: 43,
      },
    },
  },
});
