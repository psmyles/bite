import eslint from '@eslint/js';
import tseslint from 'typescript-eslint';
import eslintPluginSvelte from 'eslint-plugin-svelte';

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  ...eslintPluginSvelte.configs['flat/recommended'],
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
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
