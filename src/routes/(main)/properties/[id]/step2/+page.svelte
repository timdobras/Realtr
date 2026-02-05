<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { page } from '$app/stores';
  import { beforeNavigate } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { dndzone } from 'svelte-dnd-action';
  import { flip } from 'svelte/animate';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property, AppConfig } from '$lib/types/database';
  import BatchEnhanceModal from '$lib/components/BatchEnhanceModal.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import LazyImage from '$lib/components/LazyImage.svelte';
  import { showSuccess, showError } from '$lib/stores/notification';
  import { openEditorWindow } from '$lib/utils/editorWindow';
  export const prerender = false;

  interface ImageItem {
    id: string; // Use filename as stable ID
    filename: string;
    newName?: string;
  }

  let property: Property | null = $state(null);
  let internetImages: ImageItem[] = $state([]);
  let error = $state('');
  let loading = $state(true);
  let renamingImages = $state(false);
  let baseFileName = $state('');
  let dragDisabled = $state(false);
  let isDragging = $state(false); // Track drag state
  let showEnhanceModal = $state(false);
  let imageRefreshKey = $state(0); // Increment to force image refresh

  // Image editor setting
  let useBuiltinEditor = $state(true); // Default to built-in editor

  // Track unsaved reorder changes
  let originalOrder: string[] = $state([]);
  let hasUnsavedChanges = $state(false);
  let showUnsavedDialog = $state(false);
  let pendingNavigation: (() => void) | null = $state(null);

  // Get the id from the URL params
  let propertyId = $derived(Number($page.params.id));

  // Refresh images when window regains focus (user returns from external editor)
  function handleWindowFocus() {
    imageRefreshKey++;
  }

  // Check if current order differs from original
  function checkForChanges() {
    if (originalOrder.length === 0) return false;
    const currentOrder = internetImages.map((img) => img.id);
    if (currentOrder.length !== originalOrder.length) return false;
    return currentOrder.some((id, index) => id !== originalOrder[index]);
  }

  // Intercept navigation when there are unsaved changes
  beforeNavigate(({ cancel, to }) => {
    if (hasUnsavedChanges && to) {
      cancel();
      pendingNavigation = () => {
        hasUnsavedChanges = false;
        window.location.href = to.url.pathname;
      };
      showUnsavedDialog = true;
    }
  });

  onMount(async () => {
    // Listen for window focus to refresh images
    window.addEventListener('focus', handleWindowFocus);

    if (!propertyId) {
      error = 'Invalid property ID';
      loading = false;
      return;
    }

    try {
      loading = true;

      // Load config to check editor preference
      const config = await invoke<AppConfig | null>('load_config');
      if (config) {
        useBuiltinEditor = config.use_builtin_editor ?? true;
      }

      property = await DatabaseService.getPropertyById(propertyId);
      if (!property) {
        error = 'Property not found';
        loading = false;
        return;
      }

      await loadInternetImages();
    } catch (e) {
      error = `Failed to load property: ${e}`;
    } finally {
      loading = false;
    }
  });

  onDestroy(() => {
    window.removeEventListener('focus', handleWindowFocus);
  });

  async function loadInternetImages() {
    if (!property) return;

    try {
      const response = await invoke('list_internet_images', {
        folderPath: property.folder_path,
        status: property.status
      });

      if (Array.isArray(response)) {
        // Sort filenames numerically before processing
        const sortedFilenames = response.sort((a: string, b: string) => {
          // Extract numeric part from filename - try start first, then anywhere
          const getNumericValue = (filename: string): number => {
            // First try to match number at start (e.g., "1.jpg", "02.jpg")
            const startMatch = filename.match(/^(\d+)/);
            if (startMatch) return parseInt(startMatch[1]);

            // Then try to match number after underscore or at end (e.g., "house_1.jpg")
            const underscoreMatch = filename.match(/_(\d+)\./);
            if (underscoreMatch) return parseInt(underscoreMatch[1]);

            // Finally try any number in filename
            const anyMatch = filename.match(/(\d+)/);
            if (anyMatch) return parseInt(anyMatch[1]);

            return Infinity;
          };

          const numA = getNumericValue(a);
          const numB = getNumericValue(b);

          // If both have same numeric value (or both Infinity), sort alphabetically
          if (numA === numB) {
            return a.localeCompare(b);
          }

          return numA - numB;
        });

        // Just store filenames - LazyImage handles loading
        internetImages = sortedFilenames.map((filename, index) => ({
          id: filename, // Use filename as stable ID
          filename,
          newName: baseFileName ? `${baseFileName}_${index + 1}` : `${index + 1}`
        }));

        // Save original order for change detection
        originalOrder = sortedFilenames.slice();
        hasUnsavedChanges = false;

        // Pre-generate thumbnails in parallel for faster display
        DatabaseService.pregenerateGalleryThumbnails(
          property.folder_path,
          property.status,
          'INTERNET'
        );
      }
    } catch (e) {
      error = `Failed to load images: ${e}`;
    }
  }

  // Improved drag and drop handlers
  function handleDndConsider(e: any) {
    isDragging = true;
    dragDisabled = false;
    internetImages = e.detail.items;
    // Don't update names during dragging to reduce re-renders
  }

  function handleDndFinalize(e: any) {
    isDragging = false;
    dragDisabled = false;
    internetImages = e.detail.items;
    // Update names only after drag is complete
    updateNewNames();
    // Check if order changed from original
    hasUnsavedChanges = checkForChanges();
  }

  // Debounced name update function
  let updateTimeout: ReturnType<typeof setTimeout> | undefined;
  function updateNewNames() {
    clearTimeout(updateTimeout);
    updateTimeout = setTimeout(() => {
      internetImages = internetImages.map((image, index) => ({
        ...image,
        newName: baseFileName ? `${baseFileName}_${index + 1}` : `${index + 1}`
      }));
    }, 50); // Small delay to batch updates
  }

  async function applyRenaming() {
    if (!property || internetImages.length === 0) return;

    try {
      renamingImages = true;
      dragDisabled = true; // Disable drag during rename

      const renameMap = internetImages.map((image, index) => {
        const extension = image.filename.split('.').pop() || 'jpg';
        // Calculate newName directly from index to avoid race condition with debounced updateNewNames
        const newName = baseFileName ? `${baseFileName}_${index + 1}` : `${index + 1}`;
        return {
          old_name: image.filename,
          new_name: `${newName}.${extension}`
        };
      });

      const result: any = await invoke('rename_internet_images', {
        folderPath: property.folder_path,
        status: property.status,
        renameMap
      });

      if (result.success) {
        await loadInternetImages();
        imageRefreshKey++; // Force all thumbnails to reload with new filenames
        showSuccess(`Successfully renamed ${internetImages.length} images`);
      } else {
        showError(result.error || 'Failed to rename images');
      }
    } catch (e) {
      showError(`Failed to rename images: ${e}`);
    } finally {
      renamingImages = false;
      dragDisabled = false;
    }
  }

  async function openImageInEditor(filename: string, event: any) {
    // Prevent opening during drag
    if (isDragging) {
      event.preventDefault();
      return;
    }

    if (!property) return;

    // Check if we should use the built-in editor (opens in new window)
    if (useBuiltinEditor) {
      await openEditorWindow({
        folderPath: property.folder_path,
        status: property.status,
        subfolder: 'INTERNET',
        filename
      });
      return;
    }

    // Otherwise, use external editor
    try {
      const result: any = await invoke('open_image_in_editor', {
        folderPath: property.folder_path,
        status: property.status,
        filename,
        isFromInternet: true
      });

      if (!result.success) {
        error = result.error || 'Failed to open image in editor';
      }
    } catch (e) {
      error = `Failed to open image: ${e}`;
    }
  }

  function openEnhanceModal() {
    showEnhanceModal = true;
  }

  function closeEnhanceModal() {
    showEnhanceModal = false;
  }

  async function handleEnhanceComplete() {
    showEnhanceModal = false;
    // Reload images after enhancements applied
    await loadInternetImages();
    imageRefreshKey++; // Force thumbnail refresh
  }
