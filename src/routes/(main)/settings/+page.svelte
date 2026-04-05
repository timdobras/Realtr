<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { ScanResult, WatermarkConfig } from '$lib/types/database';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { showSuccess, showError, showInfo } from '$lib/stores/notification';

  // TypeScript interfaces
  interface AppConfig {
    rootPath?: string;
    newFolderPath: string;
    doneFolderPath: string;
    notFoundFolderPath: string;
    archiveFolderPath: string;
    setsFolderPath: string;
    isValidPath: boolean;
    lastUpdated: string | null;
    use_builtin_editor?: boolean;
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

  type FolderKey =
    | 'newFolderPath'
    | 'doneFolderPath'
    | 'notFoundFolderPath'
    | 'archiveFolderPath'
    | 'setsFolderPath';

  // Tab navigation
  const tabs = [
    {
      id: 'folders' as const,
      label: 'Folders',
      icon: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />'
    },
    {
      id: 'editors' as const,
      label: 'Editors',
      icon: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />'
    },
    {
      id: 'watermark' as const,
      label: 'Watermark',
      icon: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />'
    },
    {
      id: 'database' as const,
      label: 'Database',
      icon: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4m0 5c0 2.21-3.582 4-8 4s-8-1.79-8-4" />'
    }
  ];
  let activeTab = $state<'folders' | 'editors' | 'watermark' | 'database'>('folders');

  // Folder configuration
  const folderConfigs: {
    key: FolderKey;
    label: string;
    description: string;
    title: string;
    required: boolean;
  }[] = [
    {
      key: 'newFolderPath',
      label: 'NEW',
      description: 'New properties needing editing',
      title: 'Select folder for NEW properties',
      required: true
    },
    {
      key: 'doneFolderPath',
      label: 'DONE',
      description: 'Completed, waiting to send',
      title: 'Select folder for DONE properties',
      required: true
    },
    {
      key: 'notFoundFolderPath',
      label: 'NOT FOUND',
      description: 'Listing not on website yet',
      title: 'Select folder for NOT FOUND properties',
      required: true
    },
    {
      key: 'archiveFolderPath',
      label: 'ARCHIVE',
      description: 'Done, uploaded, sent to boss',
      title: 'Select folder for ARCHIVED properties',
      required: true
    },
    {
      key: 'setsFolderPath',
      label: 'SETS',
      description: 'ZIP archives output',
      title: 'Select folder for Sets (ZIP archives)',
      required: false
    }
  ];

  // Reactive state
  let config = $state<AppConfig>({
    newFolderPath: '',
    doneFolderPath: '',
    notFoundFolderPath: '',
    archiveFolderPath: '',
    setsFolderPath: '',
    isValidPath: false,
    lastUpdated: null,
    use_builtin_editor: true,
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

  let isLoading = $state(false);

  // Auto-save state
  let saveState = $state<'idle' | 'saving' | 'saved' | 'error'>('idle');
  let saveTimeout: number | null = null;
  let savedResetTimeout: number | null = null;

  function autoSave(delay: number = 300): void {
    if (saveTimeout) clearTimeout(saveTimeout);
    if (savedResetTimeout) clearTimeout(savedResetTimeout);
    saveState = 'saving';
    saveTimeout = window.setTimeout(async () => {
      try {
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
            use_builtin_editor: config.use_builtin_editor,
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
          saveState = 'saved';
          savedResetTimeout = window.setTimeout(() => {
            if (saveState === 'saved') saveState = 'idle';
          }, 2000);
        } else {
          saveState = 'error';
          showError(result.error || 'Failed to save configuration');
        }
      } catch (error) {
        saveState = 'error';
        showError(`Error saving configuration: ${error}`);
      }
    }, delay);
  }

  // Watermark preview state
  let watermarkPreviewUrl = $state('');
  let isGeneratingPreview = $state(false);

  // Confirmation dialog states
  let showResetConfigConfirm = $state(false);
  let showResetDatabaseConfirm = $state(false);

  // Repair status state
  let isRepairing = $state(false);
  let repairResult = $state<{
    propertiesChecked: number;
    propertiesFixed: number;
    errors: string[];
  } | null>(null);
  let showRepairResult = $state(false);

  // Scan state
  let isScanning = $state(false);
  let scanResult = $state<ScanResult | null>(null);
  let showScanResult = $state(false);

  // Load config on mount
  onMount(async () => {
    await loadConfig();
  });

  async function loadConfig(): Promise<void> {
    try {
      isLoading = true;
      const loadedConfig = await invoke<AppConfig | null>('load_config');
      if (loadedConfig) {
        config = { ...config, ...loadedConfig };
      }
      // Load watermark preview if configured
      if (config.watermark_image_path) {
        await generatePreview();
      }
    } catch (error) {
      showError(`Error loading configuration: ${error}`);
    } finally {
      isLoading = false;
    }
  }

  // Generic folder selector
  async function selectFolder(key: FolderKey, title: string): Promise<void> {
    try {
      const selected = await open({ directory: true, multiple: false, title });
      if (selected && typeof selected === 'string') {
        config[key] = selected;
        validateFolderPaths();
        autoSave(0);
      }
    } catch (error) {
      showError(`Error selecting folder: ${error}`);
    }
  }

  function validateFolderPaths(): void {
    config.isValidPath =
      config.newFolderPath !== '' &&
      config.doneFolderPath !== '' &&
      config.notFoundFolderPath !== '' &&
      config.archiveFolderPath !== '';
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
        use_builtin_editor: true,
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
      };
      watermarkPreviewUrl = '';
      saveState = 'idle';
      showSuccess('Configuration reset successfully');
    } catch (error) {
      showError(`Error resetting configuration: ${error}`);
    }
  }

  // Scan & import
  async function scanAndImport(): Promise<void> {
    const hasFolderConfigured =
      config.newFolderPath ||
      config.doneFolderPath ||
      config.notFoundFolderPath ||
      config.archiveFolderPath;

    if (!hasFolderConfigured) {
      showError('Please set up at least one status folder first');
      return;
    }

    try {
      isScanning = true;
      scanResult = await DatabaseService.scanAndImportProperties();
      showScanResult = true;
      if (scanResult?.newProperties && scanResult.newProperties > 0) {
        showSuccess(`Imported ${scanResult.newProperties} new properties`);
      } else {
        showInfo('No new properties found');
      }
    } catch (error) {
      showError(`Error scanning properties: ${error}`);
    } finally {
      isScanning = false;
    }
  }

  // Database operations
  function resetDatabase(): void {
    showResetDatabaseConfirm = true;
  }

  async function doResetDatabase(): Promise<void> {
    showResetDatabaseConfirm = false;
    try {
      await invoke('reset_database_with_proper_dates');
      showSuccess('Database cleared successfully');
    } catch (error) {
      showError(`Reset failed: ${error}`);
    }
  }

  async function repairPropertyStatuses(): Promise<void> {
    try {
      isRepairing = true;
      repairResult = null;
      const result = await DatabaseService.repairPropertyStatuses();
      if (result) {
        repairResult = result;
        showRepairResult = true;
        if (result.propertiesFixed > 0) {
          showSuccess(`Repaired ${result.propertiesFixed} properties`);
        } else {
          showInfo('All properties are correctly synced');
        }
      }
    } catch (error) {
      showError(`Repair failed: ${error}`);
    } finally {
      isRepairing = false;
    }
  }

  // Editor selection
  async function selectFastEditor(): Promise<void> {
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        title: 'Select Fast Image Editor',
        filters: [{ name: 'Executable Files', extensions: ['exe', 'app'] }]
      });
      if (selected && typeof selected === 'string') {
        config.fast_editor_path = selected;
        const pathParts = selected.split(/[/\\]/);
        const filename = pathParts[pathParts.length - 1];
        config.fast_editor_name = filename.replace(/\.(exe|app)$/i, '');
        autoSave(0);
      }
    } catch (error) {
      showError(`Error selecting fast editor: ${error}`);
    }
  }

  async function selectComplexEditor(): Promise<void> {
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        title: 'Select Complex Image Editor (Photoshop, GIMP, etc.)',
        filters: [{ name: 'Executable Files', extensions: ['exe', 'app'] }]
      });
      if (selected && typeof selected === 'string') {
        config.complex_editor_path = selected;
        const pathParts = selected.split(/[/\\]/);
        const filename = pathParts[pathParts.length - 1];
        config.complex_editor_name = filename.replace(/\.(exe|app)$/i, '');
        autoSave(0);
      }
    } catch (error) {
      showError(`Error selecting complex editor: ${error}`);
    }
  }

  function resetEditors(): void {
    config.fast_editor_path = undefined;
    config.fast_editor_name = undefined;
    config.complex_editor_path = undefined;
    config.complex_editor_name = undefined;
    autoSave(0);
  }

  // Watermark
  async function selectWatermarkImage(): Promise<void> {
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        title: 'Select Watermark Image',
        filters: [{ name: 'Image Files', extensions: ['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp'] }]
      });
      if (selected && typeof selected === 'string') {
        await invoke('copy_watermark_to_app_data', { sourcePath: selected });
        const appDataPath = await invoke<string | null>('get_watermark_from_app_data');
        if (appDataPath) {
          config.watermark_image_path = appDataPath;
          await generatePreview();
          autoSave(0);
        }
      }
    } catch (error) {
      showError(`Error selecting watermark image: ${error}`);
    }
  }

  async function generatePreview(): Promise<void> {
    if (!config.watermark_image_path) {
      watermarkPreviewUrl = '';
      return;
    }
    try {
      isGeneratingPreview = true;
      const base64Preview = await invoke<string>('generate_watermark_preview', {
        sampleImageBase64: null
      });
      watermarkPreviewUrl = `data:image/png;base64,${base64Preview}`;
    } catch (error) {
      console.error('Failed to generate preview:', error);
      watermarkPreviewUrl = '';
    } finally {
      isGeneratingPreview = false;
    }
  }

  let previewTimeout: number | null = null;
  function onWatermarkChange(): void {
    // Debounce preview
    if (previewTimeout) clearTimeout(previewTimeout);
    previewTimeout = window.setTimeout(() => generatePreview(), 500);
    // Debounce save
    autoSave(500);
  }

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
    autoSave(0);
  }
