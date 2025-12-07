<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
  import { showSuccess, showError } from '$lib/stores/notification';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import LazyImage from '$lib/components/LazyImage.svelte';

  // Just store filenames - LazyImage handles loading
  let property: Property | null = $state(null);
  let watermarkFilenames: string[] = $state([]);
  let watermarkAggeliaFilenames: string[] = $state([]);
  let error = $state('');
  let loading = $state(true);
  let processingWatermarksVar = $state(false);
  let watermarkConfig: {
    imagePath?: string;
    opacity: number;
    sizeMode: string;
    positionAnchor: string;
  } | null = $state(null);
  let processingStatus = $state('');
  let imagesToProcess = $state(0);
  let showWatermarkConfirm = $state(false);
  let showClearConfirm = $state(false);
  let fillingTo25 = $state(false);

  // Get the id from the URL params
  let propertyId = $derived(Number($page.params.id));

  // Helper function for numeric filename sorting
  function sortImagesByNumericFilename(filenames: string[]): string[] {
    return filenames.sort((a, b) => {
      const getNumericValue = (filename: string): number => {
        const match = filename.match(/^(\d+)/);
        return match ? parseInt(match[1]) : Infinity;
      };

      const numA = getNumericValue(a);
      const numB = getNumericValue(b);

      if (numA !== Infinity && numB !== Infinity) {
        return numA - numB;
      } else {
        return a.localeCompare(b);
      }
    });
  }

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

      await loadWatermarkConfig();
      await loadWatermarkImages();
    } catch (e) {
      error = `Failed to load property: ${e}`;
    } finally {
      loading = false;
    }
  });

  async function loadWatermarkConfig() {
    try {
      const config: any = await invoke('load_config');
      if (config) {
        watermarkConfig = {
          imagePath: config.watermark_image_path,
          opacity: config.watermarkConfig?.opacity || config.watermark_opacity || 0.15,
          sizeMode: config.watermarkConfig?.sizeMode || 'proportional',
          positionAnchor: config.watermarkConfig?.positionAnchor || 'center'
        };
      }
    } catch (e) {
      console.error('Failed to load watermark config:', e);
    }
  }

  async function loadWatermarkImages() {
    if (!property) return;

    try {
      // Load WATERMARK images - just filenames, LazyImage handles loading
      const watermarkList = await invoke('list_watermark_images', {
        folderPath: property.folder_path,
        status: property.status
      });

      if (Array.isArray(watermarkList)) {
        watermarkFilenames = sortImagesByNumericFilename(watermarkList);
      }

      // Load WATERMARK/AGGELIA images - just filenames
      const aggeliaList = await invoke('list_watermark_aggelia_images', {
        folderPath: property.folder_path,
        status: property.status
      });

      if (Array.isArray(aggeliaList)) {
        watermarkAggeliaFilenames = sortImagesByNumericFilename(aggeliaList);
      }

      // Pre-generate thumbnails in parallel for faster display
      pregenerateThumbnails();
    } catch (e) {
      watermarkFilenames = [];
      watermarkAggeliaFilenames = [];
    }
  }

  // Pre-generate thumbnails in background (don't await - fire and forget)
  function pregenerateThumbnails() {
    if (!property) return;
    // Pre-generate WATERMARK folder thumbnails
    DatabaseService.pregenerateGalleryThumbnails(property.folder_path, property.status, 'WATERMARK');
    DatabaseService.pregenerateGalleryThumbnails(property.folder_path, property.status, 'WATERMARK/AGGELIA');
  }

  function applyWatermarksToAllImages() {
    if (!property || !watermarkConfig?.imagePath) {
      error = 'Watermark not configured. Please set up watermark in settings first.';
      return;
    }

    showWatermarkConfirm = true;
  }

  async function doApplyWatermarks() {
    if (!property) return;
    showWatermarkConfirm = false;

    try {
      processingWatermarksVar = true;
      error = '';

      // Get count of images to process
      const internetImages: string[] = await invoke('list_internet_images', {
        folderPath: property.folder_path,
        status: property.status
      });
      const aggeliaImages: string[] = await invoke('list_aggelia_images', {
        folderPath: property.folder_path,
        status: property.status
      });

      imagesToProcess = internetImages.length + aggeliaImages.length;
      processingStatus = `Processing ${imagesToProcess} images...`;

      const result: any = await invoke('copy_and_watermark_images', {
        folderPath: property.folder_path,
        status: property.status
      });

      if (result.success) {
        const processedCount = result.data?.processed_count || imagesToProcess;
        processingStatus = `Completed! ${processedCount} images watermarked.`;
        await loadWatermarkImages();
        showSuccess(`Successfully watermarked ${processedCount} images`);
      } else {
        showError(result.error || 'Failed to apply watermarks');
      }
    } catch (e) {
      showError(`Failed to apply watermarks: ${e}`);
    } finally {
      processingWatermarksVar = false;
      setTimeout(() => {
        processingStatus = '';
        imagesToProcess = 0;
      }, 2000);
    }
  }

  function clearWatermarkFolders() {
    showClearConfirm = true;
  }

  async function doClearWatermarks() {
    showClearConfirm = false;

    try {
      const result: any = await invoke('clear_watermark_folders', {
        folderPath: property!.folder_path,
        status: property!.status
      });

      if (result.success) {
        await loadWatermarkImages();
        showSuccess('Watermark folders cleared');
      } else {
        showError(result.error || 'Failed to clear watermark folders');
      }
    } catch (e) {
      showError(`Failed to clear folders: ${e}`);
    }
  }

  async function fillAggeliaTo25() {
    if (!property) return;

    try {
      fillingTo25 = true;
      const result = await DatabaseService.fillAggeliaTo25(property.folder_path, property.status);

      if (result.success) {
        const addedCount = result.data?.added_count || 0;
        if (addedCount === 0) {
          showSuccess('Property already has 25 or more images');
        } else {
          showSuccess(`Added ${addedCount} images to reach 25`);
          await loadWatermarkImages();
        }
      } else {
        showError(result.error || 'Failed to fill to 25 images');
      }
    } catch (e) {
      showError(`Failed to fill images: ${e}`);
    } finally {
      fillingTo25 = false;
    }
  }

  async function openWatermarkedImage(filename: string, fromAggelia: boolean = false) {
    if (!property) return;

    try {
      const result: any = await invoke('open_images_in_folder', {
        folderPath: fromAggelia
          ? `${property!.folder_path}/WATERMARK/AGGELIA`
          : `${property!.folder_path}/WATERMARK`,
        status: property.status,
        selectedImage: filename
      });

      if (!result.success) {
        error = result.error || 'Failed to open image';
      }
    } catch (e) {
      error = `Failed to open image: ${e}`;
    }
  }

  // Reactive values for UI
  let totalWatermarkedImages = $derived(watermarkFilenames.length + watermarkAggeliaFilenames.length);

  async function completeAndNavigate() {
    // Set status to DONE only if property is NEW
    if (property && property.status === 'NEW') {
      try {
        await DatabaseService.updatePropertyStatus(property.id!, 'DONE');
        showSuccess('Property marked as Done');
      } catch (e) {
        showError(`Failed to update status: ${e}`);
      }
    }
    goto('/properties');
  }
