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
    fast_editor_path?: string; // Path to fast editor executable
    fast_editor_name?: string; // Display name for fast editor
    complex_editor_path?: string; // Path to complex editor executable
    complex_editor_name?: string; // Display name for complex editor
    watermark_image_path?: string; // Add this
    watermark_opacity?: number; // Add this
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
    fast_editor_path: '',
    fast_editor_name: '',
    complex_editor_path: '',
    complex_editor_name: '',
    watermark_image_path: '', // Add this
    watermark_opacity: 0.15 // Add this - 30% default
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
        // Extract application name from path
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
        // Extract application name from path
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

<div class="mx-auto max-w-4xl space-y-8">
  <!-- Page Header -->
  <div class="bg-background-100 rounded-lg border border-gray-200 p-6 shadow-sm">
    <h1 class="text-foreground-900 mb-2 text-2xl font-bold">Settings</h1>
    <p class="text-foreground-600">Configure your photo management workspace</p>
  </div>

  <!-- Status Message -->
  {#if statusMessage}
    <div
      class="rounded-lg p-4 {statusType === 'success'
        ? 'border border-green-200 bg-green-50'
        : statusType === 'error'
          ? 'border border-red-200 bg-red-50'
          : 'border border-blue-200 bg-blue-50'}"
    >
      <div class="flex items-center space-x-3">
        <span class="text-2xl">
          {statusType === 'success' ? '✅' : statusType === 'error' ? '❌' : 'ℹ️'}
        </span>
        <p
          class="text-sm font-medium {statusType === 'success'
            ? 'text-green-800'
            : statusType === 'error'
              ? 'text-red-800'
              : 'text-blue-800'}"
        >
          {statusMessage}
        </p>
      </div>
    </div>
  {/if}

  <!-- Folder Configuration -->
  <div class="bg-background-100 rounded-lg border border-gray-200 p-6 shadow-sm">
    <h2 class="text-foreground-900 mb-4 text-xl font-semibold">Folder Configuration</h2>

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
              class="text-foreground-900 w-full rounded-lg border border-gray-300 bg-gray-50 px-4 py-3 focus:outline-none"
              placeholder="Select a folder to store your photos"
            />
          </div>
          <button
            onclick={selectFolder}
            disabled={isLoading}
            class="btn-primary flex items-center space-x-2 px-6 py-3 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <span>📁</span>
            <span>Browse</span>
          </button>
        </div>
        <p class="text-foreground-500 mt-2 text-sm">
          This folder will contain all your photo projects and processed images
        </p>
      </div>

      <!-- Folder Structure Preview -->
      <!-- Folder Structure Preview -->
      {#if config.rootPath}
        <div class="rounded-lg border border-gray-200 bg-gray-50 p-4">
          <h3 class="text-foreground-900 mb-3 font-medium">Folder Structure Preview</h3>
          <div class="text-foreground-700 space-y-1 font-mono text-sm">
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
      <div class="flex items-center space-x-6">
        <div class="flex items-center space-x-2">
          <div
            class="h-3 w-3 rounded-full {config.rootPath ? 'bg-green-500' : 'bg-gray-300'}"
          ></div>
          <span class="text-foreground-600 text-sm">Folder Selected</span>
        </div>
        <div class="flex items-center space-x-2">
          <div
            class="h-3 w-3 rounded-full {config.isValidPath ? 'bg-green-500' : 'bg-gray-300'}"
          ></div>
          <span class="text-foreground-600 text-sm">Structure Created</span>
        </div>
        <div class="flex items-center space-x-2">
          <div
            class="h-3 w-3 rounded-full {config.lastUpdated ? 'bg-green-500' : 'bg-gray-300'}"
          ></div>
          <span class="text-foreground-600 text-sm">Configuration Saved</span>
        </div>
      </div>
    </div>
  </div>

  <!-- Configuration Details -->
  {#if config.lastUpdated}
    <div class="bg-background-100 rounded-lg border border-gray-200 p-6 shadow-sm">
      <h2 class="text-foreground-900 mb-4 text-xl font-semibold">Configuration Details</h2>
      <div class="space-y-3">
        <div class="flex items-center justify-between">
          <span class="text-foreground-600 text-sm">Last Updated:</span>
          <span class="text-foreground-900 text-sm">
            {new Date(config.lastUpdated).toLocaleString()}
          </span>
        </div>
        <div class="flex items-center justify-between">
          <span class="text-foreground-600 text-sm">Root Path:</span>
          <span class="text-foreground-900 font-mono text-sm">{config.rootPath}</span>
        </div>
        <div class="flex items-center justify-between">
          <span class="text-foreground-600 text-sm">Fast Editor:</span>
          <span class="text-foreground-900 text-sm"
            >{config.fast_editor_name || 'System Default'}</span
          >
        </div>
        <div class="flex items-center justify-between">
          <span class="text-foreground-600 text-sm">Complex Editor:</span>
          <span class="text-foreground-900 text-sm"
            >{config.complex_editor_name || 'System Default'}</span
          >
        </div>
        <div class="flex items-center justify-between">
          <span class="text-foreground-600 text-sm">Status:</span>
          <span class="text-sm {config.isValidPath ? 'text-green-600' : 'text-red-600'}">
            {config.isValidPath ? 'Ready' : 'Not Ready'}
          </span>
        </div>
      </div>
    </div>
  {/if}

  {#if config.isValidPath}
    <div class="bg-background-100 rounded-lg border border-gray-200 p-6 shadow-sm">
      <h2 class="text-foreground-900 mb-4 text-xl font-semibold">Import Existing Properties</h2>
      <p class="text-foreground-600 mb-4">
        Scan your selected folder for existing properties and add them to the database.
      </p>

      <button
        onclick={scanAndImport}
        disabled={isScanning}
        class="btn-primary flex items-center space-x-2 disabled:cursor-not-allowed disabled:opacity-50"
      >
        <span>{isScanning ? '🔍' : '📋'}</span>
        <span>{isScanning ? 'Scanning...' : 'Scan & Import Properties'}</span>
      </button>
    </div>
  {/if}

  <!-- Scan Results Modal -->
  {#if showScanResult && scanResult}
    <div class="bg-opacity-50 fixed inset-0 z-50 flex items-center justify-center bg-black">
      <div
        class="bg-background-100 mx-4 max-h-[80vh] w-full max-w-2xl overflow-y-auto rounded-lg p-6"
      >
        <div class="mb-6 flex items-center justify-between">
          <h3 class="text-foreground-900 text-xl font-semibold">Scan Results</h3>
          <button
            onclick={() => (showScanResult = false)}
            class="text-foreground-400 hover:text-foreground-600"
          >
            ✕
          </button>
        </div>

        <!-- Summary Stats -->
        <div class="mb-6 grid grid-cols-1 gap-4 md:grid-cols-3">
          <div class="rounded-lg bg-blue-50 p-4 text-center">
            <div class="text-2xl font-bold text-blue-600">{scanResult.foundProperties}</div>
            <div class="text-sm text-blue-800">Properties Found</div>
          </div>
          <div class="rounded-lg bg-green-50 p-4 text-center">
            <div class="text-2xl font-bold text-green-600">{scanResult.newProperties}</div>
            <div class="text-sm text-green-800">New Properties Added</div>
          </div>
          <div class="rounded-lg bg-gray-50 p-4 text-center">
            <div class="text-foreground-600 text-2xl font-bold">
              {scanResult.existingProperties}
            </div>
            <div class="text-foreground-800 text-sm">Already in Database</div>
          </div>
        </div>

        <!-- Success Message -->
        {#if scanResult.newProperties > 0}
          <div class="mb-4 rounded-lg border border-green-200 bg-green-50 p-4">
            <div class="flex items-center space-x-2">
              <span class="text-green-600">✅</span>
              <p class="font-medium text-green-800">
                Successfully imported {scanResult.newProperties} new properties!
              </p>
            </div>
          </div>
        {/if}

        <!-- Errors -->
        {#if scanResult.errors.length > 0}
          <div class="mb-4 rounded-lg border border-yellow-200 bg-yellow-50 p-4">
            <h4 class="mb-2 font-medium text-yellow-800">Issues Encountered:</h4>
            <div class="max-h-32 space-y-1 overflow-y-auto">
              {#each scanResult.errors as error}
                <div class="text-sm text-yellow-700">• {error}</div>
              {/each}
            </div>
          </div>
        {/if}

        <!-- No Properties Found -->
        {#if scanResult.foundProperties === 0}
          <div class="mb-4 rounded-lg border border-gray-200 bg-gray-50 p-4">
            <div class="flex items-center space-x-2">
              <span class="text-foreground-600">ℹ️</span>
              <p class="text-foreground-800">
                No properties found in the folder structure. Make sure your properties are organized
                in the correct format.
              </p>
            </div>
          </div>
        {/if}

        <div class="flex justify-end">
          <button onclick={() => (showScanResult = false)} class="btn-primary"> Close </button>
        </div>
      </div>
    </div>
  {/if}

  {#if config.isValidPath}
    <div class="rounded-lg border border-yellow-200 bg-yellow-50 p-6 shadow-sm">
      <h2 class="text-foreground-900 mb-4 text-xl font-semibold">Debug Database (Temporary)</h2>
      <div class="space-y-4">
        <button onclick={debugDatabase} class="btn-secondary"> Debug Database Dates </button>

        <button
          onclick={resetDatabase}
          class="rounded-lg bg-red-600 px-4 py-2 font-medium text-white transition-colors hover:bg-red-700"
        >
          Reset Database (This will delete all data!)
        </button>
      </div>
    </div>
  {/if}

  <!-- Image Editor Configuration -->
  <div class="bg-background-100 rounded-lg border border-gray-200 p-6 shadow-sm">
    <h2 class="text-foreground-900 mb-4 text-xl font-semibold">Image Editor Configuration</h2>

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
              class="text-foreground-900 w-full rounded-lg border border-gray-300 bg-gray-50 px-4 py-3 focus:outline-none"
              placeholder="No custom editor selected"
            />
            {#if config.fast_editor_path}
              <p class="text-foreground-500 mt-1 font-mono text-xs">{config.fast_editor_path}</p>
            {/if}
          </div>
          <button
            onclick={selectFastEditor}
            disabled={isLoading}
            class="btn-secondary flex items-center space-x-2 px-4 py-3 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <span>🖼️</span>
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
              class="text-foreground-900 w-full rounded-lg border border-gray-300 bg-gray-50 px-4 py-3 focus:outline-none"
              placeholder="No custom editor selected"
            />
            {#if config.complex_editor_path}
              <p class="text-foreground-500 mt-1 font-mono text-xs">{config.complex_editor_path}</p>
            {/if}
          </div>
          <button
            onclick={selectComplexEditor}
            disabled={isLoading}
            class="btn-secondary flex items-center space-x-2 px-4 py-3 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <span>🎨</span>
            <span>Browse</span>
          </button>
        </div>
        <p class="text-foreground-500 mt-2 text-sm">
          Recommended: Adobe Photoshop, GIMP, Paint.NET, Photopea
        </p>
      </div>

      <!-- Editor Actions -->
      <div class="flex items-center justify-between border-t border-gray-200 pt-4">
        <button onclick={resetEditors} class="text-sm font-medium text-red-600 hover:text-red-700">
          Clear Editor Selections
        </button>

        <div class="text-foreground-600 flex items-center space-x-2 text-sm">
          <div
            class="h-3 w-3 rounded-full {config.fast_editor_path ? 'bg-green-500' : 'bg-gray-300'}"
          ></div>
          <span>Fast Editor</span>
          <div
            class="h-3 w-3 rounded-full {config.complex_editor_path
              ? 'bg-green-500'
              : 'bg-gray-300'} ml-4"
          ></div>
          <span>Complex Editor</span>
        </div>
      </div>
    </div>
  </div>

  <!-- Watermark Configuration -->
  <div class="bg-background-100 rounded-lg border border-gray-200 p-6 shadow-sm">
    <h2 class="text-foreground-900 mb-4 text-xl font-semibold">Watermark Configuration</h2>

    <div class="space-y-6">
      <!-- Watermark Image Selection -->
      <div>
        <label class="text-foreground-700 mb-3 block text-sm font-medium"> Watermark Image </label>
        <div class="flex items-center space-x-4">
          <div class="min-w-0 flex-1">
            <input
              type="text"
              readonly
              value={config.watermark_image_path || 'No watermark image selected'}
              class="text-foreground-900 w-full rounded-lg border border-gray-300 bg-gray-50 px-4 py-3 focus:outline-none"
              placeholder="Select a watermark image"
            />
            {#if config.watermark_image_path}
              <p class="text-foreground-500 mt-1 font-mono text-xs">
                {config.watermark_image_path}
              </p>
            {/if}
          </div>
          <button
            onclick={selectWatermarkImage}
            disabled={isLoading}
            class="btn-secondary flex items-center space-x-2 px-4 py-3 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <span>🏷️</span>
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
          <span class="text-foreground-500 text-sm">0%</span>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            bind:value={config.watermark_opacity}
            class="slider h-2 flex-1 cursor-pointer appearance-none rounded-lg bg-gray-200"
          />
          <span class="text-foreground-500 text-sm">100%</span>
        </div>
        <p class="text-foreground-500 mt-2 text-sm">
          Adjust the transparency of the watermark overlay
        </p>
      </div>

      <!-- Preview Section -->
      {#if config.watermark_image_path}
        <div class="rounded-lg border border-gray-200 bg-gray-50 p-4">
          <h4 class="text-foreground-900 mb-2 font-medium">Watermark Preview</h4>
          <div class="flex items-center space-x-4">
            <div
              class="bg-background-100 flex h-16 w-16 items-center justify-center overflow-hidden rounded border border-gray-300"
            >
              <img
                src={`file://${config.watermark_image_path}`}
                alt="Watermark preview"
                class="max-h-full max-w-full object-contain"
                style="opacity: {config.watermark_opacity}"
                onerror={() => {}}
              />
            </div>
            <div class="text-foreground-600 text-sm">
              <p>Opacity: {Math.round(config.watermark_opacity * 100)}%</p>
              <p>Position: Center</p>
              <p>Size: Auto-fitted to image</p>
            </div>
          </div>
        </div>
      {/if}

      <!-- Watermark Actions -->
      <div class="flex items-center justify-between border-t border-gray-200 pt-4">
        <button
          onclick={clearWatermarkSettings}
          class="text-sm font-medium text-red-600 hover:text-red-700"
        >
          Clear Watermark Settings
        </button>

        <div class="text-foreground-600 flex items-center space-x-2 text-sm">
          <div
            class="h-3 w-3 rounded-full {config.watermark_image_path
              ? 'bg-green-500'
              : 'bg-gray-300'}"
          ></div>
          <span>Watermark Image</span>
        </div>
      </div>
    </div>
  </div>

  <!-- Action Buttons -->
  <div class="flex items-center justify-between">
    <button
      onclick={resetConfig}
      disabled={isLoading}
      class="rounded-lg bg-red-100 px-6 py-3 font-medium text-red-700 transition-colors hover:bg-red-200 disabled:cursor-not-allowed disabled:opacity-50"
    >
      Reset Configuration
    </button>

    <div class="flex items-center space-x-4">
      <button
        onclick={loadConfig}
        disabled={isLoading}
        class="btn-secondary disabled:cursor-not-allowed disabled:opacity-50"
      >
        {isLoading ? 'Loading...' : 'Reload'}
      </button>

      <button
        onclick={saveConfig}
        disabled={isLoading || !config.rootPath || !config.isValidPath}
        class="cursor-pointer rounded-lg bg-green-100 px-6 py-3 font-medium text-green-700 transition-colors hover:bg-green-200 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {isLoading ? 'Saving...' : 'Save Configuration'}
      </button>
    </div>
  </div>
</div>