</script>

<div class="bg-background-0 flex h-full flex-col">
  <!-- Header -->
  <div class="bg-background-50 border-background-200 border-b">
    <div class="flex items-center justify-between px-6 py-4">
      <div>
        <h1 class="text-foreground-900 text-xl font-semibold">Settings</h1>
        <p class="text-foreground-600 mt-0.5 text-sm">Configure your workspace</p>
      </div>
      <!-- Save indicator -->
      {#if saveState === 'saving'}
        <span class="text-foreground-500 flex items-center gap-1.5 text-xs">
          <div
            class="border-foreground-400 h-3 w-3 animate-spin rounded-full border border-t-transparent"
          ></div>
          Saving...
        </span>
      {:else if saveState === 'saved'}
        <span class="flex items-center gap-1.5 text-xs text-green-600">
          <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M5 13l4 4L19 7"
            />
          </svg>
          Saved
        </span>
      {:else if saveState === 'error'}
        <span class="flex items-center gap-1.5 text-xs text-red-600">
          <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
          Save failed
        </span>
      {/if}
    </div>
  </div>

  <!-- Two-column layout -->
  <div class="flex flex-1 overflow-hidden">
    <!-- Settings sidebar -->
    <nav class="border-background-200 w-40 flex-shrink-0 border-r py-3">
      {#each tabs as tab}
        <button
          onclick={() => (activeTab = tab.id)}
          class="flex w-full items-center gap-2.5 border-l-2 px-3 py-2 text-sm transition-colors
            {activeTab === tab.id
            ? 'border-foreground-900 bg-background-100 text-foreground-900 font-medium'
            : 'text-foreground-600 hover:bg-background-100 hover:text-foreground-900 border-transparent'}"
        >
          <svg class="h-4 w-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            {@html tab.icon}
          </svg>
          {tab.label}
        </button>
      {/each}
    </nav>

    <!-- Content -->
    <div class="flex-1 overflow-y-auto p-6">
      <!-- Folders Tab -->
      {#if activeTab === 'folders'}
        <div class="max-w-3xl space-y-4">
          <div>
            <h2 class="text-foreground-900 text-sm font-semibold">Folder Paths</h2>
            <p class="text-foreground-600 mt-0.5 text-xs">
              Configure where properties are stored by status
            </p>
          </div>

          <div class="bg-background-50 border-background-200 border">
            {#each folderConfigs as folder, i}
              <div
                class="flex items-center gap-3 px-4 py-2.5 {i > 0
                  ? 'border-background-200 border-t'
                  : ''}"
              >
                <div class="w-24 flex-shrink-0">
                  <span class="text-foreground-700 text-xs font-medium">{folder.label}</span>
                  {#if folder.required}
                    <span class="ml-0.5 text-red-500">*</span>
                  {/if}
                </div>
                <div class="min-w-0 flex-1">
                  <input
                    type="text"
                    readonly
                    value={config[folder.key] || 'Not set'}
                    class="border-background-300 bg-background-100 text-foreground-900 w-full border px-3 py-1.5 font-mono text-xs
                      {config[folder.key] ? '' : 'text-foreground-400 italic'}"
                  />
                </div>
                <button
                  onclick={() => selectFolder(folder.key, folder.title)}
                  disabled={isLoading}
                  class="bg-background-100 hover:bg-background-200 text-foreground-700 flex-shrink-0 px-3 py-1.5 text-xs font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
                >
                  Browse
                </button>
              </div>
            {/each}
          </div>

          <div class="flex items-center gap-2">
            <div
              class="h-2 w-2 rounded-full {config.isValidPath
                ? 'bg-green-500'
                : 'bg-background-300'}"
            ></div>
            <span class="text-foreground-500 text-xs">
              {config.isValidPath
                ? 'All required folders configured'
                : 'Configure all 4 required folders to enable full functionality'}
            </span>
          </div>
        </div>

        <!-- Editors Tab -->
      {:else if activeTab === 'editors'}
        <div class="max-w-3xl space-y-5">
          <div>
            <h2 class="text-foreground-900 text-sm font-semibold">Image Editors</h2>
            <p class="text-foreground-600 mt-0.5 text-xs">Configure editing applications</p>
          </div>

          <!-- Fast Editor -->
          <div class="bg-background-50 border-background-200 border p-4">
            <h3 class="text-foreground-900 mb-1 text-sm font-semibold">Fast Editor</h3>
            <p class="text-foreground-600 mb-3 text-xs">
              For quick edits: brightness, contrast, crop
            </p>

            <div class="space-y-2">
              <label class="flex cursor-pointer items-center gap-2">
                <input
                  type="radio"
                  name="editorType"
                  checked={config.use_builtin_editor === true}
                  onchange={() => {
                    config.use_builtin_editor = true;
                    autoSave(0);
                  }}
                  class="text-accent-600 h-3.5 w-3.5"
                />
                <span class="text-foreground-900 text-sm">Built-in editor</span>
                <span class="text-foreground-500 text-xs">(recommended)</span>
              </label>
              <label class="flex cursor-pointer items-center gap-2">
                <input
                  type="radio"
                  name="editorType"
                  checked={config.use_builtin_editor === false}
                  onchange={() => {
                    config.use_builtin_editor = false;
                    autoSave(0);
                  }}
                  class="text-accent-600 h-3.5 w-3.5"
                />
                <span class="text-foreground-900 text-sm">External application</span>
              </label>
            </div>

            {#if config.use_builtin_editor === false}
              <div class="border-background-200 mt-3 ml-5 border-l-2 pl-3">
                <div class="flex items-center gap-2">
                  <input
                    type="text"
                    readonly
                    value={config.fast_editor_name || 'System default'}
                    class="border-background-300 bg-background-100 text-foreground-900 min-w-0 flex-1 border px-3 py-1.5 text-sm"
                  />
                  <button
                    onclick={selectFastEditor}
                    disabled={isLoading}
                    class="bg-background-100 hover:bg-background-200 text-foreground-700 px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50"
                  >
                    Browse
                  </button>
                </div>
                {#if config.fast_editor_path}
                  <p class="text-foreground-400 mt-1 font-mono text-xs">
                    {config.fast_editor_path}
                  </p>
                {/if}
                <p class="text-foreground-500 mt-2 text-xs">
                  Recommended: IrfanView, FastStone Image Viewer
                </p>
              </div>
            {/if}
          </div>

          <!-- Advanced Editor -->
          <div class="bg-background-50 border-background-200 border p-4">
            <h3 class="text-foreground-900 mb-1 text-sm font-semibold">Advanced Editor</h3>
            <p class="text-foreground-600 mb-3 text-xs">
              For complex edits: masking, layers, retouching
            </p>

            <div class="flex items-center gap-2">
              <input
                type="text"
                readonly
                value={config.complex_editor_name || 'System default'}
                class="border-background-300 bg-background-100 text-foreground-900 min-w-0 flex-1 border px-3 py-1.5 text-sm"
              />
              <button
                onclick={selectComplexEditor}
                disabled={isLoading}
                class="bg-background-100 hover:bg-background-200 text-foreground-700 px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50"
              >
                Browse
              </button>
            </div>
            {#if config.complex_editor_path}
              <p class="text-foreground-400 mt-1 font-mono text-xs">{config.complex_editor_path}</p>
            {/if}
            <p class="text-foreground-500 mt-2 text-xs">
              Recommended: Adobe Photoshop, GIMP, Paint.NET
            </p>
          </div>

          {#if config.fast_editor_path || config.complex_editor_path}
            <button onclick={resetEditors} class="text-xs text-red-600 hover:text-red-700">
              Clear editor selections
            </button>
          {/if}
        </div>

        <!-- Watermark Tab -->
      {:else if activeTab === 'watermark'}
        <div class="max-w-3xl space-y-5">
          <div>
            <h2 class="text-foreground-900 text-sm font-semibold">Watermark Configuration</h2>
            <p class="text-foreground-600 mt-0.5 text-xs">
              Configure watermark image, size, position, and opacity
            </p>
          </div>

          <!-- Watermark Image Selection -->
          <div class="bg-background-50 border-background-200 border p-4">
            <h3 class="text-foreground-700 mb-2 text-xs font-medium">Watermark Image</h3>
            <div class="flex items-center gap-2">
              <input
                type="text"
                readonly
                value={config.watermark_image_path || 'No watermark image selected'}
                class="border-background-300 bg-background-100 text-foreground-900 min-w-0 flex-1 border px-3 py-1.5 text-sm
                  {config.watermark_image_path ? '' : 'text-foreground-400 italic'}"
              />
              <button
                onclick={selectWatermarkImage}
                disabled={isLoading}
                class="bg-background-100 hover:bg-background-200 text-foreground-700 px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50"
              >
                Browse
              </button>
            </div>
            <p class="text-foreground-500 mt-1.5 text-xs">
              PNG with transparent background recommended
            </p>
          </div>

          {#if config.watermark_image_path}
            <!-- Size Configuration -->
            <div class="bg-background-50 border-background-200 border p-4">
              <h3 class="text-foreground-900 mb-3 text-sm font-semibold">Size</h3>

              <div class="mb-4 grid grid-cols-2 gap-2 md:grid-cols-4">
                {#each ['proportional', 'fit', 'stretch', 'tile'] as mode}
                  <label class="flex cursor-pointer items-center gap-1.5">
                    <input
                      type="radio"
                      name="sizeMode"
                      value={mode}
                      bind:group={config.watermarkConfig.sizeMode}
                      onchange={onWatermarkChange}
                      class="text-accent-600 h-3.5 w-3.5"
                    />
                    <span class="text-foreground-700 text-sm capitalize">{mode}</span>
                  </label>
                {/each}
              </div>

              {#if config.watermarkConfig.sizeMode === 'proportional'}
                <div class="space-y-3">
                  <div>
                    <label class="text-foreground-700 mb-1.5 block text-xs font-medium">
                      Size: {Math.round(config.watermarkConfig.sizePercentage * 100)}%
                      <input
                        type="range"
                        min="0.05"
                        max="1"
                        step="0.05"
                        bind:value={config.watermarkConfig.sizePercentage}
                        oninput={onWatermarkChange}
                        class="bg-accent-200 mt-1.5 block h-2 w-full cursor-pointer appearance-none rounded-lg"
                      />
                    </label>
                  </div>
                  <div>
                    <label class="text-foreground-700 mb-1.5 block text-xs font-medium">
                      Relative to
                      <select
                        bind:value={config.watermarkConfig.relativeTo}
                        onchange={onWatermarkChange}
                        class="border-background-300 bg-background-100 text-foreground-900 mt-1.5 block w-full border px-3 py-1.5 text-sm font-normal focus:outline-none"
                      >
                        <option value="longest-side">Longest side</option>
                        <option value="shortest-side">Shortest side</option>
                        <option value="width">Width</option>
                        <option value="height">Height</option>
                      </select>
                    </label>
                  </div>
                </div>
              {/if}
            </div>

            <!-- Position Configuration -->
            <div class="bg-background-50 border-background-200 border p-4">
              <h3 class="text-foreground-900 mb-3 text-sm font-semibold">Position</h3>

              <div class="mb-4">
                <span class="text-foreground-700 mb-2 block text-xs font-medium">Anchor</span>
                <div class="grid max-w-[180px] grid-cols-3 gap-1.5">
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
                        onWatermarkChange();
                      }}
                      class="border-background-300 flex h-9 items-center justify-center border text-xs font-medium transition-colors
                        {config.watermarkConfig.positionAnchor === value
                        ? 'bg-accent-500 text-white'
                        : 'bg-background-50 text-foreground-700 hover:bg-accent-100'}"
                    >
                      {label}
                    </button>
                  {/each}
                </div>
              </div>

              <div class="grid grid-cols-2 gap-3" style="max-width: 180px;">
                <div>
                  <label class="text-foreground-700 mb-1.5 block text-xs font-medium">
                    Offset X
                    <input
                      type="number"
                      bind:value={config.watermarkConfig.offsetX}
                      oninput={onWatermarkChange}
                      class="border-background-300 bg-background-100 text-foreground-900 mt-1.5 block w-full border px-3 py-1.5 text-sm font-normal focus:outline-none"
                      placeholder="0"
                    />
                  </label>
                </div>
                <div>
                  <label class="text-foreground-700 mb-1.5 block text-xs font-medium">
                    Offset Y
                    <input
                      type="number"
                      bind:value={config.watermarkConfig.offsetY}
                      oninput={onWatermarkChange}
                      class="border-background-300 bg-background-100 text-foreground-900 mt-1.5 block w-full border px-3 py-1.5 text-sm font-normal focus:outline-none"
                      placeholder="0"
                    />
                  </label>
                </div>
              </div>
            </div>

            <!-- Opacity Configuration -->
            <div class="bg-background-50 border-background-200 border p-4">
              <h3 class="text-foreground-900 mb-3 text-sm font-semibold">Opacity</h3>

              <div class="mb-3">
                <label class="text-foreground-700 mb-1.5 block text-xs font-medium">
                  Opacity: {Math.round(config.watermarkConfig.opacity * 100)}%
                  <div class="mt-1.5 flex items-center gap-3">
                    <span class="text-foreground-500 text-xs font-normal">0%</span>
                    <input
                      type="range"
                      min="0"
                      max="1"
                      step="0.05"
                      bind:value={config.watermarkConfig.opacity}
                      oninput={onWatermarkChange}
                      class="bg-accent-200 h-2 flex-1 cursor-pointer appearance-none rounded-lg"
                    />
                    <span class="text-foreground-500 text-xs font-normal">100%</span>
                  </div>
                </label>
              </div>

              <label class="flex cursor-pointer items-center gap-2">
                <input
                  type="checkbox"
                  bind:checked={config.watermarkConfig.useAlphaChannel}
                  onchange={onWatermarkChange}
                  class="text-accent-600 h-3.5 w-3.5 rounded"
                />
                <span class="text-foreground-700 text-sm"
                  >Use alpha channel (respect PNG transparency)</span
                >
              </label>
            </div>

            <!-- Live Preview -->
            <div class="bg-background-100 border-background-200 border p-4">
              <div class="mb-2 flex items-center justify-between">
                <h3 class="text-foreground-900 text-sm font-semibold">Preview</h3>
                {#if isGeneratingPreview}
                  <div
                    class="border-accent-600 h-3 w-3 animate-spin rounded-full border border-t-transparent"
                  ></div>
                {/if}
              </div>

              {#if watermarkPreviewUrl}
                <div class="border-background-300 bg-background-50 overflow-hidden border">
                  <img src={watermarkPreviewUrl} alt="Watermark preview" class="h-auto w-full" />
                </div>
                <p class="text-foreground-500 mt-1.5 text-xs">
                  Preview shows how watermark will appear on images
                </p>
              {:else if isGeneratingPreview}
                <div class="flex h-40 items-center justify-center">
                  <span class="text-foreground-500 text-sm">Generating preview...</span>
                </div>
              {:else}
                <div class="flex h-40 items-center justify-center">
                  <span class="text-foreground-500 text-sm">Configure settings to see preview</span>
                </div>
              {/if}
            </div>

            <button
              onclick={clearWatermarkSettings}
              class="text-xs text-red-600 hover:text-red-700"
            >
              Clear watermark settings
            </button>
          {/if}
        </div>

        <!-- Database Tab -->
      {:else if activeTab === 'database'}
        <div class="max-w-3xl space-y-5">
          <div>
            <h2 class="text-foreground-900 text-sm font-semibold">Database</h2>
            <p class="text-foreground-600 mt-0.5 text-xs">
              Import, repair, and manage your property database
            </p>
          </div>

          <!-- Import Properties -->
          <div class="bg-background-50 border-background-200 border p-4">
            <h3 class="text-foreground-900 text-sm font-semibold">Import Properties</h3>
            <p class="text-foreground-600 mt-1 mb-3 text-xs">
              Scan configured folders for existing properties and add them to the database.
            </p>
            <button
              onclick={scanAndImport}
              disabled={isScanning || !config.isValidPath}
              class="bg-accent-500 hover:bg-accent-600 px-4 py-2 text-sm font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              {#if isScanning}
                <span class="flex items-center gap-2">
                  <div
                    class="h-3 w-3 animate-spin rounded-full border border-white border-t-transparent"
                  ></div>
                  Scanning...
                </span>
              {:else}
                Scan & Import
              {/if}
            </button>
            {#if !config.isValidPath}
              <p class="text-foreground-400 mt-2 text-xs">Configure all required folders first</p>
            {/if}
          </div>

          <!-- Repair Database -->
          <div class="bg-background-50 border-background-200 border p-4">
            <h3 class="text-foreground-900 text-sm font-semibold">Repair Database</h3>
            <p class="text-foreground-600 mt-1 mb-3 text-xs">
              Check each property's folder location and update the database status to match. Use if
              properties were moved manually.
            </p>
            <button
              onclick={repairPropertyStatuses}
              disabled={isRepairing || !config.isValidPath}
              class="bg-accent-500 hover:bg-accent-600 px-4 py-2 text-sm font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              {#if isRepairing}
                <span class="flex items-center gap-2">
                  <div
                    class="h-3 w-3 animate-spin rounded-full border border-white border-t-transparent"
                  ></div>
                  Repairing...
                </span>
              {:else}
                Repair Statuses
              {/if}
            </button>
            {#if !config.isValidPath}
              <p class="text-foreground-400 mt-2 text-xs">Configure all required folders first</p>
            {/if}
          </div>

          <!-- Danger Zone -->
          <div class="border border-red-200 p-4 dark:border-red-900">
            <h3 class="mb-3 text-sm font-semibold text-red-900 dark:text-red-200">Danger Zone</h3>

            <div class="space-y-3">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <p class="text-foreground-900 text-sm">Clear Database</p>
                  <p class="text-foreground-600 text-xs">
                    Delete all properties from the database. Folders are not affected.
                  </p>
                </div>
                <button
                  onclick={resetDatabase}
                  disabled={!config.isValidPath}
                  class="flex-shrink-0 bg-red-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-red-700 disabled:opacity-50"
                >
                  Clear
                </button>
              </div>

              <div class="border-t border-red-200 dark:border-red-900"></div>

              <div class="flex items-center justify-between gap-4">
                <div>
                  <p class="text-foreground-900 text-sm">Reset Configuration</p>
                  <p class="text-foreground-600 text-xs">Clear all settings and start over.</p>
                </div>
                <button
                  onclick={resetConfig}
                  disabled={isLoading}
                  class="flex-shrink-0 bg-red-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-red-700 disabled:opacity-50"
                >
                  Reset
                </button>
              </div>
            </div>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>

<!-- Scan Results Modal -->
{#if showScanResult && scanResult}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
    <div
      class="bg-background-50 border-background-200 mx-4 max-h-[80vh] w-full max-w-2xl overflow-y-auto border"
    >
      <div class="border-background-200 flex items-center justify-between border-b p-4">
        <h3 class="text-foreground-900 text-lg font-semibold">Scan Results</h3>
        <button
          onclick={() => (showScanResult = false)}
          aria-label="Close"
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

        {#if scanResult.newProperties > 0}
          <div class="bg-background-100 border-background-200 mb-4 border p-4">
            <div class="flex items-center gap-3">
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

        {#if scanResult.errors.length > 0}
          <div class="mb-4 border border-orange-200 bg-orange-50 p-4">
            <div class="flex items-start gap-3">
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
                    <div class="text-sm text-orange-700">{error}</div>
                  {/each}
                </div>
              </div>
            </div>
          </div>
        {/if}

        {#if scanResult.foundProperties === 0}
          <div class="border-background-300 bg-background-100 mb-4 border p-4">
            <div class="flex items-center gap-3">
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
                No properties found. Make sure your properties are organized in the correct folder
                structure.
              </p>
            </div>
          </div>
        {/if}

        <div class="flex justify-end">
          <button
            onclick={() => (showScanResult = false)}
            class="bg-accent-500 hover:bg-accent-600 px-4 py-2 text-sm font-medium text-white transition-colors"
          >
            Close
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
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    onclick={() => (showRepairResult = false)}
    onkeydown={(e) => e.key === 'Escape' && (showRepairResult = false)}
    role="dialog"
    aria-label="Repair results dialog"
    tabindex="-1"
  >
    <div
      class="bg-background-0 mx-4 max-h-[80vh] w-full max-w-md overflow-y-auto p-6 shadow-xl"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
      role="document"
    >
      <div class="mb-4 flex items-center justify-between">
        <h3 class="text-foreground-900 text-lg font-semibold">Repair Results</h3>
        <button
          onclick={() => (showRepairResult = false)}
          aria-label="Close"
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

      <div class="space-y-4">
        <div class="bg-background-50 border-background-200 grid grid-cols-2 gap-4 border p-4">
          <div class="text-center">
            <p class="text-foreground-500 text-xs">Checked</p>
            <p class="text-foreground-900 text-2xl font-bold">{repairResult.propertiesChecked}</p>
          </div>
          <div class="text-center">
            <p class="text-foreground-500 text-xs">Fixed</p>
            <p
              class="text-2xl font-bold {repairResult.propertiesFixed > 0
                ? 'text-green-600'
                : 'text-foreground-900'}"
            >
              {repairResult.propertiesFixed}
            </p>
          </div>
        </div>

        {#if repairResult.propertiesFixed > 0}
          <div class="flex items-start gap-2 border border-green-200 bg-green-50 p-3">
            <svg
              class="mt-0.5 h-4 w-4 flex-shrink-0 text-green-600"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M5 13l4 4L19 7"
              />
            </svg>
            <p class="text-sm text-green-700">
              Successfully repaired {repairResult.propertiesFixed}
              {repairResult.propertiesFixed === 1 ? 'property' : 'properties'}.
            </p>
          </div>
        {:else}
          <div class="bg-background-50 border-background-200 flex items-start gap-2 border p-3">
            <svg
              class="text-foreground-400 mt-0.5 h-4 w-4 flex-shrink-0"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M5 13l4 4L19 7"
              />
            </svg>
            <p class="text-foreground-600 text-sm">
              All properties are correctly synced. No repairs needed.
            </p>
          </div>
        {/if}

        {#if repairResult.errors.length > 0}
          <div class="border border-amber-200 bg-amber-50 p-3">
            <p class="mb-2 text-sm font-medium text-amber-800">
              Warnings ({repairResult.errors.length})
            </p>
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
          class="bg-accent-500 hover:bg-accent-600 px-4 py-2 text-sm font-medium text-white transition-colors"
        >
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
