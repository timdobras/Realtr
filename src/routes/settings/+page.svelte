<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { appDataDir } from '@tauri-apps/api/path';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { ScanResult } from '$lib/types/database';

  // TypeScript interfaces
  interface AppConfig {
    rootPath: string;
    isValidPath: boolean;
    lastUpdated: string | null;
    fast_editor_path?: string;
    fast_editor_name?: string;
    complex_editor_path?: string;
    complex_editor_name?: string;
    watermark_image_path?: string;
    watermark_opacity?: number;
  }

  interface CommandResult {
    success: boolean;
    error?: string;
  }

  // Reactive state for settings
  let config = $state<AppConfig>({
    rootPath: '',
    isValidPath: false,
    lastUpdated: null,
    fast_editor_path: undefined,
    fast_editor_name: undefined,
    complex_editor_path: undefined,
    complex_editor_name: undefined,
    watermark_image_path: '',
    watermark_opacity: 0.15
  });

  let isLoading = $state<boolean>(false);
  let statusMessage = $state<string>('');
  let statusType = $state<'success' | 'error' | 'info'>('info');

  // Load config on mount
  onMount(async () => {
    await loadConfig();
  });

  // Load existing config
  async function loadConfig(): Promise<void> {
    try {
      isLoading = true;
      statusMessage = 'Loading configuration...';
      statusType = 'info';

      const loadedConfig = await invoke<AppConfig | null>('load_config');
      if (loadedConfig) {
        config = { ...config, ...loadedConfig };
        statusMessage = 'Configuration loaded successfully';
        statusType = 'success';
      } else {
        statusMessage = 'No existing configuration found';
        statusType = 'info';
      }
    } catch (error) {
      console.error('Error loading config:', error);
      statusMessage = `Error loading configuration: ${error}`;
      statusType = 'error';
    } finally {
      isLoading = false;
    }
  }

  // Open folder dialog
  async function selectFolder(): Promise<void> {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select root folder for photo storage'
      });

      if (selected && typeof selected === 'string') {
        config.rootPath = selected;
        await validateAndCreateStructure();
      }
    } catch (error) {
      console.error('Error selecting folder:', error);
      statusMessage = `Error selecting folder: ${error}`;
      statusType = 'error';
    }
  }

  // Validate path and create folder structure
  async function validateAndCreateStructure(): Promise<void> {
    try {
      isLoading = true;
      statusMessage = 'Validating path and creating folder structure...';
      statusType = 'info';

      const result = await invoke<CommandResult>('setup_folder_structure', {
        rootPath: config.rootPath
      });

      if (result.success) {
        config.isValidPath = true;
        statusMessage = 'Folder structure created successfully!';
        statusType = 'success';
      } else {
        config.isValidPath = false;
        statusMessage = result.error || 'Failed to create folder structure';
        statusType = 'error';
      }
    } catch (error) {
      console.error('Error creating folder structure:', error);
      config.isValidPath = false;
      statusMessage = `Error: ${error}`;
      statusType = 'error';
    } finally {
      isLoading = false;
    }
  }

  // Save configuration
  async function saveConfig(): Promise<void> {
    if (!config.rootPath || !config.isValidPath) {
      statusMessage = 'Please select a valid root folder first';
      statusType = 'error';
      return;
    }

    try {
      isLoading = true;
      statusMessage = 'Saving configuration...';
      statusType = 'info';

      const result = await invoke<CommandResult>('save_config', {
        config: {
          rootPath: config.rootPath,
          isValidPath: config.isValidPath,
          lastUpdated: new Date().toISOString(),
          fast_editor_path: config.fast_editor_path,
          fast_editor_name: config.fast_editor_name,
          complex_editor_path: config.complex_editor_path,
          complex_editor_name: config.complex_editor_name,
          watermark_image_path: config.watermark_image_path,
          watermark_opacity: config.watermark_opacity
        }
      });

      if (result.success) {
        config.lastUpdated = new Date().toISOString();
        statusMessage = 'Configuration saved successfully!';
        statusType = 'success';
      } else {
        statusMessage = result.error || 'Failed to save configuration';
        statusType = 'error';
      }
    } catch (error) {
      console.error('Error saving config:', error);
      statusMessage = `Error saving configuration: ${error}`;
      statusType = 'error';
    } finally {
      isLoading = false;
    }
  }

  // Reset configuration
  async function resetConfig(): Promise<void> {
    if (
      confirm('Are you sure you want to reset the configuration? This will clear all settings.')
    ) {
      try {
        await invoke<CommandResult>('reset_config');
        config = {
          rootPath: '',
          isValidPath: false,
          lastUpdated: null
        };
        statusMessage = 'Configuration reset successfully';
        statusType = 'success';
      } catch (error) {
        statusMessage = `Error resetting configuration: ${error}`;
        statusType = 'error';
      }
    }
  }

  let isScanning = $state(false);
  let scanResult = $state<ScanResult | null>(null);
  let showScanResult = $state(false);

  async function scanAndImport() {
    if (!config.rootPath || !config.isValidPath) {
      statusMessage = 'Please set up your root folder first';
      statusType = 'error';
      return;
    }

    try {
      isScanning = true;
      statusMessage = 'Scanning folder for existing properties...';
      statusType = 'info';

      scanResult = await DatabaseService.scanAndImportProperties();
      showScanResult = true;

      if (scanResult?.newProperties && scanResult.newProperties > 0) {
        statusMessage = `Successfully imported ${scanResult.newProperties} new properties!`;
        statusType = 'success';
      } else {
        statusMessage = 'Scan completed. No new properties found.';
        statusType = 'info';
      }
    } catch (error) {
      console.error('Error scanning properties:', error);
      statusMessage = `Error scanning properties: ${error}`;
      statusType = 'error';
    } finally {
      isScanning = false;
    }
  }

  async function debugDatabase() {
    try {
      await invoke('debug_database_dates');
      statusMessage = 'Check the console for database debug info';
      statusType = 'info';
    } catch (error) {
      statusMessage = `Debug failed: ${error}`;
      statusType = 'error';
    }
  }

  async function resetDatabase() {
    if (confirm('This will completely reset the database and delete all data. Are you sure?')) {
      try {
        await invoke('reset_database_with_proper_dates');
        statusMessage = 'Database reset successfully';
        statusType = 'success';
      } catch (error) {
        statusMessage = `Reset failed: ${error}`;
        statusType = 'error';
      }
    }
  }

  // Function to select fast editor
  async function selectFastEditor(): Promise<void> {
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        title: 'Select Fast Image Editor',
        filters: [
          {
            name: 'Executable Files',
            extensions: ['exe', 'app']
          }
        ]
      });

      if (selected && typeof selected === 'string') {
        config.fast_editor_path = selected;
        const pathParts = selected.split(/[/\\]/);
        const filename = pathParts[pathParts.length - 1];
        config.fast_editor_name = filename.replace(/\.(exe|app)$/i, '');

        statusMessage = `Fast editor set to: ${config.fast_editor_name}`;
        statusType = 'success';
      }
    } catch (error) {
      console.error('Error selecting fast editor:', error);
      statusMessage = `Error selecting fast editor: ${error}`;
      statusType = 'error';
    }
  }

  // Function to select complex editor
  async function selectComplexEditor(): Promise<void> {
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        title: 'Select Complex Image Editor (Photoshop, GIMP, etc.)',
        filters: [
          {
            name: 'Executable Files',
            extensions: ['exe', 'app']
          }
        ]
      });

      if (selected && typeof selected === 'string') {
        config.complex_editor_path = selected;
        const pathParts = selected.split(/[/\\]/);
        const filename = pathParts[pathParts.length - 1];
        config.complex_editor_name = filename.replace(/\.(exe|app)$/i, '');

        statusMessage = `Complex editor set to: ${config.complex_editor_name}`;
        statusType = 'success';
      }
    } catch (error) {
      console.error('Error selecting complex editor:', error);
      statusMessage = `Error selecting complex editor: ${error}`;
      statusType = 'error';
    }
  }

  // Function to reset editor selections
  function resetEditors(): void {
    config.fast_editor_path = undefined;
    config.fast_editor_name = undefined;
    config.complex_editor_path = undefined;
    config.complex_editor_name = undefined;
    statusMessage = 'Editor selections cleared';
    statusType = 'info';
  }

  // Add watermark selection function
  async function selectWatermarkImage(): Promise<void> {
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        title: 'Select Watermark Image',
        filters: [
          {
            name: 'Image Files',
            extensions: ['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp']
          }
        ]
      });

      if (selected && typeof selected === 'string') {
        config.watermark_image_path = selected;
        statusMessage = 'Watermark image selected successfully';
        statusType = 'success';
      }
    } catch (error) {
      console.error('Error selecting watermark image:', error);
      statusMessage = `Error selecting watermark image: ${error}`;
      statusType = 'error';
    }
  }

  // Add function to clear watermark settings
  function clearWatermarkSettings(): void {
    config.watermark_image_path = '';
    config.watermark_opacity = 0.15;
    statusMessage = 'Watermark settings cleared';
    statusType = 'info';
  }
