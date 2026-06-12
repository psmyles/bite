import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  test: {
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
      // Floor set just under the current baseline. Ratchet up as more
      // pipeline/CLI/component tests land.
      thresholds: {
        statements: 46,
        branches: 45,
        functions: 50,
        lines: 46,
      },
    },
    // Two projects: pure logic runs fast in Node; Svelte component tests need a
    // DOM and the *client* build of svelte (browser export condition), so they
    // get their own jsdom project. Splitting keeps the Node suite isolated from
    // the browser resolve conditions.
    projects: [
      {
        plugins: [svelte({ hot: false })],
        test: {
          name: 'node',
          environment: 'node',
          pool: 'forks',
          include: ['src/tests/**/*.test.ts'],
          exclude: ['src/tests/**/*.svelte.test.ts'],
        },
      },
      {
        plugins: [svelte({ hot: false })],
        // Resolve svelte's client (mount-capable) build, not the SSR build.
        resolve: { conditions: ['browser'] },
        test: {
          name: 'dom',
          environment: 'jsdom',
          include: ['src/tests/**/*.svelte.test.ts'],
          setupFiles: ['src/tests/setup-dom.ts'],
        },
      },
    ],
  },
});
