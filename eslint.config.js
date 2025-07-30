import js from '@eslint/js';
import tseslint from '@typescript-eslint/eslint-plugin';
import tsparser from '@typescript-eslint/parser';
import svelte from 'eslint-plugin-svelte';
import svelteparser from 'svelte-eslint-parser';

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
      globals: {
        browser: true,
        es2017: true,
        node: true
      }
    },
    plugins: {
      '@typescript-eslint': tseslint
    },
    rules: {
      ...tseslint.configs.recommended.rules
    }
  },

  // Svelte files
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parser: svelteparser,
      parserOptions: {
        parser: tsparser
      }
    },
    plugins: {
      svelte
    },
    rules: {
      ...svelte.configs.recommended.rules
    }
  }
];
