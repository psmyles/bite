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
    // .svelte.ts files are plain TypeScript — override the Svelte parser that
    // would otherwise be applied because the filename contains ".svelte"
    files: ['**/*.svelte.ts'],
    languageOptions: {
      parser: tseslint.parser,
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
  },
  {
    // CommonJS build scripts legitimately use require() — they run under plain Node,
    // not the bundler, so the TS-eslint ESM rule does not apply to them.
    files: ['**/*.cjs'],
    rules: {
      '@typescript-eslint/no-require-imports': 'off',
    },
  },
  {
    rules: {
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          varsIgnorePattern: '^_',
          argsIgnorePattern: '^_',
          ignoreRestSiblings: true,
        },
      ],
    },
  },
  {
    ignores: [
      '.claude/',
      '.github/',
      '.husky/',
      'build/',
      'coverage/',
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
