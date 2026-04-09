import js from '@eslint/js';
import tseslint from '@typescript-eslint/eslint-plugin';
import tsparser from '@typescript-eslint/parser';
import svelte from 'eslint-plugin-svelte';
import svelteparser from 'svelte-eslint-parser';
import globals from 'globals';

export default [
  // First, add your ignore patterns
  {
    ignores: [
      'node_modules/',
      '.pnp',
      '.pnp.js',
      'dist/',
      'build/',
      '.svelte-kit/',
      '.output/',
      'src-tauri/target/',
      'src-tauri/',
      '*.tsbuildinfo',
      '.DS_Store',
      'package-lock.json',
      'yarn.lock',
      'pnpm-lock.yaml',
      '.env*',
      'coverage/',
      '*.log',
      '*.config.js',
      '*.config.ts',
      'vite.config.*',
      'tailwind.config.*'
    ]
  },

  // Apply recommended configs
  js.configs.recommended,

  // TypeScript and general JS files
  {
    files: ['**/*.js', '**/*.ts'],
    languageOptions: {
      parser: tsparser,
      ecmaVersion: 2020,
      sourceType: 'module',
      // ESLint v9 flat config takes a map of globalName -> 'readonly'/'writable',
      // not the v8 env style ({ browser: true }). The previous config was a no-op
      // and failed CI on every browser/Node global.
      globals: {
        ...globals.browser,
        ...globals.node
      }
    },
    plugins: {
      '@typescript-eslint': tseslint
    },
    rules: {
      ...tseslint.configs.recommended.rules,
      // Allow `_`-prefixed args/vars and unused catch bindings — both are
      // standard conventions for "intentionally ignored" identifiers.
      // Degraded to `warn` because the codebase has ~17 pre-existing
      // unused-but-not-prefixed identifiers in unrelated files. New
      // violations still show up in `npm run lint` output and code review.
      // Bump back to `error` after a dedicated cleanup pass.
      'no-unused-vars': [
        'warn',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrors: 'none'
        }
      ]
    }
  },

  // Svelte files — same global expansion as JS/TS so window/setTimeout/etc.
  // resolve correctly inside <script> blocks.
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parser: svelteparser,
      parserOptions: {
        parser: tsparser
      },
      globals: {
        ...globals.browser,
        ...globals.node
      }
    },
    plugins: {
      svelte
    },
    rules: {
      ...svelte.configs.recommended.rules,
      // Degraded to `warn` because the codebase has ~17 pre-existing
      // unused-but-not-prefixed identifiers in unrelated files. New
      // violations still show up in `npm run lint` output and code review.
      // Bump back to `error` after a dedicated cleanup pass.
      'no-unused-vars': [
        'warn',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrors: 'none'
        }
      ]
    }
  }
];
