<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { appDataDir } from '@tauri-apps/api/path';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { ScanResult, WatermarkConfig } from '$lib/types/database';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';

  // TypeScript interfaces
  interface AppConfig {
    rootPath?: string; // Legacy field for backward compatibility
    newFolderPath: string;
    doneFolderPath: string;
    notFoundFolderPath: string;
    archiveFolderPath: string;
    setsFolderPath: string;
    isValidPath: boolean;
    lastUpdated: string | null;
    fast_editor_path?: string;
    fast_editor_name?: string;
    complex_editor_path?: string;
    complex_editor_name?: string;
    watermark_image_path?: string;
    watermarkConfig: WatermarkConfig;
    watermark_opacity?: number;
  }

  interface CommandResult {
    success: boolean;
    error?: string;
  }

  // Reactive state for settings
  let config = $state<AppConfig>({
    newFolderPath: '',
    doneFolderPath: '',
    notFoundFolderPath: '',
    archiveFolderPath: '',
    setsFolderPath: '',
    isValidPath: false,
    lastUpdated: null,
    fast_editor_path: undefined,
    fast_editor_name: undefined,
    complex_editor_path: undefined,
    complex_editor_name: undefined,
    watermark_image_path: '',
    watermarkConfig: {
      sizeMode: 'proportional',
      sizePercentage: 0.35,
      relativeTo: 'longest-side',
      positionAnchor: 'center',
      offsetX: 0,
      offsetY: 0,
      opacity: 0.15,
      useAlphaChannel: true
    }
  });

  let isLoading = $state<boolean>(false);
  let statusMessage = $state<string>('');
  let statusType = $state<'success' | 'error' | 'info'>('info');

  // Watermark preview state
  let watermarkPreviewUrl = $state<string>('');
  let isGeneratingPreview = $state(false);

  // Confirmation dialog states
  let showResetConfigConfirm = $state(false);
  let showResetDatabaseConfirm = $state(false);

  // Repair status state
  let isRepairing = $state(false);
  let repairResult = $state<{ propertiesChecked: number; propertiesFixed: number; errors: string[] } | null>(null);
  let showRepairResult = $state(false);

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

  // Open folder dialog for each status folder
  async function selectNewFolder(): Promise<void> {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select folder for NEW properties'
      });

      if (selected && typeof selected === 'string') {
        config.newFolderPath = selected;
        validateFolderPaths();
      }
    } catch (error) {
      console.error('Error selecting folder:', error);
      statusMessage = `Error selecting folder: ${error}`;
      statusType = 'error';
    }
  }

  async function selectDoneFolder(): Promise<void> {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select folder for DONE properties'
      });

      if (selected && typeof selected === 'string') {
        config.doneFolderPath = selected;
        validateFolderPaths();
      }
    } catch (error) {
      console.error('Error selecting folder:', error);
      statusMessage = `Error selecting folder: ${error}`;
      statusType = 'error';
    }
  }

  async function selectNotFoundFolder(): Promise<void> {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select folder for NOT FOUND properties'
      });

      if (selected && typeof selected === 'string') {
        config.notFoundFolderPath = selected;
        validateFolderPaths();
      }
    } catch (error) {
      console.error('Error selecting folder:', error);
      statusMessage = `Error selecting folder: ${error}`;
      statusType = 'error';
    }
  }

  async function selectArchiveFolder(): Promise<void> {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select folder for ARCHIVED properties'
      });

      if (selected && typeof selected === 'string') {
        config.archiveFolderPath = selected;
        validateFolderPaths();
      }
    } catch (error) {
      console.error('Error selecting folder:', error);
      statusMessage = `Error selecting folder: ${error}`;
      statusType = 'error';
    }
  }

  async function selectSetsFolder(): Promise<void> {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select folder for Sets (ZIP archives)'
      });

      if (selected && typeof selected === 'string') {
        config.setsFolderPath = selected;
        statusMessage = 'Sets folder path updated';
        statusType = 'success';
      }
    } catch (error) {
      console.error('Error selecting folder:', error);
      statusMessage = `Error selecting folder: ${error}`;
      statusType = 'error';
    }
  }

  // Validate that all folder paths are set
  function validateFolderPaths(): void {
    config.isValidPath =
      config.newFolderPath !== '' &&
      config.doneFolderPath !== '' &&
      config.notFoundFolderPath !== '' &&
      config.archiveFolderPath !== '';

    if (config.isValidPath) {
      statusMessage = 'All folder paths configured';
      statusType = 'success';
    }
  }

  // Save configuration
  async function saveConfig(): Promise<void> {
    if (!config.isValidPath) {
      statusMessage = 'Please configure all 4 folder paths first';
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
          newFolderPath: config.newFolderPath,
          doneFolderPath: config.doneFolderPath,
          notFoundFolderPath: config.notFoundFolderPath,
          archiveFolderPath: config.archiveFolderPath,
          setsFolderPath: config.setsFolderPath,
          isValidPath: config.isValidPath,
          lastUpdated: new Date().toISOString(),
          fast_editor_path: config.fast_editor_path,
          fast_editor_name: config.fast_editor_name,
          complex_editor_path: config.complex_editor_path,
          complex_editor_name: config.complex_editor_name,
          watermark_image_path: config.watermark_image_path,
          watermarkConfig: config.watermarkConfig
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
  function resetConfig(): void {
    showResetConfigConfirm = true;
  }

  async function doResetConfig(): Promise<void> {
    showResetConfigConfirm = false;
    try {
      await invoke<CommandResult>('reset_config');
      config = {
        newFolderPath: '',
        doneFolderPath: '',
        notFoundFolderPath: '',
        archiveFolderPath: '',
        setsFolderPath: '',
        isValidPath: false,
        lastUpdated: null,
        watermarkConfig: {
          sizeMode: 'proportional',
          sizePercentage: 0.35,
          relativeTo: 'longest-side',
          positionAnchor: 'center',
          offsetX: 0,
          offsetY: 0,
          opacity: 0.15,
          useAlphaChannel: true
        }
      };
      statusMessage = 'Configuration reset successfully';
      statusType = 'success';
    } catch (error) {
      statusMessage = `Error resetting configuration: ${error}`;
      statusType = 'error';
    }
  }

  let isScanning = $state(false);
  let scanResult = $state<ScanResult | null>(null);
  let showScanResult = $state(false);

  async function scanAndImport() {
    // Check if at least one status folder is configured
    const hasFolderConfigured =
      config.newFolderPath ||
      config.doneFolderPath ||
      config.notFoundFolderPath ||
      config.archiveFolderPath;

    if (!hasFolderConfigured) {
      statusMessage = 'Please set up at least one status folder first';
      statusType = 'error';
      return;
    }

    try {
      isScanning = true;
      statusMessage = 'Scanning folders for existing properties...';
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

  function resetDatabase() {
    showResetDatabaseConfirm = true;
  }

  async function doResetDatabase() {
    showResetDatabaseConfirm = false;
    try {
      await invoke('reset_database_with_proper_dates');
      statusMessage = 'Database reset successfully';
      statusType = 'success';
    } catch (error) {
      statusMessage = `Reset failed: ${error}`;
      statusType = 'error';
    }
  }

  // Function to repair property statuses
  async function repairPropertyStatuses() {
    try {
      isRepairing = true;
      repairResult = null;
      const result = await DatabaseService.repairPropertyStatuses();
      if (result) {
        repairResult = result;
        showRepairResult = true;
        if (result.propertiesFixed > 0) {
          statusMessage = `Repaired ${result.propertiesFixed} properties`;
          statusType = 'success';
        } else {
          statusMessage = 'All properties are correctly synced';
          statusType = 'info';
        }
      }
    } catch (error) {
      statusMessage = `Repair failed: ${error}`;
      statusType = 'error';
    } finally {
      isRepairing = false;
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
        // Copy to app data for persistence
        await invoke('copy_watermark_to_app_data', { sourcePath: selected });

        // Get the app data path
        const appDataPath = await invoke<string | null>('get_watermark_from_app_data');
        if (appDataPath) {
          config.watermark_image_path = appDataPath;
          await generatePreview();
          statusMessage = 'Watermark image selected and saved';
          statusType = 'success';
        }
      }
    } catch (error) {
      console.error('Error selecting watermark image:', error);
      statusMessage = `Error selecting watermark image: ${error}`;
      statusType = 'error';
    }
  }

  // Generate watermark preview
  async function generatePreview(): Promise<void> {
    if (!config.watermark_image_path) {
      watermarkPreviewUrl = '';
      return;
    }

    try {
      isGeneratingPreview = true;
      const base64Preview = await invoke<string>('generate_watermark_preview', {
        sampleImageBase64: null // null uses default gray sample image
      });
      watermarkPreviewUrl = `data:image/png;base64,${base64Preview}`;
    } catch (error) {
      console.error('Failed to generate preview:', error);
      watermarkPreviewUrl = '';
    } finally {
      isGeneratingPreview = false;
    }
  }

  // Debounced preview generation
  let previewTimeout: number | null = null;
  function schedulePreviewUpdate() {
    if (previewTimeout) {
      clearTimeout(previewTimeout);
    }
    previewTimeout = window.setTimeout(() => {
      generatePreview();
    }, 500);
  }

  // Add function to clear watermark settings
  function clearWatermarkSettings(): void {
    config.watermark_image_path = '';
    config.watermarkConfig = {
      sizeMode: 'proportional',
      sizePercentage: 0.35,
      relativeTo: 'longest-side',
      positionAnchor: 'center',
      offsetX: 0,
      offsetY: 0,
      opacity: 0.15,
      useAlphaChannel: true
    };
    watermarkPreviewUrl = '';
    statusMessage = 'Watermark settings cleared';
    statusType = 'info';
  }
</script>

<div class="bg-background-0 min-h-full">
  <!-- Header -->
  <div class="bg-background-50 border-background-200 border-b">
    <div class="px-8 py-6">
      <div class="mx-auto max-w-4xl">
        <h1 class="text-foreground-900 text-2xl font-semibold">Settings</h1>
        <p class="text-foreground-600 mt-1 text-sm">Configure your workspace</p>
      </div>
    </div>
  </div>

  <div class="mx-auto max-w-4xl space-y-6 p-8">
    <!-- Status Message -->
    {#if statusMessage}
      <div
        class="rounded-lg border px-4 py-3 {statusType === 'success'
          ? 'border-green-300 bg-green-50'
          : statusType === 'error'
            ? 'border-red-300 bg-red-50'
            : 'border-accent-300 bg-accent-50'}"
      >
        <p
          class="text-sm {statusType === 'success'
            ? 'text-green-800'
            : statusType === 'error'
              ? 'text-red-800'
              : 'text-accent-800'}"
        >
          {statusMessage}
        </p>
      </div>
    {/if}

    <!-- Folder Configuration -->
    <div class="bg-background-50 border-background-200 border p-4">
      <div class="mb-4">
        <h2 class="text-foreground-900 text-lg font-semibold">Folder Configuration</h2>
        <p class="text-foreground-600 text-sm">Set up your photo storage directory</p>
      </div>

      <div class="space-y-6">
        <!-- NEW Folder Selection -->
        <div>
          <label class="text-foreground-700 mb-3 block text-sm font-medium">
            NEW Properties Folder
          </label>
          <div class="flex items-center space-x-4">
            <div class="min-w-0 flex-1">
              <input
                type="text"
                readonly
                value={config.newFolderPath || 'No folder selected'}
                class="text-foreground-900 border-background-300 bg-background-100 focus:ring-accent-500 focus:border-accent-500 w-full rounded-lg border px-4 py-3 focus:ring-2 focus:outline-none"
              />
            </div>
            <button
              onclick={selectNewFolder}
              disabled={isLoading}
              class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <span>Browse</span>
            </button>
          </div>
          <p class="text-foreground-500 mt-2 text-sm">
            Folder for new properties that need editing
          </p>
        </div>

        <!-- DONE Folder Selection -->
        <div>
          <label class="text-foreground-700 mb-3 block text-sm font-medium">
            DONE Properties Folder
          </label>
          <div class="flex items-center space-x-4">
            <div class="min-w-0 flex-1">
              <input
                type="text"
                readonly
                value={config.doneFolderPath || 'No folder selected'}
                class="text-foreground-900 border-background-300 bg-background-100 focus:ring-accent-500 focus:border-accent-500 w-full rounded-lg border px-4 py-3 focus:ring-2 focus:outline-none"
              />
            </div>
            <button
              onclick={selectDoneFolder}
              disabled={isLoading}
              class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <span>Browse</span>
            </button>
          </div>
          <p class="text-foreground-500 mt-2 text-sm">
            Folder for properties completed this week, waiting to be sent
          </p>
        </div>

        <!-- NOT FOUND Folder Selection -->
        <div>
          <label class="text-foreground-700 mb-3 block text-sm font-medium">
            NOT FOUND Properties Folder
          </label>
          <div class="flex items-center space-x-4">
            <div class="min-w-0 flex-1">
              <input
                type="text"
                readonly
                value={config.notFoundFolderPath || 'No folder selected'}
                class="text-foreground-900 border-background-300 bg-background-100 focus:ring-accent-500 focus:border-accent-500 w-full rounded-lg border px-4 py-3 focus:ring-2 focus:outline-none"
              />
            </div>
            <button
              onclick={selectNotFoundFolder}
              disabled={isLoading}
              class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <span>Browse</span>
            </button>
          </div>
          <p class="text-foreground-500 mt-2 text-sm">
            Folder for properties where listing doesn't exist on website yet
          </p>
        </div>

        <!-- ARCHIVE Folder Selection -->
        <div>
          <label class="text-foreground-700 mb-3 block text-sm font-medium">
            ARCHIVE Properties Folder
          </label>
          <div class="flex items-center space-x-4">
            <div class="min-w-0 flex-1">
              <input
                type="text"
                readonly
                value={config.archiveFolderPath || 'No folder selected'}
                class="text-foreground-900 border-background-300 bg-background-100 focus:ring-accent-500 focus:border-accent-500 w-full rounded-lg border px-4 py-3 focus:ring-2 focus:outline-none"
              />
            </div>
            <button
              onclick={selectArchiveFolder}
              disabled={isLoading}
              class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <span>Browse</span>
            </button>
          </div>
          <p class="text-foreground-500 mt-2 text-sm">
            Folder for properties that are done, uploaded, and sent to boss
          </p>
        </div>

        <!-- Sets Folder Selection -->
        <div>
          <label class="text-foreground-700 mb-3 block text-sm font-medium">
            Sets Folder (ZIP Archives)
          </label>
          <div class="flex items-center space-x-4">
            <div class="min-w-0 flex-1">
              <input
                type="text"
                readonly
                value={config.setsFolderPath || 'No folder selected'}
                class="text-foreground-900 border-background-300 bg-background-100 focus:ring-accent-500 focus:border-accent-500 w-full rounded-lg border px-4 py-3 focus:ring-2 focus:outline-none"
              />
            </div>
            <button
              onclick={selectSetsFolder}
              disabled={isLoading}
              class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <span>Browse</span>
            </button>
          </div>
          <p class="text-foreground-500 mt-2 text-sm">
            Folder where completed property sets will be saved as ZIP files
          </p>
        </div>

        <!-- Status Indicators -->
        <div class="bg-background-100 flex items-center space-x-6 rounded-lg p-4">
          <div class="flex items-center space-x-2">
            <div
              class="h-3 w-3 rounded-full {config.isValidPath
                ? 'bg-green-500'
                : 'bg-background-300'}"
            ></div>
            <span class="text-foreground-600 text-sm font-medium">All Folders Configured</span>
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
      <div class="bg-background-50 border-background-200 border p-4">
        <div class="mb-4">
          <h2 class="text-foreground-900 text-lg font-semibold">Configuration Details</h2>
          <p class="text-foreground-600 text-sm">Current application settings</p>
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
      <div class="bg-background-50 border-background-200 border p-4">
        <div class="mb-4">
          <h2 class="text-foreground-900 text-lg font-semibold">Import Existing Properties</h2>
          <p class="text-foreground-600 text-sm">
            Scan your folder for existing properties and add them to the database
          </p>
        </div>

        <button
          onclick={scanAndImport}
          disabled={isScanning}
          class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-3 px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
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
    <div class="bg-background-50 border-background-200 border p-4">
      <div class="mb-4">
        <h2 class="text-foreground-900 text-lg font-semibold">Image Editor Configuration</h2>
        <p class="text-foreground-600 text-sm">Set up your preferred image editing applications</p>
      </div>

      <div class="space-y-5">
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

    <!-- Enhanced Watermark Configuration -->
    <div class="bg-background-50 border-background-200 border p-4">
      <div class="mb-4">
        <h2 class="text-foreground-900 text-lg font-semibold">Watermark Configuration</h2>
        <p class="text-foreground-600 text-sm">
          Configure watermark image, size, position, and opacity
        </p>
      </div>

      <div class="space-y-5">
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
            PNG with transparent background recommended. Image will be stored in app data.
          </p>
        </div>

        {#if config.watermark_image_path}
          <!-- Size Configuration -->
          <div class="border-background-200 rounded-lg border p-4">
            <h3 class="text-foreground-900 mb-4 font-medium">Size</h3>

            <!-- Size Mode Radio Buttons -->
            <div class="mb-4 grid grid-cols-2 gap-3 md:grid-cols-4">
              <label class="flex cursor-pointer items-center space-x-2">
                <input
                  type="radio"
                  name="sizeMode"
                  value="proportional"
                  bind:group={config.watermarkConfig.sizeMode}
                  onchange={schedulePreviewUpdate}
                  class="text-accent-600 focus:ring-accent-500 h-4 w-4"
                />
                <span class="text-foreground-700 text-sm">Proportional</span>
              </label>
              <label class="flex cursor-pointer items-center space-x-2">
                <input
                  type="radio"
                  name="sizeMode"
                  value="fit"
                  bind:group={config.watermarkConfig.sizeMode}
                  onchange={schedulePreviewUpdate}
                  class="text-accent-600 focus:ring-accent-500 h-4 w-4"
                />
                <span class="text-foreground-700 text-sm">Fit</span>
              </label>
              <label class="flex cursor-pointer items-center space-x-2">
                <input
                  type="radio"
                  name="sizeMode"
                  value="stretch"
                  bind:group={config.watermarkConfig.sizeMode}
                  onchange={schedulePreviewUpdate}
                  class="text-accent-600 focus:ring-accent-500 h-4 w-4"
                />
                <span class="text-foreground-700 text-sm">Stretch</span>
              </label>
              <label class="flex cursor-pointer items-center space-x-2">
                <input
                  type="radio"
                  name="sizeMode"
                  value="tile"
                  bind:group={config.watermarkConfig.sizeMode}
                  onchange={schedulePreviewUpdate}
                  class="text-accent-600 focus:ring-accent-500 h-4 w-4"
                />
                <span class="text-foreground-700 text-sm">Tile</span>
              </label>
            </div>

            <!-- Proportional Settings -->
            {#if config.watermarkConfig.sizeMode === 'proportional'}
              <div class="space-y-4">
                <div>
                  <label class="text-foreground-700 mb-2 block text-sm font-medium">
                    Size: {Math.round(config.watermarkConfig.sizePercentage * 100)}%
                  </label>
                  <input
                    type="range"
                    min="0.05"
                    max="1"
                    step="0.05"
                    bind:value={config.watermarkConfig.sizePercentage}
                    oninput={schedulePreviewUpdate}
                    class="bg-accent-200 h-2 w-full cursor-pointer appearance-none rounded-lg"
                  />
                </div>
                <div>
                  <label class="text-foreground-700 mb-2 block text-sm">Relative to</label>
                  <select
                    bind:value={config.watermarkConfig.relativeTo}
                    onchange={schedulePreviewUpdate}
                    class="text-foreground-900 border-background-300 bg-background-100 w-full rounded-lg border px-3 py-2 focus:outline-none"
                  >
                    <option value="longest-side">Longest side</option>
                    <option value="shortest-side">Shortest side</option>
                    <option value="width">Width</option>
                    <option value="height">Height</option>
                  </select>
                </div>
              </div>
            {/if}
          </div>

          <!-- Position Configuration -->
          <div class="border-background-200 rounded-lg border p-4">
            <h3 class="text-foreground-900 mb-4 font-medium">Position</h3>

            <!-- Anchor Selection (9-point grid) -->
            <div class="mb-4">
              <label class="text-foreground-700 mb-3 block text-sm">Anchor</label>
              <div class="grid grid-cols-3 gap-2">
                {#each [['top-left', 'TL'], ['top-center', 'TC'], ['top-right', 'TR'], ['center-left', 'CL'], ['center', 'C'], ['center-right', 'CR'], ['bottom-left', 'BL'], ['bottom-center', 'BC'], ['bottom-right', 'BR']] as [value, label]}
                  <button
                    type="button"
                    onclick={() => {
                      config.watermarkConfig.positionAnchor = value as
                        | 'top-left'
                        | 'top-center'
                        | 'top-right'
                        | 'center-left'
                        | 'center'
                        | 'center-right'
                        | 'bottom-left'
                        | 'bottom-center'
                        | 'bottom-right';
                      schedulePreviewUpdate();
                    }}
                    class="border-background-300 hover:bg-accent-100 flex h-12 items-center justify-center rounded-lg border text-sm font-medium transition-colors {config
                      .watermarkConfig.positionAnchor === value
                      ? 'bg-accent-500 text-white'
                      : 'bg-background-50 text-foreground-700'}"
                  >
                    {label}
                  </button>
                {/each}
              </div>
            </div>

            <!-- Offset Controls -->
            <div class="grid grid-cols-2 gap-4">
              <div>
                <label class="text-foreground-700 mb-2 block text-sm">Offset X</label>
                <input
                  type="number"
                  bind:value={config.watermarkConfig.offsetX}
                  oninput={schedulePreviewUpdate}
                  class="text-foreground-900 border-background-300 bg-background-100 w-full rounded-lg border px-3 py-2 focus:outline-none"
                  placeholder="0"
                />
              </div>
              <div>
                <label class="text-foreground-700 mb-2 block text-sm">Offset Y</label>
                <input
                  type="number"
                  bind:value={config.watermarkConfig.offsetY}
                  oninput={schedulePreviewUpdate}
                  class="text-foreground-900 border-background-300 bg-background-100 w-full rounded-lg border px-3 py-2 focus:outline-none"
                  placeholder="0"
                />
              </div>
            </div>
          </div>

          <!-- Opacity Configuration -->
          <div class="border-background-200 rounded-lg border p-4">
            <h3 class="text-foreground-900 mb-4 font-medium">Opacity</h3>

            <div class="mb-4">
              <label class="text-foreground-700 mb-3 block text-sm font-medium">
                Opacity: {Math.round(config.watermarkConfig.opacity * 100)}%
              </label>
              <div class="flex items-center space-x-4">
                <span class="text-foreground-500 text-sm font-medium">0%</span>
                <div class="flex-1">
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.05"
                    bind:value={config.watermarkConfig.opacity}
                    oninput={schedulePreviewUpdate}
                    class="bg-accent-200 h-2 w-full cursor-pointer appearance-none rounded-lg"
                  />
                </div>
                <span class="text-foreground-500 text-sm font-medium">100%</span>
              </div>
            </div>

            <label class="flex cursor-pointer items-center space-x-2">
              <input
                type="checkbox"
                bind:checked={config.watermarkConfig.useAlphaChannel}
                onchange={schedulePreviewUpdate}
                class="text-accent-600 focus:ring-accent-500 h-4 w-4 rounded"
              />
              <span class="text-foreground-700 text-sm"
                >Use alpha channel (respect PNG transparency)</span
              >
            </label>
          </div>

          <!-- Live Preview -->
          <div class="border-background-300 bg-background-100 rounded-lg border p-4">
            <div class="mb-3 flex items-center justify-between">
              <div class="flex items-center space-x-2">
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
                <h4 class="text-foreground-900 font-medium">Live Preview</h4>
              </div>
              {#if isGeneratingPreview}
                <div
                  class="border-accent-600 h-4 w-4 animate-spin rounded-full border-2 border-t-transparent"
                ></div>
              {/if}
            </div>

            {#if watermarkPreviewUrl}
              <div class="border-background-300 bg-background-50 overflow-hidden rounded-lg border">
                <img src={watermarkPreviewUrl} alt="Watermark preview" class="h-auto w-full" />
              </div>
              <p class="text-foreground-500 mt-2 text-xs">
                Preview shows how watermark will appear on images
              </p>
            {:else if isGeneratingPreview}
              <div class="flex h-48 items-center justify-center">
                <div class="text-foreground-500 text-sm">Generating preview...</div>
              </div>
            {:else}
              <div class="flex h-48 items-center justify-center">
                <div class="text-foreground-500 text-sm">Configure settings to see preview</div>
              </div>
            {/if}
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
            <span class="text-foreground-600 text-sm font-medium">Watermark Configured</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Database Repair Section -->
    {#if config.isValidPath}
      <div class="border-background-200 bg-background-50 rounded-xl border p-6">
        <div class="mb-4 flex items-center space-x-3">
          <div class="bg-accent-100 flex h-10 w-10 items-center justify-center rounded-lg">
            <svg class="text-accent-600 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
              />
            </svg>
          </div>
          <div>
            <h2 class="text-foreground-900 text-xl font-semibold">Repair Database</h2>
            <p class="text-foreground-600 text-sm">Sync property statuses with actual folder locations</p>
          </div>
        </div>

        <p class="text-foreground-600 mb-4 text-sm">
          If properties aren't showing correctly, this will check each property's actual folder location
          and update the database status to match. Use this if properties were moved manually or after
          a failed operation.
        </p>

        <div class="flex items-center space-x-4">
          <button
            onclick={repairPropertyStatuses}
            disabled={isRepairing}
            class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-4 py-2 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            {#if isRepairing}
              <div class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"></div>
              <span>Repairing...</span>
            {:else}
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                />
              </svg>
              <span>Repair Property Statuses</span>
            {/if}
          </button>
        </div>
      </div>
    {/if}

    <!-- Database Management Section -->
    {#if config.isValidPath}
      <div class="rounded-xl border border-red-200 bg-red-50 p-6">
        <div class="mb-4 flex items-center space-x-3">
          <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-red-100">
            <svg class="h-5 w-5 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
              />
            </svg>
          </div>
          <div>
            <h2 class="text-xl font-semibold text-red-900">Database Management</h2>
            <p class="text-sm text-red-700">Clear all properties to start fresh</p>
          </div>
        </div>

        <div class="flex items-center space-x-4">
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
            <span>Clear Database</span>
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
          disabled={isLoading || !config.isValidPath}
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
      class="bg-background-50 border-background-200 mx-4 max-h-[80vh] w-full max-w-2xl overflow-y-auto border"
    >
      <div class="border-background-200 flex items-center justify-between border-b p-4">
        <h3 class="text-foreground-900 text-lg font-semibold">Scan Results</h3>
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

      <div class="p-4">
        <!-- Summary Stats -->
        <div class="mb-4 grid grid-cols-1 gap-4 md:grid-cols-3">
          <div class="bg-background-100 border-background-200 border p-4 text-center">
            <div class="text-foreground-900 text-2xl font-semibold">
              {scanResult.foundProperties}
            </div>
            <div class="text-foreground-600 text-sm font-medium">Properties Found</div>
          </div>
          <div class="bg-background-100 border-background-200 border p-4 text-center">
            <div class="text-foreground-900 text-2xl font-semibold">{scanResult.newProperties}</div>
            <div class="text-foreground-600 text-sm font-medium">New Properties Added</div>
          </div>
          <div class="bg-background-100 border-background-200 border p-4 text-center">
            <div class="text-foreground-900 text-2xl font-semibold">
              {scanResult.existingProperties}
            </div>
            <div class="text-foreground-600 text-sm font-medium">Already in Database</div>
          </div>
        </div>

        <!-- Success Message -->
        {#if scanResult.newProperties > 0}
          <div class="bg-background-100 border-background-200 mb-4 border p-4">
            <div class="flex items-center space-x-3">
              <svg
                class="text-foreground-700 h-5 w-5 flex-shrink-0"
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
              <p class="text-foreground-900 font-medium">
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

<!-- Reset Configuration Confirmation Dialog -->
<ConfirmDialog
  bind:open={showResetConfigConfirm}
  title="Reset Configuration"
  message="Are you sure you want to reset the configuration? This will clear all settings."
  confirmText="Reset"
  destructive={true}
  onConfirm={doResetConfig}
  onCancel={() => (showResetConfigConfirm = false)}
/>

<!-- Clear Database Confirmation Dialog -->
<ConfirmDialog
  bind:open={showResetDatabaseConfirm}
  title="Clear Database"
  message="This will delete all properties from the database. Your folders will NOT be deleted. Are you sure?"
  confirmText="Clear Database"
  destructive={true}
  onConfirm={doResetDatabase}
  onCancel={() => (showResetDatabaseConfirm = false)}
/>

<!-- Repair Result Modal -->
{#if showRepairResult && repairResult}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    onclick={() => (showRepairResult = false)}
    onkeydown={(e) => e.key === 'Escape' && (showRepairResult = false)}
    role="dialog"
    tabindex="-1"
  >
    <div
      class="bg-background-0 mx-4 max-h-[80vh] w-full max-w-md overflow-y-auto rounded-lg p-6 shadow-xl"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
      role="document"
    >
      <div class="mb-4 flex items-center justify-between">
        <h3 class="text-foreground-900 text-lg font-semibold">Repair Results</h3>
        <button
          onclick={() => (showRepairResult = false)}
          class="text-foreground-400 hover:text-foreground-600 p-1"
        >
          <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <div class="space-y-4">
        <!-- Stats -->
        <div class="bg-background-50 border-background-200 grid grid-cols-2 gap-4 rounded-lg border p-4">
          <div class="text-center">
            <p class="text-foreground-500 text-xs">Checked</p>
            <p class="text-foreground-900 text-2xl font-bold">{repairResult.propertiesChecked}</p>
          </div>
          <div class="text-center">
            <p class="text-foreground-500 text-xs">Fixed</p>
            <p class="text-2xl font-bold {repairResult.propertiesFixed > 0 ? 'text-green-600' : 'text-foreground-900'}">
              {repairResult.propertiesFixed}
            </p>
          </div>
        </div>

        <!-- Status Message -->
        {#if repairResult.propertiesFixed > 0}
          <div class="flex items-start gap-2 rounded-lg border border-green-200 bg-green-50 p-3">
            <svg class="mt-0.5 h-4 w-4 flex-shrink-0 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
            <p class="text-sm text-green-700">
              Successfully repaired {repairResult.propertiesFixed} {repairResult.propertiesFixed === 1 ? 'property' : 'properties'}.
              The database is now synced with the actual folder locations.
            </p>
          </div>
        {:else}
          <div class="bg-background-50 border-background-200 flex items-start gap-2 rounded-lg border p-3">
            <svg class="text-foreground-400 mt-0.5 h-4 w-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
            <p class="text-foreground-600 text-sm">
              All properties are correctly synced. No repairs needed.
            </p>
          </div>
        {/if}

        <!-- Errors (if any) -->
        {#if repairResult.errors.length > 0}
          <div class="rounded-lg border border-amber-200 bg-amber-50 p-3">
            <p class="mb-2 text-sm font-medium text-amber-800">Warnings ({repairResult.errors.length})</p>
            <div class="max-h-32 overflow-y-auto">
              {#each repairResult.errors as error}
                <p class="text-xs text-amber-700">{error}</p>
              {/each}
            </div>
          </div>
        {/if}
      </div>

      <div class="mt-6 flex justify-end">
        <button
          onclick={() => (showRepairResult = false)}
          class="bg-accent-500 hover:bg-accent-600 rounded-lg px-6 py-2 font-medium text-white transition-colors"
        >
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
