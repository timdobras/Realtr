<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { beforeNavigate } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { dndzone } from 'svelte-dnd-action';
  import { flip } from 'svelte/animate';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
  import PerspectiveCorrectionModal from '$lib/components/PerspectiveCorrectionModal.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { showSuccess, showError } from '$lib/stores/notification';
  export const prerender = false;

  interface ImageItem {
    id: string; // Use filename as stable ID
    filename: string;
    dataUrl: string;
    loading: boolean;
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
  let showPerspectiveModal = $state(false);
  let showRenameConfirm = $state(false);

  // Track unsaved reorder changes
  let originalOrder: string[] = $state([]);
  let hasUnsavedChanges = $state(false);
  let showUnsavedDialog = $state(false);
  let pendingNavigation: (() => void) | null = $state(null);

  // Get the id from the URL params
  let propertyId = $derived(Number($page.params.id));

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
    if (!propertyId) {
      error = 'Invalid property ID';
      loading = false;
      return;
    }

    try {
      loading = true;
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
          // Extract numeric part from filename
          const getNumericValue = (filename: string): number => {
            const match = filename.match(/^(\d+)/);
            return match ? parseInt(match[1]) : Infinity;
          };

          const numA = getNumericValue(a);
          const numB = getNumericValue(b);

          return numA - numB;
        });

        internetImages = sortedFilenames.map((filename, index) => ({
          id: filename, // Use filename as stable ID
          filename,
          dataUrl: '',
          loading: true,
          newName: baseFileName ? `${baseFileName}_${index + 1}` : `${index + 1}`
        }));

        // Save original order for change detection
        originalOrder = sortedFilenames.slice();
        hasUnsavedChanges = false;

        // Load thumbnails
        for (let i = 0; i < internetImages.length; i++) {
          const image = internetImages[i];
          try {
            const base64Data = await invoke('get_internet_image_as_base64', {
              folderPath: property.folder_path,
              status: property.status,
              filename: image.filename
            });

            const ext = image.filename.split('.').pop()?.toLowerCase() || '';
            const mimeType = getMimeType(ext);

            internetImages[i] = {
              ...image,
              dataUrl: `data:${mimeType};base64,${base64Data}`,
              loading: false
            };
          } catch (e) {
            internetImages[i] = { ...image, dataUrl: '', loading: false };
          }
        }
      }
    } catch (e) {
      error = `Failed to load images: ${e}`;
    }
  }

  function getMimeType(ext: string): string {
    return ['jpg', 'jpeg'].includes(ext)
      ? 'image/jpeg'
      : ext === 'png'
        ? 'image/png'
        : ext === 'gif'
          ? 'image/gif'
          : ext === 'webp'
            ? 'image/webp'
            : ext === 'bmp'
              ? 'image/bmp'
              : 'image/jpeg';
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

  function applyRenaming() {
    if (!property || internetImages.length === 0) return;
    showRenameConfirm = true;
  }

  async function doRenaming() {
    if (!property) return;
    showRenameConfirm = false;

    try {
      renamingImages = true;
      dragDisabled = true; // Disable drag during rename

      const renameMap = internetImages.map((image, index) => {
        const extension = image.filename.split('.').pop() || 'jpg';
        return {
          old_name: image.filename,
          new_name: `${image.newName}.${extension}`
        };
      });

      const result: any = await invoke('rename_internet_images', {
        folderPath: property.folder_path,
        status: property.status,
        renameMap
      });

      if (result.success) {
        await loadInternetImages();
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

  let renameConfirmMessage = $derived(
    `This will rename ${internetImages.length} images. This action cannot be undone.\n\nExamples:\n${internetImages
      .slice(0, 3)
      .map((img) => `${img.filename} → ${img.newName}.${img.filename.split('.').pop()}`)
      .join('\n')}`
  );

  async function openImageInEditor(filename: string, event: any) {
    // Prevent opening during drag
    if (isDragging) {
      event.preventDefault();
      return;
    }

    if (!property) return;

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

  function openPerspectiveModal() {
    showPerspectiveModal = true;
  }

  function closePerspectiveModal() {
    showPerspectiveModal = false;
  }

  async function handlePerspectiveComplete() {
    showPerspectiveModal = false;
    // Reload images after perspective corrections
    await loadInternetImages();
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
            <!-- Auto-Straighten Button -->
            <button
              onclick={openPerspectiveModal}
              disabled={renamingImages || internetImages.length === 0 || isDragging}
              class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 flex items-center space-x-2 border px-4 py-3 font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
              title="Auto-straighten images using perspective correction"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4"
                />
              </svg>
              <span>Auto-Straighten</span>
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
                  <button
                    class="bg-background-100 flex aspect-square w-full items-center justify-center"
                    onclick={(e) => openImageInEditor(image.filename, e)}
                    disabled={isDragging}
                  >
                    {#if image.loading}
                      <div
                        class="border-foreground-300 h-4 w-4 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                    {:else if image.dataUrl}
                      <img
                        src={image.dataUrl}
                        alt={image.filename}
                        loading="lazy"
                        class="h-full w-full object-cover"
                      />
                    {:else}
                      <div class="text-xs text-red-700">Failed</div>
                    {/if}
                  </button>

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

  <!-- Perspective Correction Modal -->
  {#if showPerspectiveModal && property}
    <PerspectiveCorrectionModal
      folderPath={property.folder_path}
      status={property.status}
      propertyId={property.id ?? 0}
      onClose={closePerspectiveModal}
      onComplete={handlePerspectiveComplete}
    />
  {/if}

  <!-- Rename Confirmation Dialog -->
  {#if showRenameConfirm}
    <ConfirmDialog
      title="Rename Images"
      message={renameConfirmMessage}
      confirmText="Rename"
      destructive={true}
      onConfirm={doRenaming}
      onCancel={() => (showRenameConfirm = false)}
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
