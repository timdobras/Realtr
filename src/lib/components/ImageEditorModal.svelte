<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { Dialog, DialogContent, DialogFooter } from '$lib/components/ui';
  import { Dialog as BitsDialog } from 'bits-ui';
  import { showSuccess, showError } from '$lib/stores/notification';
  import {
    type EditorState,
    type EditorTool,
    type HistoryEntry,
    createDefaultState,
    cloneState,
    stateToParams,
    hasModifications
  } from '$lib/types/imageEditor';

  interface Props {
    open?: boolean;
    folderPath: string;
    status: string;
    subfolder: string;
    filename: string;
    onClose: () => void;
    onSave: () => void;
  }

  let {
    open = $bindable(true),
    folderPath,
    status,
    subfolder,
    filename,
    onClose,
    onSave
  }: Props = $props();

  // Full image path resolved from backend
  let imagePath = $state('');

  // Editor state
  let editorState = $state<EditorState>(createDefaultState());
  let history = $state<HistoryEntry[]>([]);
  let historyIndex = $state(-1);
  let activeTool = $state<EditorTool>('adjust');

  // Preview state
  let previewBase64 = $state<string | null>(null);
  let isLoadingPreview = $state(false);
  let isSaving = $state(false);
  let imageDimensions = $state<{ width: number; height: number } | null>(null);

  // Debounce timer for preview updates
  let previewTimeout: ReturnType<typeof setTimeout> | null = null;

  // CSS filter values for instant feedback
  let cssFilters = $derived(() => {
    const b = 1 + editorState.adjustments.brightness / 100;
    const c = 1 + editorState.adjustments.contrast / 100;
    const e = Math.pow(2, editorState.adjustments.exposure / 50);
    return `brightness(${b * e}) contrast(${c})`;
  });

  // CSS rotation for instant feedback
  let cssRotation = $derived(`rotate(${editorState.rotation.fine}deg)`);

  // Initialize when modal opens
  $effect(() => {
    if (open && folderPath && filename) {
      initEditor();
    }
  });

  async function initEditor() {
    try {
      // Reset state
      editorState = createDefaultState();
      history = [];
      historyIndex = -1;
      previewBase64 = null;
      imagePath = '';

      // Get full property path from backend
      const result = await invoke<{
        success: boolean;
        error?: string;
        data?: { full_path: string };
      }>('get_full_property_path', {
        folderPath,
        status
      });

      if (!result.success || !result.data?.full_path) {
        throw new Error(result.error || 'Failed to get property path');
      }

      const fullPath = result.data.full_path;

      // Construct the full image path
      imagePath = `${fullPath}\\${subfolder}\\${filename}`;

      // Get image dimensions
      const dims = await invoke<[number, number]>('editor_get_dimensions', {
        imagePath
      });
      imageDimensions = { width: dims[0], height: dims[1] };

      // Load initial preview
      await updatePreview();

      // Save initial state to history
      pushHistory('Initial');
    } catch (err) {
      console.error('Failed to initialize editor:', err);
      showError(`Failed to load image: ${err}`);
    }
  }

  function pushHistory(label: string) {
    if (historyIndex < history.length - 1) {
      history = history.slice(0, historyIndex + 1);
    }
    history = [...history, { state: cloneState(editorState), label }];
    historyIndex = history.length - 1;
  }

  function undo() {
    if (historyIndex > 0) {
      historyIndex--;
      editorState = cloneState(history[historyIndex].state);
      schedulePreviewUpdate();
    }
  }

  function redo() {
    if (historyIndex < history.length - 1) {
      historyIndex++;
      editorState = cloneState(history[historyIndex].state);
      schedulePreviewUpdate();
    }
  }

  function resetAll() {
    editorState = createDefaultState();
    pushHistory('Reset all');
    schedulePreviewUpdate();
  }

  function schedulePreviewUpdate(delay = 50) {
    if (previewTimeout) {
      clearTimeout(previewTimeout);
    }
    previewTimeout = setTimeout(() => {
      updatePreview();
    }, delay);
  }

  async function updatePreview() {
    if (!imagePath) return;

    isLoadingPreview = true;
    try {
      const params = stateToParams(editorState);
      const base64 = await invoke<string>('editor_generate_preview', {
        imagePath,
        params,
        previewSize: 1200
      });
      previewBase64 = base64;
    } catch (err) {
      console.error('Failed to generate preview:', err);
    } finally {
      isLoadingPreview = false;
    }
  }

  function handleFineRotationChange(value: number) {
    editorState.rotation.fine = value;
  }

  function handleFineRotationCommit() {
    pushHistory(`Rotate ${editorState.rotation.fine.toFixed(1)}°`);
    schedulePreviewUpdate();
  }

  function rotateQuarter(direction: 1 | -1) {
    editorState.rotation.quarterTurns = ((editorState.rotation.quarterTurns + direction + 4) %
      4) as 0 | 1 | 2 | 3;
    pushHistory(`Rotate ${direction === 1 ? '90°' : '-90°'}`);
    schedulePreviewUpdate();
  }

  function handleAdjustmentChange(key: keyof EditorState['adjustments'], value: number) {
    editorState.adjustments[key] = value;
  }

  function handleAdjustmentCommit(key: keyof EditorState['adjustments']) {
    pushHistory(`Adjust ${key}`);
    schedulePreviewUpdate();
  }

  function resetAdjustment(key: keyof EditorState['adjustments']) {
    editorState.adjustments[key] = 0;
    pushHistory(`Reset ${key}`);
    schedulePreviewUpdate();
  }

  function toggleCrop() {
    editorState.crop.enabled = !editorState.crop.enabled;
    if (editorState.crop.enabled) {
      editorState.crop.x = 0;
      editorState.crop.y = 0;
      editorState.crop.width = 1;
      editorState.crop.height = 1;
    }
    pushHistory(editorState.crop.enabled ? 'Enable crop' : 'Disable crop');
    schedulePreviewUpdate();
  }

  async function handleSave() {
    if (!imagePath) return;

    isSaving = true;
    try {
      const params = stateToParams(editorState);
      const result = await invoke<{ success: boolean; error?: string }>('editor_save_image', {
        imagePath,
        params
      });

      if (result.success) {
        showSuccess('Image saved successfully');
        onSave();
        open = false;
      } else {
        showError(result.error || 'Failed to save image');
      }
    } catch (err) {
      console.error('Failed to save image:', err);
      showError(`Failed to save: ${err}`);
    } finally {
      isSaving = false;
    }
  }

  function handleClose() {
    if (hasModifications(editorState)) {
      if (!confirm('You have unsaved changes. Are you sure you want to close?')) {
        return;
      }
    }
    onClose();
    open = false;
  }

  function handleOpenChange(newOpen: boolean) {
    if (!newOpen) {
      handleClose();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.ctrlKey || e.metaKey) {
      if (e.key === 'z') {
        e.preventDefault();
        if (e.shiftKey) {
          redo();
        } else {
          undo();
        }
      } else if (e.key === 's') {
        e.preventDefault();
        handleSave();
      }
    } else if (e.key === 'Escape') {
      handleClose();
    }
  }

  let canUndo = $derived(historyIndex > 0);
  let canRedo = $derived(historyIndex < history.length - 1);