</script>

{#if loading}
  <div class="flex h-64 items-center justify-center">
    <div class="text-foreground-500 flex items-center gap-2 text-sm">
      <div
        class="border-foreground-300 h-4 w-4 animate-spin rounded-full border-2 border-t-transparent"
      ></div>
      <span>Loading...</span>
    </div>
  </div>
{:else if error}
  <div class="p-8">
    <div class="rounded-lg border border-red-300 bg-red-50 px-4 py-3">
      <p class="text-sm text-red-800">{error}</p>
    </div>
  </div>
{:else if property}
  <div class="min-h-full space-y-5 p-6">
    <!-- Step Header -->
    <!-- <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-4 flex items-center space-x-4">
        <div class="bg-accent-100 flex h-12 w-12 items-center justify-center rounded-lg">
          <svg
            class="text-accent-600 h-6 w-6"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4"
            />
          </svg>
        </div>
        <div>
          <h1 class="text-foreground-900 text-2xl font-bold">Step 2: Order & Rename</h1>
          <p class="text-foreground-600">
            Arrange your images in the desired order and rename them
          </p>
        </div>
      </div>

      <div class="bg-background-100 border-background-200 rounded-lg border p-4">
        <div class="flex items-start space-x-3">
          <svg
            class="text-accent-600 mt-0.5 h-5 w-5"
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
          <div>
            <p class="text-foreground-700 mb-1 text-sm font-medium">How this step works:</p>
            <ul class="text-foreground-600 space-y-1 text-sm">
              <li>• Drag images to reorder them or use arrow buttons</li>
              <li>• Set individual names for each image</li>
              <li>• Use base filename to apply consistent naming</li>
              <li>• Apply renaming when satisfied with the order</li>
            </ul>
          </div>
        </div>
      </div>
    </div> -->

    <!-- Naming Controls -->
    <!-- <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-4 flex items-center space-x-3">
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
              d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 100 4m0-4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 100 4m0-4v2m0-6V4"
            />
          </svg>
        </div>
        <div>
          <h2 class="text-foreground-900 text-lg font-semibold">Naming Options</h2>
          <p class="text-foreground-600 text-sm">Set up consistent naming for your images</p>
        </div>
      </div>

      <div class="grid grid-cols-1 gap-6 md:grid-cols-2">
        <div>
          <label class="text-foreground-700 mb-2 block text-sm font-medium">
            Base Filename (Optional)
          </label>
          <input
            type="text"
            bind:value={baseFileName}
            oninput={updateNewNamesFromBase}
            placeholder="e.g., property, apartment, house"
            class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full rounded-lg border px-4 py-3 transition-colors focus:ring-2 focus:outline-none"
            disabled={isDragging || renamingImages}
          />
          <p class="text-foreground-500 mt-2 text-xs">
            Images will be named: {baseFileName || 'image'}_1, {baseFileName || 'image'}_2, etc.
          </p>
        </div>

        <div class="flex items-end">
          <button
            onclick={autoNumberAll}
            disabled={internetImages.length === 0 || isDragging || renamingImages}
            class="bg-background-200 text-foreground-700 hover:bg-background-300 w-full rounded-lg px-4 py-3 font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            Auto-Number All Images
          </button>
        </div>
      </div>
    </div> -->

    <!-- Action Bar -->
    {#if internetImages.length > 0}
      <div class="bg-background-50 border-background-200 border p-4">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-foreground-900 font-semibold">Ready to apply changes?</p>
            <p class="text-foreground-600 text-sm">
              Rename {internetImages.length} images with your new naming scheme
            </p>
          </div>

          <div class="flex items-center space-x-3">
            <!-- Auto-Enhance Button -->
            <button
              onclick={openEnhanceModal}
              disabled={renamingImages || internetImages.length === 0 || isDragging}
              class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 flex items-center space-x-2 border px-4 py-3 font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
              title="Auto-enhance images (straighten + brightness/exposure/contrast)"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.143L13 21l-2.286-6.857L5 12l5.714-2.143L13 3z"
                />
              </svg>
              <span>Auto-Enhance</span>
            </button>

            <button
              onclick={applyRenaming}
              disabled={renamingImages || internetImages.length === 0 || isDragging}
              class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-3 px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              {#if renamingImages}
                <div class="h-4 w-4 animate-spin border-2 border-white border-t-transparent"></div>
                <span>Renaming...</span>
              {:else}
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M9 12l2 2 4-4"
                  />
                </svg>
                <span>Apply Renaming</span>
              {/if}
            </button>
          </div>
        </div>
      </div>
    {/if}

    <!-- Images Grid -->
    <section class="bg-background-50 border-background-200 border">
      <div class="p-4">
        <div class="mb-4 flex items-center justify-between">
          <div>
            <h2 class="text-foreground-900 text-lg font-semibold">
              Image Gallery ({internetImages.length})
            </h2>
            <p class="text-foreground-600 text-sm">
              {isDragging ? 'Dragging...' : 'Drag images to reorder'}
            </p>
          </div>

          {#if internetImages.length > 0}
            <div class="text-foreground-500 flex items-center space-x-2 text-sm">
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M7 16V4m0 0L3 8m4-4l4 4"
                />
              </svg>
              <span>Drag & Drop Enabled</span>
            </div>
          {/if}
        </div>

        {#if internetImages.length === 0}
          <div class="py-10 text-center">
            <h3 class="text-foreground-900 mb-2 text-lg font-semibold">
              No images in INTERNET folder
            </h3>
            <p class="text-foreground-500 mx-auto mb-5 max-w-md">
              You need to copy images from Step 1 before you can order and rename them.
            </p>
            <a
              href="/properties/{property.id}/step1"
              class="bg-accent-500 hover:bg-accent-600 inline-flex items-center space-x-2 px-6 py-3 font-medium text-white transition-colors"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M15 19l-7-7 7-7"
                />
              </svg>
              <span>Back to Step 1</span>
            </a>
          </div>
        {:else}
          <div
            class="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6"
            use:dndzone={{
              items: internetImages,
              dragDisabled,
              flipDurationMs: isDragging ? 0 : 150,
              dropTargetStyle: {}
            }}
            onconsider={handleDndConsider}
            onfinalize={handleDndFinalize}
          >
            {#each internetImages as image, index (image.id)}
              <div
                class="bg-background-100 border-background-200 overflow-hidden border"
                animate:flip={{ duration: isDragging ? 0 : 150 }}
              >
                <!-- Image Preview -->
                <div class="relative">
                  <LazyImage
                    folderPath={property.folder_path}
                    status={property.status}
                    subfolder="INTERNET"
                    filename={image.filename}
                    alt={image.filename}
                    class="aspect-square w-full cursor-pointer"
                    onclick={() =>
                      !isDragging &&
                      openImageInEditor(image.filename, { preventDefault: () => {} })}
                    refreshKey={imageRefreshKey}
                  />

                  <!-- Order Badge -->
                  <div
                    class="bg-accent-500 absolute top-2 left-2 flex h-6 w-6 items-center justify-center text-xs font-semibold text-white"
                  >
                    {index + 1}
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    <!-- Next Step Navigation -->
    <div class="bg-background-50 border-background-200 border p-4">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-foreground-900 font-semibold">Ready for the next step?</h3>
          <p class="text-foreground-600 text-sm">
            Once you've ordered and renamed your images, proceed to Step 3 for copying to AGGELIA.
          </p>
        </div>

        <a
          href="/properties/{property.id}/step3"
          class="inline-flex items-center space-x-2 px-6 py-3 font-medium transition-colors {internetImages.length ===
          0
            ? 'bg-background-200 text-foreground-500 cursor-not-allowed'
            : 'bg-accent-500 hover:bg-accent-600 text-white'}"
          class:pointer-events-none={internetImages.length === 0}
        >
          <span>Step 3: Copy to AGGELIA</span>
          <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M9 5l7 7-7 7"
            />
          </svg>
        </a>
      </div>
    </div>
  </div>

  <!-- Batch Enhance Modal -->
  {#if showEnhanceModal && property}
    <BatchEnhanceModal
      folderPath={property.folder_path}
      status={property.status}
      onClose={closeEnhanceModal}
      onComplete={handleEnhanceComplete}
    />
  {/if}

  <!-- Unsaved Changes Dialog -->
  {#if showUnsavedDialog}
    <ConfirmDialog
      title="Unsaved Changes"
      message="You have reordered images but haven't applied the rename. Your changes will be lost if you leave this page."
      confirmText="Leave Page"
      cancelText="Stay"
      destructive={true}
      onConfirm={() => {
        showUnsavedDialog = false;
        if (pendingNavigation) pendingNavigation();
      }}
      onCancel={() => {
        showUnsavedDialog = false;
        pendingNavigation = null;
      }}
    />
  {/if}
{/if}