</script>

<div class="bg-background-0 min-h-full">
  <!-- Header -->
  <div class="bg-background-50 border-background-200 border-b">
    <div class="px-6 py-8">
      <div class="mx-auto max-w-4xl">
        <h1 class="text-foreground-900 text-3xl font-bold">Settings</h1>
        <p class="text-foreground-600 mt-2">Configure your property management workspace</p>
      </div>
    </div>
  </div>

  <div class="mx-auto max-w-4xl space-y-8 p-6">
    <!-- Status Message -->
    {#if statusMessage}
      <div
        class="rounded-lg border p-4 {statusType === 'success'
          ? 'border-green-200 bg-green-50'
          : statusType === 'error'
            ? 'border-red-200 bg-red-50'
            : 'border-accent-200 bg-accent-50'}"
      >
        <div class="flex items-center space-x-3">
          <div class="flex-shrink-0">
            {#if statusType === 'success'}
              <svg
                class="h-5 w-5 text-green-600"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
            {:else if statusType === 'error'}
              <svg
                class="h-5 w-5 text-red-600"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
            {:else}
              <svg
                class="text-accent-600 h-5 w-5"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
            {/if}
          </div>
          <p
            class="text-sm font-medium {statusType === 'success'
              ? 'text-green-800'
              : statusType === 'error'
                ? 'text-red-800'
                : 'text-accent-800'}"
          >
            {statusMessage}
          </p>
        </div>
      </div>
    {/if}

    <!-- Folder Configuration -->
    <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-6 flex items-center space-x-3">
        <div class="bg-accent-100 flex h-10 w-10 items-center justify-center rounded-lg">
          <svg
            class="text-accent-600 h-5 w-5"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
            />
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M8 5a2 2 0 012-2h4a2 2 0 012 2v6H8V5z"
            />
          </svg>
        </div>
        <div>
          <h2 class="text-foreground-900 text-xl font-semibold">Folder Configuration</h2>
          <p class="text-foreground-600 text-sm">Set up your photo storage directory</p>
        </div>
      </div>

      <div class="space-y-6">
        <!-- Root Folder Selection -->
        <div>
          <label class="text-foreground-700 mb-3 block text-sm font-medium">
            Root Storage Folder
          </label>
          <div class="flex items-center space-x-4">
            <div class="min-w-0 flex-1">
              <input
                type="text"
                readonly
                value={config.rootPath || 'No folder selected'}
                class="text-foreground-900 border-background-300 bg-background-100 focus:ring-accent-500 focus:border-accent-500 w-full rounded-lg border px-4 py-3 focus:ring-2 focus:outline-none"
                placeholder="Select a folder to store your photos"
              />
            </div>
            <button
              onclick={selectFolder}
              disabled={isLoading}
              class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
                />
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M8 5a2 2 0 012-2h4a2 2 0 012 2v6H8V5z"
                />
              </svg>
              <span>Browse</span>
            </button>
          </div>
          <p class="text-foreground-500 mt-2 text-sm">
            This folder will contain all your photo projects and processed images
          </p>
        </div>

        <!-- Folder Structure Preview -->
        {#if config.rootPath}
          <div class="border-background-300 bg-background-100 rounded-lg border p-4">
            <div class="mb-3 flex items-center space-x-2">
              <svg
                class="text-foreground-600 h-4 w-4"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
                />
              </svg>
              <h3 class="text-foreground-900 font-medium">Folder Structure Preview</h3>
            </div>
            <div
              class="text-foreground-700 bg-background-200 space-y-1 rounded p-3 font-mono text-sm"
            >
              <div>📁 {config.rootPath}</div>
              <div class="ml-4">├── 📁 FOTOGRAFIES - DONE</div>
              <div class="ml-8">│ └── 📁 [CITY/STREETANDNUMBER]</div>
              <div class="ml-12">│ ├── 📁 INTERNET</div>
              <div class="ml-16">│ │ └── 📁 AGGELIA</div>
              <div class="ml-12">│ ├── 📁 WATERMARK</div>
              <div class="ml-16">│ │ └── 📁 AGGELIA</div>
              <div class="ml-12">│ └── 📄 [original images]</div>
              <div class="ml-4">└── 📁 FOTOGRAFIES - NEW</div>
              <div class="ml-8">└── 📁 [CITY/STREETANDNUMBER]</div>
              <div class="ml-12">├── 📁 INTERNET</div>
              <div class="ml-16">│ └── 📁 AGGELIA</div>
              <div class="ml-12">├── 📁 WATERMARK</div>
              <div class="ml-16">│ └── 📁 AGGELIA</div>
              <div class="ml-12">└── 📄 [original images]</div>
            </div>
          </div>
        {/if}

        <!-- Status Indicators -->
        <div class="bg-background-100 flex items-center space-x-6 rounded-lg p-4">
          <div class="flex items-center space-x-2">
            <div
              class="h-3 w-3 rounded-full {config.rootPath ? 'bg-green-500' : 'bg-background-300'}"
            ></div>
            <span class="text-foreground-600 text-sm font-medium">Folder Selected</span>
          </div>
          <div class="flex items-center space-x-2">
            <div
              class="h-3 w-3 rounded-full {config.isValidPath
                ? 'bg-green-500'
                : 'bg-background-300'}"
            ></div>
            <span class="text-foreground-600 text-sm font-medium">Structure Created</span>
          </div>
          <div class="flex items-center space-x-2">
            <div
              class="h-3 w-3 rounded-full {config.lastUpdated
                ? 'bg-green-500'
                : 'bg-background-300'}"
            ></div>
            <span class="text-foreground-600 text-sm font-medium">Configuration Saved</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Configuration Details -->
    {#if config.lastUpdated}
      <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
        <div class="mb-6 flex items-center space-x-3">
          <div class="bg-accent-100 flex h-10 w-10 items-center justify-center rounded-lg">
            <svg
              class="text-accent-600 h-5 w-5"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
              />
            </svg>
          </div>
          <div>
            <h2 class="text-foreground-900 text-xl font-semibold">Configuration Details</h2>
            <p class="text-foreground-600 text-sm">Current application settings</p>
          </div>
        </div>

        <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
          <div class="bg-background-100 rounded-lg p-4">
            <span class="text-foreground-600 text-sm font-medium">Last Updated</span>
            <p class="text-foreground-900 mt-1 text-sm">
              {new Date(config.lastUpdated).toLocaleString()}
            </p>
          </div>
          <div class="bg-background-100 rounded-lg p-4">
            <span class="text-foreground-600 text-sm font-medium">Status</span>
            <p
              class="mt-1 text-sm {config.isValidPath
                ? 'text-green-600'
                : 'text-red-600'} font-medium"
            >
              {config.isValidPath ? 'Ready' : 'Not Ready'}
            </p>
          </div>
          <div class="bg-background-100 rounded-lg p-4">
            <span class="text-foreground-600 text-sm font-medium">Fast Editor</span>
            <p class="text-foreground-900 mt-1 text-sm">
              {config.fast_editor_name || 'System Default'}
            </p>
          </div>
          <div class="bg-background-100 rounded-lg p-4">
            <span class="text-foreground-600 text-sm font-medium">Complex Editor</span>
            <p class="text-foreground-900 mt-1 text-sm">
              {config.complex_editor_name || 'System Default'}
            </p>
          </div>
        </div>

        <div class="bg-background-100 mt-4 rounded-lg p-4">
          <span class="text-foreground-600 text-sm font-medium">Root Path</span>
          <p class="text-foreground-900 mt-1 font-mono text-sm break-all">{config.rootPath}</p>
        </div>
      </div>
    {/if}

    <!-- Import Existing Properties -->
    {#if config.isValidPath}
      <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
        <div class="mb-6 flex items-center space-x-3">
          <div class="bg-accent-100 flex h-10 w-10 items-center justify-center rounded-lg">
            <svg
              class="text-accent-600 h-5 w-5"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M9 19l3 3m0 0l3-3m-3 3V10"
              />
            </svg>
          </div>
          <div>
            <h2 class="text-foreground-900 text-xl font-semibold">Import Existing Properties</h2>
            <p class="text-foreground-600 text-sm">
              Scan your folder for existing properties and add them to the database
            </p>
          </div>
        </div>

        <button
          onclick={scanAndImport}
          disabled={isScanning}
          class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-3 rounded-lg px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
        >
          {#if isScanning}
            <div
              class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
            ></div>
            <span>Scanning...</span>
          {:else}
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
            <span>Scan & Import Properties</span>
          {/if}
        </button>
      </div>
    {/if}

    <!-- Image Editor Configuration -->
    <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-6 flex items-center space-x-3">
        <div class="bg-accent-100 flex h-10 w-10 items-center justify-center rounded-lg">
          <svg
            class="text-accent-600 h-5 w-5"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
            />
          </svg>
        </div>
        <div>
          <h2 class="text-foreground-900 text-xl font-semibold">Image Editor Configuration</h2>
          <p class="text-foreground-600 text-sm">
            Set up your preferred image editing applications
          </p>
        </div>
      </div>

      <div class="space-y-6">
        <!-- Fast Editor Selection -->
        <div>
          <label class="text-foreground-700 mb-3 block text-sm font-medium">
            Fast Image Editor
            <span class="text-foreground-500 font-normal"
              >(for quick edits, brightness, contrast)</span
            >
          </label>
          <div class="flex items-center space-x-4">
            <div class="min-w-0 flex-1">
              <input
                type="text"
                readonly
                value={config.fast_editor_name || 'System default (Windows Photos, etc.)'}
                class="text-foreground-900 border-background-300 bg-background-100 w-full rounded-lg border px-4 py-3 focus:outline-none"
                placeholder="No custom editor selected"
              />
              {#if config.fast_editor_path}
                <p class="text-foreground-500 mt-1 font-mono text-xs">{config.fast_editor_path}</p>
              {/if}
            </div>
            <button
              onclick={selectFastEditor}
              disabled={isLoading}
              class="bg-background-200 text-foreground-700 hover:bg-background-300 flex items-center space-x-2 rounded-lg px-4 py-3 font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
                />
              </svg>
              <span>Browse</span>
            </button>
          </div>
          <p class="text-foreground-500 mt-2 text-sm">
            Recommended: Windows Photos, IrfanView, FastStone Image Viewer
          </p>
        </div>

        <!-- Complex Editor Selection -->
        <div>
          <label class="text-foreground-700 mb-3 block text-sm font-medium">
            Advanced Image Editor
            <span class="text-foreground-500 font-normal"
              >(for complex edits, watermarks, masking)</span
            >
          </label>
          <div class="flex items-center space-x-4">
            <div class="min-w-0 flex-1">
              <input
                type="text"
                readonly
                value={config.complex_editor_name || 'System default'}
                class="text-foreground-900 border-background-300 bg-background-100 w-full rounded-lg border px-4 py-3 focus:outline-none"
                placeholder="No custom editor selected"
              />
              {#if config.complex_editor_path}
                <p class="text-foreground-500 mt-1 font-mono text-xs">
                  {config.complex_editor_path}
                </p>
              {/if}
            </div>
            <button
              onclick={selectComplexEditor}
              disabled={isLoading}
              class="bg-background-200 text-foreground-700 hover:bg-background-300 flex items-center space-x-2 rounded-lg px-4 py-3 font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v1a2 2 0 002 2h6a2 2 0 012 2v8a4 4 0 01-4 4H7z"
                />
              </svg>
              <span>Browse</span>
            </button>
          </div>
          <p class="text-foreground-500 mt-2 text-sm">
            Recommended: Adobe Photoshop, GIMP, Paint.NET, Photopea
          </p>
        </div>

        <!-- Editor Status & Actions -->
        <div class="border-background-200 flex items-center justify-between border-t pt-6">
          <button
            onclick={resetEditors}
            class="flex items-center space-x-1 text-sm font-medium text-red-600 hover:text-red-700"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
              />
            </svg>
            <span>Clear Editor Selections</span>
          </button>

          <div class="flex items-center space-x-6">
            <div class="flex items-center space-x-2">
              <div
                class="h-3 w-3 rounded-full {config.fast_editor_path
                  ? 'bg-green-500'
                  : 'bg-background-300'}"
              ></div>
              <span class="text-foreground-600 text-sm font-medium">Fast Editor</span>
            </div>
            <div class="flex items-center space-x-2">
              <div
                class="h-3 w-3 rounded-full {config.complex_editor_path
                  ? 'bg-green-500'
                  : 'bg-background-300'}"
              ></div>
              <span class="text-foreground-600 text-sm font-medium">Advanced Editor</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Watermark Configuration -->
    <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-6 flex items-center space-x-3">
        <div class="bg-accent-100 flex h-10 w-10 items-center justify-center rounded-lg">
          <svg
            class="text-accent-600 h-5 w-5"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"
            />
          </svg>
        </div>
        <div>
          <h2 class="text-foreground-900 text-xl font-semibold">Watermark Configuration</h2>
          <p class="text-foreground-600 text-sm">
            Set up your watermark image and opacity settings
          </p>
        </div>
      </div>

      <div class="space-y-6">
        <!-- Watermark Image Selection -->
        <div>
          <label class="text-foreground-700 mb-3 block text-sm font-medium">Watermark Image</label>
          <div class="flex items-center space-x-4">
            <div class="min-w-0 flex-1">
              <input
                type="text"
                readonly
                value={config.watermark_image_path || 'No watermark image selected'}
                class="text-foreground-900 border-background-300 bg-background-100 w-full rounded-lg border px-4 py-3 focus:outline-none"
                placeholder="Select a watermark image"
              />
              {#if config.watermark_image_path}
                <p class="text-foreground-500 mt-1 font-mono text-xs break-all">
                  {config.watermark_image_path}
                </p>
              {/if}
            </div>
            <button
              onclick={selectWatermarkImage}
              disabled={isLoading}
              class="bg-background-200 text-foreground-700 hover:bg-background-300 flex items-center space-x-2 rounded-lg px-4 py-3 font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"
                />
              </svg>
              <span>Browse</span>
            </button>
          </div>
          <p class="text-foreground-500 mt-2 text-sm">
            Recommended: PNG with transparent background for best results
          </p>
        </div>

        <!-- Opacity Slider -->
        <div>
          <label class="text-foreground-700 mb-3 block text-sm font-medium">
            Watermark Opacity: {Math.round(config.watermark_opacity * 100)}%
          </label>
          <div class="flex items-center space-x-4">
            <span class="text-foreground-500 text-sm font-medium">0%</span>
            <div class="flex-1">
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                bind:value={config.watermark_opacity}
                class="bg-background-300 slider h-2 w-full cursor-pointer appearance-none rounded-lg"
                style="background: linear-gradient(to right, #3b82f6 0%, #3b82f6 {config.watermark_opacity *
                  100}%, #e5e7eb {config.watermark_opacity * 100}%, #e5e7eb 100%);"
              />
            </div>
            <span class="text-foreground-500 text-sm font-medium">100%</span>
          </div>
          <p class="text-foreground-500 mt-2 text-sm">
            Adjust the transparency of the watermark overlay
          </p>
        </div>

        <!-- Preview Section -->
        {#if config.watermark_image_path}
          <div class="border-background-300 bg-background-100 rounded-lg border p-4">
            <div class="mb-3 flex items-center space-x-2">
              <svg
                class="text-foreground-600 h-4 w-4"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                />
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                />
              </svg>
              <h4 class="text-foreground-900 font-medium">Watermark Preview</h4>
            </div>
            <div class="flex items-center space-x-4">
              <div
                class="border-background-300 bg-background-50 flex h-16 w-16 items-center justify-center overflow-hidden rounded-lg border"
              >
                <img
                  src={`file://${config.watermark_image_path}`}
                  alt="Watermark preview"
                  class="max-h-full max-w-full object-contain"
                  style="opacity: {config.watermark_opacity}"
                  onerror={() => {}}
                />
              </div>
              <div class="text-foreground-600 space-y-1 text-sm">
                <p>
                  <span class="font-medium">Opacity:</span>
                  {Math.round(config.watermark_opacity * 100)}%
                </p>
                <p><span class="font-medium">Position:</span> Center</p>
                <p><span class="font-medium">Size:</span> Auto-fitted to image</p>
              </div>
            </div>
          </div>
        {/if}

        <!-- Watermark Actions -->
        <div class="border-background-200 flex items-center justify-between border-t pt-6">
          <button
            onclick={clearWatermarkSettings}
            class="flex items-center space-x-1 text-sm font-medium text-red-600 hover:text-red-700"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
              />
            </svg>
            <span>Clear Watermark Settings</span>
          </button>

          <div class="flex items-center space-x-2">
            <div
              class="h-3 w-3 rounded-full {config.watermark_image_path
                ? 'bg-green-500'
                : 'bg-background-300'}"
            ></div>
            <span class="text-foreground-600 text-sm font-medium">Watermark Image</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Debug Section (Temporary) -->
    {#if config.isValidPath}
      <div class="rounded-xl border border-orange-200 bg-orange-50 p-6">
        <div class="mb-4 flex items-center space-x-3">
          <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-orange-100">
            <svg
              class="h-5 w-5 text-orange-600"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
              />
            </svg>
          </div>
          <div>
            <h2 class="text-xl font-semibold text-orange-900">Debug Database (Development)</h2>
            <p class="text-sm text-orange-700">Temporary development tools</p>
          </div>
        </div>

        <div class="flex items-center space-x-4">
          <button
            onclick={debugDatabase}
            class="flex items-center space-x-2 rounded-lg bg-orange-200 px-4 py-2 font-medium text-orange-800 transition-colors hover:bg-orange-300"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
            <span>Debug Database Dates</span>
          </button>

          <button
            onclick={resetDatabase}
            class="flex items-center space-x-2 rounded-lg bg-red-600 px-4 py-2 font-medium text-white transition-colors hover:bg-red-700"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
              />
            </svg>
            <span>Reset Database</span>
          </button>
        </div>
      </div>
    {/if}

    <!-- Action Buttons -->
    <div class="border-background-200 flex items-center justify-between border-t pt-8">
      <button
        onclick={resetConfig}
        disabled={isLoading}
        class="flex items-center space-x-2 rounded-lg bg-red-100 px-6 py-3 font-medium text-red-700 transition-colors hover:bg-red-200 disabled:cursor-not-allowed disabled:opacity-50"
      >
        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
          />
        </svg>
        <span>Reset Configuration</span>
      </button>

      <div class="flex items-center space-x-4">
        <button
          onclick={loadConfig}
          disabled={isLoading}
          class="bg-background-200 text-foreground-700 hover:bg-background-300 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
        >
          {#if isLoading}
            <div
              class="border-foreground-700 h-4 w-4 animate-spin rounded-full border-2 border-t-transparent"
            ></div>
            <span>Loading...</span>
          {:else}
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
              />
            </svg>
            <span>Reload</span>
          {/if}
        </button>

        <button
          onclick={saveConfig}
          disabled={isLoading || !config.rootPath || !config.isValidPath}
          class="flex items-center space-x-2 rounded-lg bg-green-600 px-6 py-3 font-medium text-white transition-colors hover:bg-green-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {#if isLoading}
            <div
              class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
            ></div>
            <span>Saving...</span>
          {:else}
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M5 13l4 4L19 7"
              />
            </svg>
            <span>Save Configuration</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
</div>

<!-- Scan Results Modal -->
{#if showScanResult && scanResult}
  <div class="bg-opacity-50 fixed inset-0 z-50 flex items-center justify-center bg-black">
    <div
      class="bg-background-50 border-background-200 mx-4 max-h-[80vh] w-full max-w-2xl overflow-y-auto rounded-xl border shadow-xl"
    >
      <div class="border-background-200 flex items-center justify-between border-b p-6">
        <div class="flex items-center space-x-3">
          <div class="bg-accent-100 flex h-10 w-10 items-center justify-center rounded-lg">
            <svg
              class="text-accent-600 h-5 w-5"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
              />
            </svg>
          </div>
          <h3 class="text-foreground-900 text-xl font-semibold">Scan Results</h3>
        </div>
        <button
          onclick={() => (showScanResult = false)}
          class="text-foreground-400 hover:text-foreground-600 p-1"
        >
          <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>

      <div class="p-6">
        <!-- Summary Stats -->
        <div class="mb-6 grid grid-cols-1 gap-4 md:grid-cols-3">
          <div class="bg-accent-50 border-accent-200 rounded-lg border p-4 text-center">
            <div class="text-accent-600 text-2xl font-bold">{scanResult.foundProperties}</div>
            <div class="text-accent-800 text-sm font-medium">Properties Found</div>
          </div>
          <div class="rounded-lg border border-green-200 bg-green-50 p-4 text-center">
            <div class="text-2xl font-bold text-green-600">{scanResult.newProperties}</div>
            <div class="text-sm font-medium text-green-800">New Properties Added</div>
          </div>
          <div class="bg-background-100 border-background-200 rounded-lg border p-4 text-center">
            <div class="text-foreground-600 text-2xl font-bold">
              {scanResult.existingProperties}
            </div>
            <div class="text-foreground-800 text-sm font-medium">Already in Database</div>
          </div>
        </div>

        <!-- Success Message -->
        {#if scanResult.newProperties > 0}
          <div class="mb-4 rounded-lg border border-green-200 bg-green-50 p-4">
            <div class="flex items-center space-x-3">
              <svg
                class="h-5 w-5 flex-shrink-0 text-green-600"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              <p class="font-medium text-green-800">
                Successfully imported {scanResult.newProperties} new properties!
              </p>
            </div>
          </div>
        {/if}

        <!-- Errors -->
        {#if scanResult.errors.length > 0}
          <div class="mb-4 rounded-lg border border-orange-200 bg-orange-50 p-4">
            <div class="flex items-start space-x-3">
              <svg
                class="mt-0.5 h-5 w-5 flex-shrink-0 text-orange-600"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              <div>
                <h4 class="mb-2 font-medium text-orange-800">Issues Encountered:</h4>
                <div class="max-h-32 space-y-1 overflow-y-auto">
                  {#each scanResult.errors as error}
                    <div class="text-sm text-orange-700">• {error}</div>
                  {/each}
                </div>
              </div>
            </div>
          </div>
        {/if}

        <!-- No Properties Found -->
        {#if scanResult.foundProperties === 0}
          <div class="border-background-300 bg-background-100 mb-4 rounded-lg border p-4">
            <div class="flex items-center space-x-3">
              <svg
                class="text-foreground-600 h-5 w-5"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              <p class="text-foreground-800">
                No properties found in the folder structure. Make sure your properties are organized
                in the correct format.
              </p>
            </div>
          </div>
        {/if}

        <div class="flex justify-end">
          <button
            onclick={() => (showScanResult = false)}
            class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors"
          >
            <span>Close</span>
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