</script>

<svelte:window onkeydown={handleKeydown} />

<Dialog bind:open onOpenChange={handleOpenChange}>
  <DialogContent class="flex max-h-[95vh] w-[95vw] max-w-[1600px] flex-col overflow-hidden">
    <!-- Header -->
    <div class="border-background-200 flex items-center justify-between border-b px-5 py-3">
      <div class="flex items-center gap-4">
        <div>
          <BitsDialog.Title class="text-foreground-900 text-lg font-semibold">
            Image Editor
          </BitsDialog.Title>
          <p class="text-foreground-500 text-xs">
            {filename}
            {#if imageDimensions}
              <span class="text-foreground-400">
                ({imageDimensions.width} x {imageDimensions.height})
              </span>
            {/if}
          </p>
        </div>
      </div>

      <!-- Undo/Redo buttons -->
      <div class="flex items-center gap-2">
        <button
          onclick={undo}
          disabled={!canUndo}
          class="text-foreground-600 hover:bg-background-100 rounded p-2 disabled:opacity-30"
          title="Undo (Ctrl+Z)"
          aria-label="Undo"
        >
          <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"
            />
          </svg>
        </button>
        <button
          onclick={redo}
          disabled={!canRedo}
          class="text-foreground-600 hover:bg-background-100 rounded p-2 disabled:opacity-30"
          title="Redo (Ctrl+Shift+Z)"
          aria-label="Redo"
        >
          <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M21 10h-10a8 8 0 00-8 8v2M21 10l-6 6m6-6l-6-6"
            />
          </svg>
        </button>
        <button
          onclick={resetAll}
          class="text-foreground-600 hover:bg-background-100 rounded px-3 py-2 text-sm"
          title="Reset all changes"
        >
          Reset
        </button>
        <BitsDialog.Close
          class="text-foreground-500 hover:text-foreground-700 ml-2 rounded p-1"
          aria-label="Close"
        >
          <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </BitsDialog.Close>
      </div>
    </div>

    <!-- Main content -->
    <div class="flex flex-1 overflow-hidden">
      <!-- Left sidebar: Tools -->
      <div class="border-background-200 flex w-72 flex-col border-r">
        <!-- Tool tabs -->
        <div class="border-background-200 flex border-b">
          <button
            onclick={() => (activeTool = 'rotate')}
            class="flex-1 px-3 py-2 text-sm font-medium transition-colors {activeTool === 'rotate'
              ? 'bg-accent-100 text-accent-700 border-accent-500 border-b-2'
              : 'text-foreground-600 hover:bg-background-100'}"
          >
            Rotate
          </button>
          <button
            onclick={() => (activeTool = 'crop')}
            class="flex-1 px-3 py-2 text-sm font-medium transition-colors {activeTool === 'crop'
              ? 'bg-accent-100 text-accent-700 border-accent-500 border-b-2'
              : 'text-foreground-600 hover:bg-background-100'}"
          >
            Crop
          </button>
          <button
            onclick={() => (activeTool = 'adjust')}
            class="flex-1 px-3 py-2 text-sm font-medium transition-colors {activeTool === 'adjust'
              ? 'bg-accent-100 text-accent-700 border-accent-500 border-b-2'
              : 'text-foreground-600 hover:bg-background-100'}"
          >
            Adjust
          </button>
        </div>

        <!-- Tool controls -->
        <div class="flex-1 overflow-y-auto p-4">
          {#if activeTool === 'rotate'}
            <div class="space-y-4">
              <div>
                <div class="text-foreground-700 mb-2 text-sm font-medium">
                  Fine Rotation: {editorState.rotation.fine.toFixed(1)}°
                </div>
                <input
                  type="range"
                  min="-45"
                  max="45"
                  step="0.1"
                  value={editorState.rotation.fine}
                  oninput={(e) => handleFineRotationChange(parseFloat(e.currentTarget.value))}
                  onchange={handleFineRotationCommit}
                  class="bg-background-200 h-2 w-full cursor-pointer appearance-none rounded-lg"
                />
                <div class="text-foreground-500 mt-1 flex justify-between text-xs">
                  <span>-45°</span>
                  <span>0°</span>
                  <span>+45°</span>
                </div>
              </div>

              <div class="flex gap-2">
                <button
                  onclick={() => rotateQuarter(-1)}
                  class="bg-background-100 hover:bg-background-200 flex-1 rounded px-3 py-2 text-sm font-medium"
                >
                  -90°
                </button>
                <button
                  onclick={() => rotateQuarter(1)}
                  class="bg-background-100 hover:bg-background-200 flex-1 rounded px-3 py-2 text-sm font-medium"
                >
                  +90°
                </button>
              </div>

              {#if editorState.rotation.quarterTurns !== 0}
                <p class="text-foreground-500 text-xs">
                  Quarter turns: {editorState.rotation.quarterTurns * 90}°
                </p>
              {/if}
            </div>
          {:else if activeTool === 'crop'}
            <div class="space-y-4">
              <button
                onclick={toggleCrop}
                class="w-full rounded px-4 py-2 text-sm font-medium {editorState.crop.enabled
                  ? 'bg-accent-500 text-white'
                  : 'bg-background-100 text-foreground-700 hover:bg-background-200'}"
              >
                {editorState.crop.enabled ? 'Crop Active' : 'Enable Crop'}
              </button>

              {#if editorState.crop.enabled}
                <p class="text-foreground-500 text-xs">
                  Crop region will be applied when you save.
                </p>
                <div class="text-foreground-600 space-y-1 text-xs">
                  <p>X: {(editorState.crop.x * 100).toFixed(1)}%</p>
                  <p>Y: {(editorState.crop.y * 100).toFixed(1)}%</p>
                  <p>Width: {(editorState.crop.width * 100).toFixed(1)}%</p>
                  <p>Height: {(editorState.crop.height * 100).toFixed(1)}%</p>
                </div>
              {/if}
            </div>
          {:else if activeTool === 'adjust'}
            <div class="space-y-4">
              {#each [{ key: 'brightness', label: 'Brightness' }, { key: 'exposure', label: 'Exposure' }, { key: 'contrast', label: 'Contrast' }, { key: 'highlights', label: 'Highlights' }, { key: 'shadows', label: 'Shadows' }] as { key, label } (key)}
                {@const adjustKey = key as keyof EditorState['adjustments']}
                <div>
                  <div class="mb-1 flex items-center justify-between">
                    <span class="text-foreground-700 text-sm font-medium">
                      {label}
                    </span>
                    <div class="flex items-center gap-2">
                      <span class="text-foreground-500 w-8 text-right text-xs">
                        {editorState.adjustments[adjustKey]}
                      </span>
                      {#if editorState.adjustments[adjustKey] !== 0}
                        <button
                          onclick={() => resetAdjustment(adjustKey)}
                          class="text-foreground-400 hover:text-foreground-600 text-xs"
                          title="Reset {label}"
                          aria-label="Reset {label}"
                        >
                          <svg
                            class="h-3 w-3"
                            fill="none"
                            stroke="currentColor"
                            viewBox="0 0 24 24"
                          >
                            <path
                              stroke-linecap="round"
                              stroke-linejoin="round"
                              stroke-width="2"
                              d="M6 18L18 6M6 6l12 12"
                            />
                          </svg>
                        </button>
                      {/if}
                    </div>
                  </div>
                  <input
                    type="range"
                    min="-100"
                    max="100"
                    step="1"
                    value={editorState.adjustments[adjustKey]}
                    oninput={(e) =>
                      handleAdjustmentChange(adjustKey, parseInt(e.currentTarget.value))}
                    onchange={() => handleAdjustmentCommit(adjustKey)}
                    class="bg-background-200 h-2 w-full cursor-pointer appearance-none rounded-lg"
                  />
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      <!-- Preview area -->
      <div class="bg-background-100 flex flex-1 items-center justify-center overflow-hidden p-6">
        {#if previewBase64}
          <div class="relative max-h-full max-w-full">
            <img
              src="data:image/jpeg;base64,{previewBase64}"
              alt="Preview"
              class="max-h-[calc(95vh-160px)] max-w-full object-contain"
              style="filter: {cssFilters()}; transform: {cssRotation};"
            />
            {#if isLoadingPreview}
              <div class="absolute inset-0 flex items-center justify-center bg-black/20">
                <div class="bg-background-50 rounded-lg px-4 py-2 shadow-lg">
                  <span class="text-foreground-600 text-sm">Updating...</span>
                </div>
              </div>
            {/if}
          </div>
        {:else}
          <div class="text-foreground-500 flex flex-col items-center gap-2">
            <svg class="h-12 w-12 animate-spin" fill="none" viewBox="0 0 24 24">
              <circle
                class="opacity-25"
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                stroke-width="4"
              ></circle>
              <path
                class="opacity-75"
                fill="currentColor"
                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
              ></path>
            </svg>
            <span>Loading image...</span>
          </div>
        {/if}
      </div>
    </div>

    <!-- Footer -->
    <DialogFooter>
      <button
        onclick={handleClose}
        disabled={isSaving}
        class="bg-background-100 text-foreground-700 hover:bg-background-200 rounded px-4 py-2 text-sm font-medium"
      >
        Cancel
      </button>
      <button
        onclick={handleSave}
        disabled={isSaving || !hasModifications(editorState)}
        class="bg-accent-500 hover:bg-accent-600 rounded px-4 py-2 text-sm font-medium text-white disabled:opacity-50"
      >
        {isSaving ? 'Saving...' : 'Save'}
      </button>
    </DialogFooter>
  </DialogContent>
</Dialog>
