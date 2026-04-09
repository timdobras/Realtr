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
      // and failed CI on every browser/Node global. Svelte 5 rune names are added
      // explicitly so .svelte.ts store files don't trip no-undef.
      globals: {
        ...globals.browser,
        ...globals.node,
        $state: 'readonly',
        $derived: 'readonly',
        $effect: 'readonly',
        $props: 'readonly',
        $bindable: 'readonly',
        $inspect: 'readonly',
        $host: 'readonly'
      }
    },
    plugins: {
      '@typescript-eslint': tseslint
    },
    rules: {
      ...tseslint.configs.recommended.rules,
      // Allow `_`-prefixed args/vars and unused catch bindings — both are
      // standard conventions for "intentionally ignored" identifiers.
      'no-unused-vars': [
        'error',
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
      'no-unused-vars': [
        'error',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrors: 'none'
        }
      ]
    }
  }
];
