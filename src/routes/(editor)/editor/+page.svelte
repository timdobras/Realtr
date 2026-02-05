<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
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
  import WebGLCanvas from '$lib/components/WebGLCanvas.svelte';
  import CropOverlay from '$lib/components/CropOverlay.svelte';
  import EditorTitleBar from '$lib/components/EditorTitleBar.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';

  // Get params from URL
  let folderPath = $derived($page.url.searchParams.get('folderPath') || '');
  let status = $derived($page.url.searchParams.get('status') || '');
  let subfolder = $derived($page.url.searchParams.get('subfolder') || '');
  let filename = $derived($page.url.searchParams.get('filename') || '');

  // Full image path resolved from backend
  let imagePath = $state('');

  // Editor state
  let editorState = $state<EditorState>(createDefaultState());
  let history = $state<HistoryEntry[]>([]);
  let historyIndex = $state(-1);
  let activeTool = $state<EditorTool>('adjust'); // 'crop' or 'adjust' - rotation controls are on the canvas

  // Preview state
  let previewBase64 = $state<string | null>(null);
  let isSaving = $state(false);
  let imageDimensions = $state<{ width: number; height: number } | null>(null);
  let isInitialized = $state(false);
  let initError = $state<string | null>(null);

  // Track when user is actively dragging the rotation slider
  let isRotating = $state(false);
  let rotationTimeoutId: ReturnType<typeof setTimeout> | null = null;

  // Track when crop overlay is shown (separate from crop.enabled which controls if crop is applied on save)
  let isEditingCrop = $state(false);

  // WebGLCanvas state for crop overlay positioning
  let canvasZoom = $state(1);
  let canvasPanX = $state(0);
  let canvasPanY = $state(0);
  let canvasFitScale = $state(100);
  let canvasImageWidth = $state(0);
  let canvasImageHeight = $state(0);

  // Container dimensions for crop overlay
  let canvasContainer = $state<HTMLDivElement | null>(null);
  let containerWidth = $state(0);
  let containerHeight = $state(0);

  // Unsaved changes dialog
  let showUnsavedDialog = $state(false);

  // Image navigation
  let folderImages = $state<string[]>([]);
  let currentImageIndex = $state(0);
  let pendingNavigationDirection = $state<'prev' | 'next' | null>(null);
  let showNavigationDialog = $state(false);

  // Show rotation background while adjusting, hide after 300ms of inactivity
  function showRotationBackground() {
    isRotating = true;
    if (rotationTimeoutId) {
      clearTimeout(rotationTimeoutId);
    }
    rotationTimeoutId = setTimeout(() => {
      isRotating = false;
      rotationTimeoutId = null;
    }, 800);
  }

  // Note: With WebGL, we no longer need debouncing or request tracking
  // All preview updates happen instantly on the GPU

  let resizeObserver: ResizeObserver | null = null;

  onMount(async () => {
    if (folderPath && filename) {
      await initEditor();
    }
  });

  // Track container size for crop overlay - use $effect since container is conditionally rendered
  $effect(() => {
    if (canvasContainer && !resizeObserver) {
      resizeObserver = new ResizeObserver((entries) => {
        for (const entry of entries) {
          containerWidth = entry.contentRect.width;
          containerHeight = entry.contentRect.height;
        }
      });
      resizeObserver.observe(canvasContainer);
    }
  });

  onDestroy(() => {
    if (resizeObserver) {
      resizeObserver.disconnect();
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
      initError = null;

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

      // Load list of images in the folder for navigation
      let images: string[] = [];
      if (subfolder === 'INTERNET') {
        images = await invoke<string[]>('list_internet_images', { folderPath, status });
      } else if (subfolder === 'INTERNET\\AGGELIA' || subfolder === 'AGGELIA') {
        images = await invoke<string[]>('list_aggelia_images', { folderPath, status });
      } else if (!subfolder) {
        images = await invoke<string[]>('list_original_images', { folderPath, status });
      }
      folderImages = images;
      currentImageIndex = images.indexOf(filename);
      if (currentImageIndex === -1) currentImageIndex = 0;

      // Construct the full image path (handle empty subfolder)
      imagePath = subfolder ? `${fullPath}\\${subfolder}\\${filename}` : `${fullPath}\\${filename}`;

      // Load image into cache and get initial preview
      // This caches the image for fast subsequent preview generation
      const loadResult = await invoke<{ width: number; height: number; previewBase64: string }>(
        'editor_load_image',
        {
          imagePath,
          previewSize: 1000
        }
      );
      imageDimensions = { width: loadResult.width, height: loadResult.height };
      previewBase64 = loadResult.previewBase64;

      // Update window title
      const currentWindow = getCurrentWindow();
      await currentWindow.setTitle(`Image Editor - ${filename}`);

      // Save initial state to history
      pushHistory('Initial');
      isInitialized = true;
    } catch (err) {
      console.error('Failed to initialize editor:', err);
      initError = `Failed to load image: ${err}`;
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
      // No backend call needed - WebGL canvas updates instantly via reactive props!
    }
  }

  function redo() {
    if (historyIndex < history.length - 1) {
      historyIndex++;
      editorState = cloneState(history[historyIndex].state);
      // No backend call needed - WebGL canvas updates instantly via reactive props!
    }
  }

  function resetAll() {
    editorState = createDefaultState();
    pushHistory('Reset all');
    // No backend call needed - WebGL canvas updates instantly via reactive props!
  }

  // Note: schedulePreviewUpdate and updatePreview removed - WebGL handles all preview updates instantly

  function handleRotationChange(value: number) {
    editorState.rotation.fine = value;
    // No backend call needed - WebGL canvas updates instantly via reactive props!
    // Note: WebGL shows rotation without auto-crop. Final save will apply proper auto-crop.
  }

  function handleRotationCommit() {
    pushHistory(`Rotate ${editorState.rotation.fine}°`);
  }

  function rotate90(direction: 1 | -1) {
    editorState.rotation.quarterTurns = ((editorState.rotation.quarterTurns + direction + 4) %
      4) as 0 | 1 | 2 | 3;
    pushHistory(`Rotate ${direction === 1 ? '+' : '-'}90°`);
    // No backend call needed - WebGL canvas updates instantly via reactive props!
  }

  function handleAdjustmentChange(key: keyof EditorState['adjustments'], value: number) {
    editorState.adjustments[key] = value;
    // No backend call needed - WebGL canvas updates instantly via reactive props!
  }

  function handleAdjustmentCommit(key: keyof EditorState['adjustments']) {
    pushHistory(`Adjust ${key}`);
  }

  function resetAdjustment(key: keyof EditorState['adjustments']) {
    editorState.adjustments[key] = 0;
    pushHistory(`Reset ${key}`);
    // No backend call needed - WebGL canvas updates instantly via reactive props!
  }

  async function applyAutoAdjustments() {
    try {
      const adjustments = await invoke<{
        brightness: number;
        exposure: number;
        contrast: number;
        highlights: number;
        shadows: number;
      }>('editor_analyze_image');

      editorState.adjustments.brightness = adjustments.brightness;
      editorState.adjustments.exposure = adjustments.exposure;
      editorState.adjustments.contrast = adjustments.contrast;
      editorState.adjustments.highlights = adjustments.highlights;
      editorState.adjustments.shadows = adjustments.shadows;
      pushHistory('Auto adjust');
      // WebGL canvas updates instantly via reactive props!
    } catch (err) {
      console.error('Auto adjust failed:', err);
      showError('Auto adjust failed');
    }
  }

  let isAutoStraightening = $state(false);

  async function applyAutoStraighten() {
    if (isAutoStraightening) return;
    isAutoStraightening = true;
    try {
      const result = await invoke<{
        angle: number;
        confidence: number;
      }>('editor_auto_straighten');

      if (Math.abs(result.angle) > 0.05) {
        editorState.rotation.fine = result.angle;
        showRotationBackground();

        // Show confidence-based feedback
        const confidencePercent = Math.round(result.confidence * 100);

        pushHistory(`Auto straighten ${result.angle.toFixed(1)}°`);

        if (result.confidence >= 0.6) {
          showSuccess(`Rotated ${result.angle.toFixed(1)}° (${confidencePercent}% confident)`);
        } else if (result.confidence >= 0.3) {
          showSuccess(
            `Rotated ${result.angle.toFixed(1)}° (${confidencePercent}% confident - please verify)`
          );
        } else {
          showError(
            `Low confidence (${confidencePercent}%) - rotated ${result.angle.toFixed(1)}°, please verify`
          );
        }
      } else {
        showSuccess('Image appears level');
      }
    } catch (err) {
      console.error('Auto straighten failed:', err);
      showError('Auto straighten failed');
    } finally {
      isAutoStraightening = false;
    }
  }

  function toggleCrop() {
    isEditingCrop = !isEditingCrop;
    if (isEditingCrop) {
      // Starting crop mode - initialize to full frame if no crop was previously applied
      editorState.crop.enabled = true;
      if (editorState.crop.width === 1 && editorState.crop.height === 1) {
        editorState.crop.x = 0;
        editorState.crop.y = 0;
      }
      pushHistory('Edit crop');
    } else {
      // Just closing crop mode without applying
      pushHistory('Close crop editor');
    }
  }

  function handleCropChange(x: number, y: number, width: number, height: number) {
    editorState.crop.x = x;
    editorState.crop.y = y;
    editorState.crop.width = width;
    editorState.crop.height = height;
    // No history push during drag - only on commit
  }

  function handleCropCommit() {
    pushHistory('Crop adjusted');
  }

  function applyCrop() {
    // Exit crop editing mode - crop.enabled stays true so crop is applied on save
    isEditingCrop = false;
    pushHistory('Apply crop');
  }

  function cancelCrop() {
    // Exit crop editing mode and reset crop settings
    isEditingCrop = false;
    editorState.crop.enabled = false;
    editorState.crop.x = 0;
    editorState.crop.y = 0;
    editorState.crop.width = 1;
    editorState.crop.height = 1;
    pushHistory('Cancel crop');
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
        // Reload the image from disk to get the saved version
        const loadResult = await invoke<{ width: number; height: number; previewBase64: string }>(
          'editor_load_image',
          {
            imagePath,
            previewSize: 1000
          }
        );
        imageDimensions = { width: loadResult.width, height: loadResult.height };
        previewBase64 = loadResult.previewBase64;
        // Reset state to mark as "saved" (no modifications)
        editorState = createDefaultState();
        history = [{ state: cloneState(editorState), label: 'Saved' }];
        historyIndex = 0;
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

  async function handleClose() {
    if (hasModifications(editorState)) {
      showUnsavedDialog = true;
      return;
    }
    const currentWindow = getCurrentWindow();
    await currentWindow.close();
  }

  async function confirmClose() {
    showUnsavedDialog = false;
    const currentWindow = getCurrentWindow();
    await currentWindow.close();
  }

  function cancelClose() {
    showUnsavedDialog = false;
  }

  // Navigation functions
  let canGoPrevious = $derived(currentImageIndex > 0);
  let canGoNext = $derived(currentImageIndex < folderImages.length - 1);

  function goToPrevious() {
    if (!canGoPrevious) return;
    if (hasModifications(editorState)) {
      pendingNavigationDirection = 'prev';
      showNavigationDialog = true;
      return;
    }
    navigateToImage(currentImageIndex - 1);
  }

  function goToNext() {
    if (!canGoNext) return;
    if (hasModifications(editorState)) {
      pendingNavigationDirection = 'next';
      showNavigationDialog = true;
      return;
    }
    navigateToImage(currentImageIndex + 1);
  }

  async function navigateToImage(index: number) {
    if (index < 0 || index >= folderImages.length) return;

    const newFilename = folderImages[index];
    currentImageIndex = index;

    // Get full property path
    const result = await invoke<{
      success: boolean;
      error?: string;
      data?: { full_path: string };
    }>('get_full_property_path', {
      folderPath,
      status
    });

    if (!result.success || !result.data?.full_path) {
      showError('Failed to navigate to image');
      return;
    }

    const fullPath = result.data.full_path;
    imagePath = subfolder ? `${fullPath}\\${subfolder}\\${newFilename}` : `${fullPath}\\${newFilename}`;

    // Load the new image
    try {
      const loadResult = await invoke<{ width: number; height: number; previewBase64: string }>(
        'editor_load_image',
        {
          imagePath,
          previewSize: 1000
        }
      );
      imageDimensions = { width: loadResult.width, height: loadResult.height };
      previewBase64 = loadResult.previewBase64;

      // Reset editor state
      editorState = createDefaultState();
      isEditingCrop = false;
      history = [{ state: cloneState(editorState), label: 'Initial' }];
      historyIndex = 0;

      // Update window title
      const currentWindow = getCurrentWindow();
      await currentWindow.setTitle(`Image Editor - ${newFilename}`);
    } catch (err) {
      console.error('Failed to load image:', err);
      showError(`Failed to load image: ${err}`);
    }
  }

  function confirmNavigation() {
    showNavigationDialog = false;
    if (pendingNavigationDirection === 'prev') {
      navigateToImage(currentImageIndex - 1);
    } else if (pendingNavigationDirection === 'next') {
      navigateToImage(currentImageIndex + 1);
    }
    pendingNavigationDirection = null;
  }

  function cancelNavigation() {
    showNavigationDialog = false;
    pendingNavigationDirection = null;
  }

  function handleKeydown(e: KeyboardEvent) {
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
      } else if (e.key === 'a') {
        e.preventDefault();
        applyAutoStraighten();
      } else if (e.key === 'd') {
        e.preventDefault();
        applyAutoAdjustments();
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        goToPrevious();
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        goToNext();
      }
    } else if (e.altKey) {
      if (e.key === 'ArrowLeft') {
        e.preventDefault();
        editorState.rotation.fine = Math.max(-10, editorState.rotation.fine - 0.1);
        showRotationBackground();
        pushHistory(`Rotate ${editorState.rotation.fine.toFixed(1)}°`);
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        editorState.rotation.fine = Math.min(10, editorState.rotation.fine + 0.1);
        showRotationBackground();
        pushHistory(`Rotate ${editorState.rotation.fine.toFixed(1)}°`);
      }
    } else if (e.key === 'Escape') {
      // Cancel crop if in crop editing mode, otherwise close editor
      if (isEditingCrop) {
        e.preventDefault();
        cancelCrop();
      } else {
        handleClose();
      }
    } else if (e.key === 'Enter') {
      // Apply crop when in crop editing mode
      if (isEditingCrop) {
        e.preventDefault();
        applyCrop();
      }
    } else if (e.key === 'c' || e.key === 'C') {
      // Toggle crop mode
      e.preventDefault();
      toggleCrop();
    }
  }

  let canUndo = $derived(historyIndex > 0);
  let canRedo = $derived(historyIndex < history.length - 1);
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="bg-background-0 flex h-screen flex-col">
  <!-- Custom Title Bar -->
  <EditorTitleBar
    {filename}
    dimensions={imageDimensions}
    {canUndo}
    {canRedo}
    canSave={hasModifications(editorState)}
    {isSaving}
    onUndo={undo}
    onRedo={redo}
    onReset={resetAll}
    onSave={handleSave}
    onClose={handleClose}
  />

  {#if initError}
    <!-- Error state -->
    <div class="flex flex-1 items-center justify-center">
      <div class="text-center">
        <svg
          class="mx-auto h-12 w-12 text-red-500"
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
        <p class="text-foreground-700 mt-4 text-lg">{initError}</p>
        <button
          onclick={handleClose}
          class="bg-background-200 text-foreground-700 hover:bg-background-300 mt-4 rounded px-4 py-2"
        >
          Close
        </button>
      </div>
    </div>
  {:else if !isInitialized}
    <!-- Loading state -->
    <div class="flex flex-1 items-center justify-center">
      <div class="text-foreground-500 flex flex-col items-center gap-2">
        <svg class="h-12 w-12 animate-spin" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
          ></circle>
          <path
            class="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
          ></path>
        </svg>
        <span>Loading image...</span>
      </div>
    </div>
  {:else}
    <!-- Main content -->
    <div class="flex flex-1 overflow-hidden">
      <!-- Preview area - WebGL canvas fills the space -->
      <div bind:this={canvasContainer} class="relative flex-1 overflow-hidden">
        {#if previewBase64}
          <WebGLCanvas
            imageBase64={previewBase64}
            brightness={editorState.adjustments.brightness}
            exposure={editorState.adjustments.exposure}
            contrast={editorState.adjustments.contrast}
            highlights={editorState.adjustments.highlights}
            shadows={editorState.adjustments.shadows}
            rotation={editorState.rotation.fine}
            quarterTurns={editorState.rotation.quarterTurns}
            showRotatedBackground={isRotating}
            cropEnabled={editorState.crop.enabled && !isEditingCrop}
            cropX={editorState.crop.x}
            cropY={editorState.crop.y}
            cropWidth={editorState.crop.width}
            cropHeight={editorState.crop.height}
            bind:zoom={canvasZoom}
            bind:panX={canvasPanX}
            bind:panY={canvasPanY}
            bind:fitScalePercent={canvasFitScale}
            bind:canvasImageWidth={canvasImageWidth}
            bind:canvasImageHeight={canvasImageHeight}
          />
          {#if isEditingCrop}
            <CropOverlay
              cropX={editorState.crop.x}
              cropY={editorState.crop.y}
              cropWidth={editorState.crop.width}
              cropHeight={editorState.crop.height}
              zoom={canvasZoom}
              panX={canvasPanX}
              panY={canvasPanY}
              fitScalePercent={canvasFitScale}
              imageWidth={canvasImageWidth}
              imageHeight={canvasImageHeight}
              {containerWidth}
              {containerHeight}
              rotation={editorState.rotation.fine}
              onCropChange={handleCropChange}
              onCropCommit={handleCropCommit}
            />
          {/if}
          <!-- Navigation arrows -->
          {#if folderImages.length > 1}
            <button
              onclick={goToPrevious}
              disabled={!canGoPrevious}
              class="absolute left-4 top-1/2 -translate-y-1/2 rounded-sm bg-black/70 p-3 text-white transition-opacity hover:bg-black/85 disabled:cursor-not-allowed disabled:opacity-30"
              title="Previous image (Ctrl+Left)"
              aria-label="Previous image"
            >
              <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
              </svg>
            </button>
            <button
              onclick={goToNext}
              disabled={!canGoNext}
              class="absolute right-4 top-1/2 -translate-y-1/2 rounded-sm bg-black/70 p-3 text-white transition-opacity hover:bg-black/85 disabled:cursor-not-allowed disabled:opacity-30"
              title="Next image (Ctrl+Right)"
              aria-label="Next image"
            >
              <svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
              </svg>
            </button>
            <!-- Image counter -->
            <div class="absolute right-4 top-4 rounded-sm bg-black/70 px-2 py-1 text-xs text-white">
              {currentImageIndex + 1} / {folderImages.length}
            </div>
          {/if}
          <!-- Bottom controls overlay -->
          <div class="absolute bottom-4 left-1/2 flex -translate-x-1/2 items-center gap-2 rounded-sm bg-black/70 p-1.5">
            <!-- Crop controls -->
            {#if isEditingCrop}
              <!-- Apply crop button -->
              <button
                onclick={applyCrop}
                class="flex items-center gap-1 rounded-sm bg-green-600 px-3 py-1.5 text-white hover:bg-green-700"
                title="Apply crop (Enter)"
              >
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                </svg>
                <span class="text-xs font-medium">Apply</span>
              </button>
              <!-- Cancel crop button -->
              <button
                onclick={cancelCrop}
                class="flex items-center gap-1 rounded-sm px-3 py-1.5 text-white hover:bg-white/20"
                title="Cancel crop (Esc)"
              >
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                </svg>
                <span class="text-xs font-medium">Cancel</span>
              </button>
            {:else}
              <!-- Crop button -->
              <button
                onclick={toggleCrop}
                class="flex items-center gap-1 rounded-sm px-2 py-1.5 text-white hover:bg-white/20"
                title="Enable crop"
              >
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v16h16M4 8h12v12" />
                </svg>
                <span class="text-xs font-medium">Crop</span>
              </button>
            {/if}
            <!-- Separator -->
            <div class="mx-1 h-5 w-px bg-white/30"></div>
            <!-- Rotate CCW button -->
            <button
              onclick={() => rotate90(-1)}
              class="flex items-center gap-0.5 rounded-sm px-2 py-1.5 text-white hover:bg-white/20"
              title="Rotate 90° counter-clockwise"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 5l-7 7 7 7" />
              </svg>
              <span class="text-xs font-medium">90</span>
            </button>
            <!-- Fine rotation slider -->
            <div class="flex items-center gap-3">
              <input
                type="range"
                min="-10"
                max="10"
                step="0.05"
                value={editorState.rotation.fine}
                oninput={(e) => {
                  showRotationBackground();
                  handleRotationChange(parseFloat(e.currentTarget.value));
                }}
                onchange={handleRotationCommit}
                class="h-2 w-80 cursor-pointer appearance-none rounded-full bg-white/30 [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white"
              />
              <span class="min-w-12 text-center text-sm text-white">{editorState.rotation.fine.toFixed(1)}°</span>
            </div>
            <!-- Rotate CW button -->
            <button
              onclick={() => rotate90(1)}
              class="flex items-center gap-0.5 rounded-sm px-2 py-1.5 text-white hover:bg-white/20"
              title="Rotate 90° clockwise"
            >
              <span class="text-xs font-medium">90</span>
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
              </svg>
            </button>
            <!-- Separator -->
            <div class="mx-1 h-5 w-px bg-white/30"></div>
            <!-- Auto Straighten button -->
            <button
              onclick={applyAutoStraighten}
              disabled={isAutoStraightening}
              class="flex items-center gap-1 rounded-sm px-2 py-1.5 text-white hover:bg-white/20 disabled:opacity-50"
              title="Auto straighten (Ctrl+A)"
            >
              {#if isAutoStraightening}
                <svg class="h-4 w-4 animate-spin" fill="none" viewBox="0 0 24 24">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3"></circle>
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
                </svg>
              {:else}
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
                </svg>
              {/if}
              <span class="text-xs font-medium">Auto</span>
            </button>
          </div>
        {:else}
          <div class="bg-background-900 flex h-full w-full items-center justify-center">
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
              <span>Loading preview...</span>
            </div>
          </div>
        {/if}
      </div>

      <!-- Right sidebar: Tools -->
      <div class="border-background-200 bg-background-0 flex w-72 flex-col border-l">
        <!-- Tool tabs -->
        <div class="border-background-200 flex border-b">
          <button
            onclick={() => (activeTool = 'crop')}
            class="flex-1 px-3 py-2 text-sm font-medium transition-colors {activeTool === 'crop'
              ? 'text-accent-600 border-accent-500 border-b'
              : 'text-foreground-600 hover:bg-background-100'}"
          >
            Crop
          </button>
          <button
            onclick={() => (activeTool = 'adjust')}
            class="flex-1 px-3 py-2 text-sm font-medium transition-colors {activeTool === 'adjust'
              ? 'text-accent-600 border-accent-500 border-b'
              : 'text-foreground-600 hover:bg-background-100'}"
          >
            Adjust
          </button>
        </div>

        <!-- Tool options -->
        <div class="flex-1 overflow-y-auto p-4">
          {#if activeTool === 'crop'}
            <div class="space-y-4">
              {#if isEditingCrop}
                <div class="rounded bg-green-600/10 p-3">
                  <p class="text-sm font-medium text-green-700">Editing Crop</p>
                  <p class="text-foreground-500 mt-1 text-xs">
                    Drag corners or edges to adjust. Press Apply to keep, Cancel to discard.
                  </p>
                </div>

                <div class="text-foreground-600 space-y-1 text-xs">
                  <p>X: {(editorState.crop.x * 100).toFixed(1)}%</p>
                  <p>Y: {(editorState.crop.y * 100).toFixed(1)}%</p>
                  <p>Width: {(editorState.crop.width * 100).toFixed(1)}%</p>
                  <p>Height: {(editorState.crop.height * 100).toFixed(1)}%</p>
                </div>
              {:else if editorState.crop.enabled}
                <div class="rounded bg-blue-600/10 p-3">
                  <p class="text-sm font-medium text-blue-700">Crop Applied</p>
                  <p class="text-foreground-500 mt-1 text-xs">
                    Crop will be saved when you click Save. Click Crop button to edit again.
                  </p>
                </div>

                <div class="text-foreground-600 space-y-1 text-xs">
                  <p>X: {(editorState.crop.x * 100).toFixed(1)}%</p>
                  <p>Y: {(editorState.crop.y * 100).toFixed(1)}%</p>
                  <p>Width: {(editorState.crop.width * 100).toFixed(1)}%</p>
                  <p>Height: {(editorState.crop.height * 100).toFixed(1)}%</p>
                </div>
              {:else}
                <p class="text-foreground-500 text-sm">
                  Click the Crop button in the bottom bar to start cropping.
                </p>
              {/if}
            </div>
          {:else if activeTool === 'adjust'}
            <div class="space-y-4">
              <!-- Auto button -->
              <button
                onclick={applyAutoAdjustments}
                class="bg-accent-500 hover:bg-accent-600 w-full rounded px-4 py-2 text-sm font-medium text-white transition-colors"
                title="Auto adjust (Ctrl+D)"
              >
                Auto
              </button>

              {#each [
                { key: 'brightness', label: 'Brightness', icon: 'sun' },
                { key: 'exposure', label: 'Exposure', icon: 'aperture' },
                { key: 'contrast', label: 'Contrast', icon: 'contrast' },
                { key: 'highlights', label: 'Highlights', icon: 'highlights' },
                { key: 'shadows', label: 'Shadows', icon: 'shadows' }
              ] as { key, label, icon } (key)}
                {@const adjustKey = key as keyof EditorState['adjustments']}
                <div>
                  <div class="mb-1 flex items-center justify-between">
                    <span class="text-foreground-700 flex items-center gap-2 text-sm font-medium">
                      {#if icon === 'sun'}
                        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <circle cx="12" cy="12" r="4" stroke-width="2" />
                          <path stroke-width="2" d="M12 2v2m0 16v2M4.93 4.93l1.41 1.41m11.32 11.32l1.41 1.41M2 12h2m16 0h2M6.34 17.66l-1.41 1.41m12.73-12.73l1.41-1.41" />
                        </svg>
                      {:else if icon === 'aperture'}
                        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <circle cx="12" cy="12" r="10" stroke-width="2" />
                          <path stroke-width="2" d="M14.31 8l5.74 9.94M9.69 8h11.48M7.38 12l5.74-9.94M9.69 16L3.95 6.06M14.31 16H2.83M16.62 12l-5.74 9.94" />
                        </svg>
                      {:else if icon === 'contrast'}
                        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <circle cx="12" cy="12" r="10" stroke-width="2" />
                          <path stroke-width="2" fill="currentColor" d="M12 2a10 10 0 0 1 0 20V2z" />
                        </svg>
                      {:else if icon === 'highlights'}
                        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-width="2" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707" />
                          <circle cx="12" cy="12" r="4" stroke-width="2" fill="currentColor" fill-opacity="0.3" />
                        </svg>
                      {:else if icon === 'shadows'}
                        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <circle cx="12" cy="12" r="10" stroke-width="2" />
                          <path stroke-width="2" fill="currentColor" fill-opacity="0.5" d="M12 2a10 10 0 0 0 0 20 7 7 0 0 1 0-20z" />
                        </svg>
                      {/if}
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
    </div>
  {/if}
</div>

<!-- Unsaved changes confirmation dialog -->
{#if showUnsavedDialog}
  <ConfirmDialog
    bind:open={showUnsavedDialog}
    title="Unsaved Changes"
    message="You have unsaved changes. Are you sure you want to close without saving?"
    confirmText="Close Without Saving"
    cancelText="Cancel"
    destructive={true}
    onConfirm={confirmClose}
    onCancel={cancelClose}
  />
{/if}

<!-- Navigation confirmation dialog -->
{#if showNavigationDialog}
  <ConfirmDialog
    bind:open={showNavigationDialog}
    title="Unsaved Changes"
    message="You have unsaved changes. Are you sure you want to navigate to another image without saving?"
    confirmText="Discard Changes"
    cancelText="Cancel"
    destructive={true}
    onConfirm={confirmNavigation}
    onCancel={cancelNavigation}
  />
{/if}