</script>

{#if loading}
  <div class="flex h-64 items-center justify-center">
    <div class="text-center">
      <div
        class="border-accent-500 mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-t-transparent"
      ></div>
      <p class="text-foreground-600 font-medium">Loading watermark data...</p>
    </div>
  </div>
{:else if error}
  <div class="p-6">
    <div class="border-background-300 bg-background-100 border p-4">
      <p class="text-foreground-900 font-medium">{error}</p>
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
              d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"
            />
          </svg>
        </div>
        <div>
          <h1 class="text-foreground-900 text-2xl font-bold">Step 4: Add Watermark</h1>
          <p class="text-foreground-600">Apply professional watermarks to your final images</p>
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
            <p class="text-foreground-700 mb-1 text-sm font-medium">Final step workflow:</p>
            <ul class="text-foreground-600 space-y-1 text-sm">
              <li>• Apply watermarks to all processed images</li>
              <li>• Creates publication-ready copies with branding</li>
              <li>• Maintains original aspect ratios and quality</li>
              <li>• Final images ready for web publication</li>
            </ul>
          </div>
        </div>
      </div>
    </div> -->

    <!-- Processing Progress -->
    {#if processingWatermarksVar || processingStatus}
      <div class="bg-background-100 border-background-300 border p-4">
        <div class="flex items-center space-x-3">
          {#if processingWatermarksVar}
            <div
              class="border-accent-500 h-5 w-5 animate-spin rounded-full border-2 border-t-transparent"
            ></div>
          {:else}
            <svg
              class="text-accent-500 h-5 w-5"
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
          {/if}
          <div class="flex-1">
            <p class="text-foreground-900 font-medium">
              {processingStatus || 'Processing watermarks...'}
            </p>
            {#if processingWatermarksVar && imagesToProcess > 0}
              <p class="text-foreground-500 text-sm">
                This may take a moment for {imagesToProcess} images.
              </p>
            {/if}
          </div>
        </div>
      </div>
    {/if}

    <!-- Statistics -->
    <div class="grid grid-cols-1 gap-4 md:grid-cols-3">
      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">Watermarked</p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">{watermarkFilenames.length}</p>
      </div>

      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">
          Advanced + Watermark
        </p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">
          {watermarkAggeliaFilenames.length}
        </p>
      </div>

      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">Total Ready</p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">{totalWatermarkedImages}</p>
      </div>
    </div>

    <!-- Watermark Configuration -->
    <div class="bg-background-50 border-background-200 border p-4">
      <div class="mb-4">
        <h2 class="text-foreground-900 text-sm font-semibold">Watermark Configuration</h2>
        <p class="text-foreground-600 text-xs">Current watermark settings</p>
      </div>

      {#if watermarkConfig?.imagePath}
        <div class="grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-4">
          <div class="bg-background-100 border-background-200 border p-3">
            <p class="text-foreground-500 mb-1 text-xs font-medium tracking-wide uppercase">
              Status
            </p>
            <p class="text-foreground-900 text-sm font-semibold">Configured</p>
          </div>
          <div class="bg-background-100 border-background-200 border p-3">
            <p class="text-foreground-500 mb-1 text-xs font-medium tracking-wide uppercase">
              Opacity
            </p>
            <p class="text-foreground-900 text-sm font-semibold">
              {Math.round(watermarkConfig.opacity * 100)}%
            </p>
          </div>
          <div class="bg-background-100 border-background-200 border p-3">
            <p class="text-foreground-500 mb-1 text-xs font-medium tracking-wide uppercase">
              Size Mode
            </p>
            <p class="text-foreground-900 text-sm font-semibold capitalize">
              {watermarkConfig.sizeMode}
            </p>
          </div>
          <div class="bg-background-100 border-background-200 border p-3">
            <p class="text-foreground-500 mb-1 text-xs font-medium tracking-wide uppercase">
              Position
            </p>
            <p class="text-foreground-900 text-sm font-semibold capitalize">
              {watermarkConfig.positionAnchor.replace('-', ' ')}
            </p>
          </div>
        </div>
      {:else}
        <div class="bg-background-100 border-background-200 border p-4">
          <h3 class="text-foreground-900 mb-2 text-sm font-semibold">Watermark Not Configured</h3>
          <p class="text-foreground-600 mb-3 text-sm">
            Please configure your watermark image and opacity in settings before applying
            watermarks.
          </p>
          <a
            href="/settings"
            class="bg-accent-500 hover:bg-accent-600 inline-flex items-center space-x-2 px-4 py-2 text-sm font-medium text-white transition-colors"
          >
            <span>Go to Settings</span>
          </a>
        </div>
      {/if}
    </div>

    <!-- Actions -->
    <div class="bg-background-50 border-background-200 border p-4">
      <div class="mb-3">
        <h2 class="text-foreground-900 text-sm font-semibold">Watermark Actions</h2>
        <p class="text-foreground-600 text-xs">
          Apply or manage watermarks on your processed images
        </p>
      </div>

      <div class="flex flex-col space-y-3 sm:flex-row sm:space-y-0 sm:space-x-3">
        <button
          onclick={applyWatermarksToAllImages}
          disabled={processingWatermarksVar || !watermarkConfig?.imagePath}
          class="bg-accent-500 hover:bg-accent-600 flex items-center justify-center space-x-2 px-4 py-2 text-sm font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
        >
          {#if processingWatermarksVar}
            <div class="h-4 w-4 animate-spin border-2 border-white border-t-transparent"></div>
            <span>Processing...</span>
          {:else}
            <span>Apply Watermarks</span>
          {/if}
        </button>

        {#if totalWatermarkedImages > 0}
          <button
            onclick={clearWatermarkFolders}
            disabled={processingWatermarksVar}
            class="bg-background-200 text-foreground-700 hover:bg-background-300 border-background-300 flex items-center justify-center space-x-2 border px-4 py-2 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
              />
            </svg>
            <span>Clear All</span>
          </button>
        {/if}

        <button
          onclick={fillAggeliaTo25}
          disabled={processingWatermarksVar || fillingTo25}
          class="bg-background-200 text-foreground-700 hover:bg-background-300 border-background-300 flex items-center justify-center space-x-2 border px-4 py-2 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
        >
          {#if fillingTo25}
            <div
              class="border-foreground-500 h-4 w-4 animate-spin border-2 border-t-transparent"
            ></div>
            <span>Filling...</span>
          {:else}
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
              />
            </svg>
            <span>Fill to 25</span>
          {/if}
        </button>
      </div>
    </div>

    <!-- WATERMARK Images -->
    <section class="bg-background-50 border-background-200 border">
      <div class="p-4">
        <div class="mb-3">
          <h2 class="text-foreground-900 text-sm font-semibold">
            WATERMARK Folder ({watermarkFilenames.length})
          </h2>
          <p class="text-foreground-600 text-xs">
            Publication-ready images with watermarks applied
          </p>
        </div>

        {#if watermarkFilenames.length === 0}
          <div class="py-8 text-center">
            <p class="text-foreground-500 mb-3 text-sm">No watermarked images yet</p>
            <p class="text-foreground-600 text-xs">
              Click "Apply Watermarks" above to create watermarked copies.
            </p>
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {#each watermarkFilenames as filename}
              <button
                class="border-background-200 group relative aspect-square w-full overflow-hidden border transition-opacity hover:opacity-75"
                onclick={() => openWatermarkedImage(filename, false)}
                disabled={processingWatermarksVar}
              >
                <LazyImage
                  folderPath={property.folder_path}
                  status={property.status}
                  subfolder="WATERMARK"
                  {filename}
                  alt={filename}
                  class="h-full w-full"
                />

                <!-- Filename on hover -->
                <div
                  class="bg-foreground-900/75 absolute inset-x-0 bottom-0 p-2 opacity-0 transition-opacity group-hover:opacity-100"
                >
                  <p class="truncate text-xs text-white" title={filename}>
                    {filename}
                  </p>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    <!-- WATERMARK/AGGELIA Images -->
    <section class="bg-background-50 border-background-200 border">
      <div class="p-4">
        <div class="mb-3">
          <h2 class="text-foreground-900 text-sm font-semibold">
            WATERMARK/AGGELIA Folder ({watermarkAggeliaFilenames.length})
          </h2>
          <p class="text-foreground-600 text-xs">Advanced edited images with watermarks</p>
        </div>

        {#if watermarkAggeliaFilenames.length === 0}
          <div class="py-8 text-center">
            <p class="text-foreground-500 mb-3 text-sm">No advanced watermarked images yet</p>
            <p class="text-foreground-600 text-xs">
              Advanced edited images with watermarks will appear here.
            </p>
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {#each watermarkAggeliaFilenames as filename}
              <button
                class="border-background-200 group relative aspect-square w-full overflow-hidden border transition-opacity hover:opacity-75"
                onclick={() => openWatermarkedImage(filename, true)}
                disabled={processingWatermarksVar}
              >
                <LazyImage
                  folderPath={property.folder_path}
                  status={property.status}
                  subfolder="WATERMARK/AGGELIA"
                  {filename}
                  alt={filename}
                  class="h-full w-full"
                />

                <!-- Filename on hover -->
                <div
                  class="bg-foreground-900/75 absolute inset-x-0 bottom-0 p-2 opacity-0 transition-opacity group-hover:opacity-100"
                >
                  <p class="truncate text-xs text-white" title={filename}>
                    {filename}
                  </p>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    <!-- Completion -->
    <div class="bg-background-50 border-background-200 border p-6 text-center">
      <h3 class="text-foreground-900 mb-2 text-lg font-semibold">Workflow Complete</h3>
      <p class="text-foreground-600 mx-auto mb-4 max-w-2xl text-sm">
        Your images have been processed through all workflow steps. The WATERMARK folders contain
        your final, publication-ready images.
      </p>

      <div
        class="flex flex-col items-center justify-center space-y-3 sm:flex-row sm:space-y-0 sm:space-x-3"
      >
        <button
          onclick={completeAndNavigate}
          class="bg-accent-500 hover:bg-accent-600 inline-flex items-center space-x-2 px-4 py-2 text-sm font-medium text-white transition-colors"
        >
          <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M5 13l4 4L19 7"
            />
          </svg>
          <span>Complete</span>
        </button>

        <a
          href="/properties"
          class="bg-background-200 text-foreground-700 hover:bg-background-300 inline-flex items-center space-x-2 px-4 py-2 text-sm font-medium transition-colors"
        >
          <span>Dashboard</span>
        </a>
      </div>
    </div>
  </div>

  <!-- Apply Watermarks Confirmation Dialog -->
  {#if showWatermarkConfirm}
    <ConfirmDialog
      title="Apply Watermarks"
      message="This will copy images from INTERNET folders to WATERMARK folders and apply watermark to them. This may take a while. Continue?"
      confirmText="Apply"
      onConfirm={doApplyWatermarks}
      onCancel={() => (showWatermarkConfirm = false)}
    />
  {/if}

  <!-- Clear Watermarks Confirmation Dialog -->
  {#if showClearConfirm}
    <ConfirmDialog
      title="Clear Watermark Folders"
      message="Are you sure you want to clear all watermarked images? This action cannot be undone."
      confirmText="Clear"
      destructive={true}
      onConfirm={doClearWatermarks}
      onCancel={() => (showClearConfirm = false)}
    />
  {/if}
{/if}
