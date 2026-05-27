import eslint from '@eslint/js';
import tseslint from 'typescript-eslint';
import eslintPluginSvelte from 'eslint-plugin-svelte';
import globals from 'globals';

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  ...eslintPluginSvelte.configs['flat/recommended'],
  {
    files: ['**/*.svelte'],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
        __SVELTE_VERSION__: 'readonly',
        __XYFLOW_VERSION__: 'readonly',
        __APP_VERSION__: 'readonly',
      },
      parserOptions: {
        parser: tseslint.parser,
      },
    },
  },
  {
    files: ['**/*.ts', '**/*.js', '**/*.cjs', '**/*.mjs'],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
  },
  {
    ignores: [
      '.claude/',
      '.github/',
      '.husky/',
      'build/',
      'dist/',
      'dist-cli/',
      'dist-electron/',
      'dist-web/',
      'examples/',
      'node_modules/',
      'release/',
      'test_images/',
      'unity-tools/',
    ],
  }
);
