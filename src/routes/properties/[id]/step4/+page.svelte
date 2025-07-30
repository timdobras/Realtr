<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';

  interface ImageItem {
    filename: string;
    dataUrl: string;
    loading: boolean;
  }

  let propertyId: number | null = null;
  let property: Property | null = null;
  let watermarkImages: ImageItem[] = [];
  let watermarkAggeliaImages: ImageItem[] = [];
  let error = '';
  let loading = true;
  let processingWatermarksVar = false;
  let watermarkConfig: { imagePath?: string; opacity: number } | null = null;
  let processingProgress = 0;
  let processingStatus = '';

  // Get the id from the URL params
  $: propertyId = Number($page.params.id);

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
      const config = await invoke('load_config');
      if (config) {
        watermarkConfig = {
          imagePath: config.watermark_image_path,
          opacity: config.watermark_opacity || 0.15
        };
      }
    } catch (e) {
      console.error('Failed to load watermark config:', e);
    }
  }

  async function loadWatermarkImages() {
    if (!property) return;

    try {
      // Load WATERMARK images
      const watermarkList = await invoke('list_watermark_images', {
        folderPath: property.folder_path
      });

      if (Array.isArray(watermarkList)) {
        const sortedWatermarkList = sortImagesByNumericFilename(watermarkList);
        watermarkImages = sortedWatermarkList.map((filename) => ({
          filename,
          dataUrl: '',
          loading: true
        }));

        // Load thumbnails
        for (let i = 0; i < watermarkImages.length; i++) {
          const image = watermarkImages[i];
          try {
            const base64Data = await invoke('get_watermark_image_as_base64', {
              folderPath: property.folder_path,
              filename: image.filename,
              fromAggelia: false
            });

            const ext = image.filename.split('.').pop()?.toLowerCase() || '';
            const mimeType = getMimeType(ext);

            watermarkImages[i] = {
              ...image,
              dataUrl: `data:${mimeType};base64,${base64Data}`,
              loading: false
            };
          } catch (e) {
            watermarkImages[i] = { ...image, dataUrl: '', loading: false };
          }
        }
      }

      // Load WATERMARK/AGGELIA images
      const aggeliaList = await invoke('list_watermark_aggelia_images', {
        folderPath: property.folder_path
      });

      if (Array.isArray(aggeliaList)) {
        const sortedAggeliaList = sortImagesByNumericFilename(aggeliaList);
        watermarkAggeliaImages = sortedAggeliaList.map((filename) => ({
          filename,
          dataUrl: '',
          loading: true
        }));

        // Load thumbnails
        for (let i = 0; i < watermarkAggeliaImages.length; i++) {
          const image = watermarkAggeliaImages[i];
          try {
            const base64Data = await invoke('get_watermark_image_as_base64', {
              folderPath: property.folder_path,
              filename: image.filename,
              fromAggelia: true
            });

            const ext = image.filename.split('.').pop()?.toLowerCase() || '';
            const mimeType = getMimeType(ext);

            watermarkAggeliaImages[i] = {
              ...image,
              dataUrl: `data:${mimeType};base64,${base64Data}`,
              loading: false
            };
          } catch (e) {
            watermarkAggeliaImages[i] = { ...image, dataUrl: '', loading: false };
          }
        }
      }
    } catch (e) {
      watermarkImages = [];
      watermarkAggeliaImages = [];
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

  async function applyWatermarksToAllImages() {
    if (!property || !watermarkConfig?.imagePath) {
      error = 'Watermark not configured. Please set up watermark in settings first.';
      return;
    }

    const confirmMessage =
      'This will copy images from INTERNET folders to WATERMARK folders and apply watermark to them. This may take a while. Continue?';

    if (!confirm(confirmMessage)) {
      return;
    }

    try {
      processingWatermarksVar = true;
      processingProgress = 0;
      processingStatus = 'Initializing watermark process...';
      error = '';

      // Simulate progress updates (in real implementation, this would come from backend)
      const progressInterval = setInterval(() => {
        if (processingProgress < 90) {
          processingProgress += Math.random() * 10;
          if (processingProgress < 30) {
            processingStatus = 'Copying images to WATERMARK folders...';
          } else if (processingProgress < 60) {
            processingStatus = 'Applying watermarks to images...';
          } else if (processingProgress < 90) {
            processingStatus = 'Finalizing watermarked images...';
          }
        }
      }, 500);

      const result = await invoke('copy_and_watermark_images', {
        folderPath: property.folder_path
      });

      clearInterval(progressInterval);
      processingProgress = 100;
      processingStatus = 'Watermark application completed!';

      if (result.success) {
        await loadWatermarkImages();
        setTimeout(() => {
          processingStatus = '';
        }, 2000);
      } else {
        error = result.error || 'Failed to apply watermarks';
        processingStatus = 'Error occurred during processing';
      }
    } catch (e) {
      error = `Failed to apply watermarks: ${e}`;
      processingStatus = 'Error occurred during processing';
    } finally {
      processingWatermarksVar = false;
      if (!error) {
        setTimeout(() => {
          processingProgress = 0;
          processingStatus = '';
        }, 1000);
      }
    }
  }

  async function clearWatermarkFolders() {
    if (
      !confirm(
        'Are you sure you want to clear all watermarked images? This action cannot be undone.'
      )
    ) {
      return;
    }

    try {
      const result = await invoke('clear_watermark_folders', {
        folderPath: property.folder_path
      });

      if (result.success) {
        await loadWatermarkImages();
      } else {
        error = result.error || 'Failed to clear watermark folders';
      }
    } catch (e) {
      error = `Failed to clear folders: ${e}`;
    }
  }

  async function openWatermarkedImage(filename: string, fromAggelia: boolean = false) {
    if (!property) return;

    try {
      const result = await invoke('open_images_in_folder', {
        folderPath: fromAggelia
          ? `${property.folder_path}/WATERMARK/AGGELIA`
          : `${property.folder_path}/WATERMARK`,
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
  $: totalWatermarkedImages = watermarkImages.length + watermarkAggeliaImages.length;
  $: workflowProgress = 100; // Step 4 is 100% of workflow
</script>

{#if loading}
  <div class="flex h-64 items-center justify-center">
    <div class="text-center">
      <div
        class="mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-blue-500 border-t-transparent"
      ></div>
      <p class="text-foreground-600">Loading watermark data...</p>
    </div>
  </div>
{:else if error}
  <div class="p-6">
    <div class="rounded-lg border border-red-200 bg-red-50 p-4">
      <div class="flex items-center space-x-2">
        <span class="text-red-600">❌</span>
        <p class="font-medium text-red-800">{error}</p>
      </div>
    </div>
  </div>
{:else if property}
  <div class="space-y-6 p-6">
    <!-- Processing Progress -->
    {#if processingWatermarksVar || processingProgress > 0}
      <div class="rounded-xl border border-blue-200 bg-blue-50 p-6">
        <div class="mb-4 flex items-center space-x-4">
          <div
            class="h-6 w-6 animate-spin rounded-full border-3 border-blue-500 border-t-transparent"
          ></div>
          <div class="flex-1">
            <p class="font-medium text-blue-900">
              {processingStatus || 'Processing watermarks...'}
            </p>
            <p class="text-sm text-blue-700">
              Please wait while we apply watermarks to your images.
            </p>
          </div>
        </div>
        <div class="h-4 w-full rounded-full bg-blue-200">
          <div
            class="h-4 rounded-full bg-blue-600 transition-all duration-300 ease-out"
            style="width: {processingProgress}%"
          ></div>
        </div>
        <div class="mt-2 flex justify-between text-xs text-blue-600">
          <span>Processing...</span>
          <span>{Math.round(processingProgress)}%</span>
        </div>
      </div>
    {/if}

    <!-- Statistics Dashboard -->
    <div class="grid grid-cols-1 gap-4 md:grid-cols-3">
      <div class="bg-background-100 border-background-200 rounded-xl border p-6 shadow-sm">
        <div class="flex items-center space-x-3">
          <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-amber-100">
            <span class="text-2xl">🏷️</span>
          </div>
          <div>
            <p class="text-foreground-900 text-2xl font-bold">{watermarkImages.length}</p>
            <p class="text-foreground-600 text-sm">Watermarked Images</p>
          </div>
        </div>
      </div>

      <div class="bg-background-100 border-background-200 rounded-xl border p-6 shadow-sm">
        <div class="flex items-center space-x-3">
          <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-purple-100">
            <span class="text-2xl">🎨</span>
          </div>
          <div>
            <p class="text-foreground-900 text-2xl font-bold">{watermarkAggeliaImages.length}</p>
            <p class="text-foreground-600 text-sm">Advanced + Watermark</p>
          </div>
        </div>
      </div>

      <div class="bg-background-100 border-background-200 rounded-xl border p-6 shadow-sm">
        <div class="flex items-center space-x-3">
          <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-green-100">
            <span class="text-2xl">📊</span>
          </div>
          <div>
            <p class="text-foreground-900 text-2xl font-bold">{totalWatermarkedImages}</p>
            <p class="text-foreground-600 text-sm">Total Ready</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Watermark Configuration Display -->
    <div class="bg-background-100 border-background-200 rounded-xl border p-6 shadow-sm">
      <h2 class="text-foreground-900 mb-6 text-lg font-semibold">Watermark Configuration</h2>
      {#if watermarkConfig?.imagePath}
        <div class="flex items-center space-x-6">
          <div
            class="bg-background-100 border-background-300 flex h-20 w-20 items-center justify-center overflow-hidden rounded-xl border-2 shadow-inner"
          >
            <img
              src={`file://${watermarkConfig.imagePath}`}
              alt="Watermark preview"
              class="max-h-full max-w-full object-contain"
              style="opacity: {watermarkConfig.opacity}"
              onerror={() => {}}
            />
          </div>
          <div class="flex-1">
            <div class="grid grid-cols-1 gap-4 md:grid-cols-3">
              <div class="bg-background-50 rounded-lg p-4">
                <p class="text-foreground-500 text-xs font-medium tracking-wide uppercase">
                  Status
                </p>
                <p class="text-sm font-semibold text-green-600">✅ Configured</p>
              </div>
              <div class="bg-background-50 rounded-lg p-4">
                <p class="text-foreground-500 text-xs font-medium tracking-wide uppercase">
                  Opacity
                </p>
                <p class="text-foreground-900 text-sm font-semibold">
                  {Math.round(watermarkConfig.opacity * 100)}%
                </p>
              </div>
              <div class="bg-background-50 rounded-lg p-4">
                <p class="text-foreground-500 text-xs font-medium tracking-wide uppercase">
                  Position
                </p>
                <p class="text-foreground-900 text-sm font-semibold">Center</p>
              </div>
            </div>
          </div>
        </div>
      {:else}
        <div class="rounded-xl border border-yellow-200 bg-yellow-50 p-6">
          <div class="flex items-center space-x-3">
            <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-yellow-100">
              <span class="text-2xl">⚠️</span>
            </div>
            <div class="flex-1">
              <h3 class="mb-1 font-medium text-yellow-900">Watermark Not Configured</h3>
              <p class="mb-3 text-sm text-yellow-800">
                Please configure your watermark image and opacity before applying watermarks.
              </p>
              <a
                href="/settings"
                class="inline-flex items-center space-x-2 rounded-lg bg-yellow-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-yellow-700"
              >
                <span>⚙️</span>
                <span>Go to Settings</span>
              </a>
            </div>
          </div>
        </div>
      {/if}
    </div>

    <!-- Action Controls -->
    <div class="bg-background-100 border-background-200 rounded-xl border p-6 shadow-sm">
      <h2 class="text-foreground-900 mb-6 text-lg font-semibold">Watermark Actions</h2>

      <div
        class="flex flex-col space-y-4 lg:flex-row lg:items-center lg:justify-between lg:space-y-0 lg:space-x-6"
      >
        <div class="flex-1">
          <p class="text-foreground-700 mb-2">
            Apply watermarks to all images from INTERNET and INTERNET/AGGELIA folders.
          </p>
          <p class="text-foreground-500 text-sm">
            This will create watermarked copies in WATERMARK and WATERMARK/AGGELIA folders.
          </p>
        </div>

        <div class="flex flex-col space-y-3 sm:flex-row sm:space-y-0 sm:space-x-4">
          <button
            onclick={applyWatermarksToAllImages}
            disabled={processingWatermarksVar || !watermarkConfig?.imagePath}
            class="btn-primary flex cursor-pointer items-center justify-center space-x-2 rounded-lg bg-blue-700 px-6 py-3 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <span class="text-lg">🏷️</span>
            <span>Apply Watermarks</span>
            {#if processingWatermarksVar}
              <div
                class="ml-2 h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
              ></div>
            {/if}
          </button>

          <button
            onclick={clearWatermarkFolders}
            disabled={processingWatermarksVar || totalWatermarkedImages === 0}
            class="flex items-center justify-center space-x-2 rounded-lg bg-red-600 px-6 py-3 font-medium text-white transition-colors hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <span>🗑️</span>
            <span>Clear All</span>
          </button>
        </div>
      </div>
    </div>

    <!-- WATERMARK Images Section -->
    <section class="bg-background-100 border-background-200 rounded-xl border shadow-sm">
      <div class="p-6">
        <div class="mb-6 flex items-center justify-between">
          <h2 class="text-foreground-900 text-xl font-semibold">
            WATERMARK Folder ({watermarkImages.length})
          </h2>
          {#if watermarkImages.length > 0}
            <div class="text-foreground-500 text-sm">Click images to view • Publication ready</div>
          {/if}
        </div>

        {#if watermarkImages.length === 0}
          <div class="py-16 text-center">
            <div
              class="mx-auto mb-4 flex h-24 w-24 items-center justify-center rounded-full bg-amber-100"
            >
              <span class="text-3xl">🏷️</span>
            </div>
            <h3 class="text-foreground-700 mb-2 text-lg font-medium">No watermarked images yet</h3>
            <p class="text-foreground-500 mb-6">
              Click "Apply Watermarks" to create watermarked copies of your images.
            </p>
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
            {#each watermarkImages as image}
              <div class="group relative">
                <button
                  class="bg-background-100 aspect-square w-full overflow-hidden rounded-xl border-2 border-amber-300 shadow-lg shadow-amber-100 transition-all duration-200 hover:scale-105 hover:shadow-xl"
                  onclick={() => openWatermarkedImage(image.filename, false)}
                  disabled={processingWatermarksVar}
                >
                  {#if image.loading}
                    <div class="bg-background-100 flex h-full w-full items-center justify-center">
                      <div class="text-center">
                        <div
                          class="mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-3 border-amber-500 border-t-transparent"
                        ></div>
                        <p class="text-foreground-500 text-xs">Loading...</p>
                      </div>
                    </div>
                  {:else if image.dataUrl}
                    <img
                      src={image.dataUrl}
                      alt={image.filename}
                      loading="lazy"
                      class="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
                    />
                  {:else}
                    <div class="flex h-full w-full items-center justify-center bg-red-100">
                      <div class="text-center text-red-500">
                        <span class="mb-2 block text-2xl">❌</span>
                        <p class="text-xs">Failed to load</p>
                      </div>
                    </div>
                  {/if}

                  <!-- Watermark indicator -->
                  <div
                    class="absolute top-2 right-2 flex items-center space-x-1 rounded-full bg-amber-500 px-2 py-1 text-xs text-white shadow-lg"
                  >
                    <span>🏷️</span>
                    <span class="hidden sm:inline">Watermarked</span>
                  </div>

                  <!-- Filename -->
                  <div
                    class="absolute right-0 bottom-0 left-0 bg-gradient-to-t from-amber-900/80 via-amber-800/60 to-transparent p-3 pt-6"
                  >
                    <p class="truncate text-xs font-medium text-white" title={image.filename}>
                      {image.filename}
                    </p>
                  </div>
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    <!-- WATERMARK/AGGELIA Images Section -->
    <section class="bg-background-100 border-background-200 rounded-xl border shadow-sm">
      <div class="p-6">
        <div class="mb-6 flex items-center justify-between">
          <h2 class="text-foreground-900 text-xl font-semibold">
            WATERMARK/AGGELIA Folder ({watermarkAggeliaImages.length})
          </h2>
          {#if watermarkAggeliaImages.length > 0}
            <div class="text-foreground-500 text-sm">Advanced edited + watermarked images</div>
          {/if}
        </div>

        {#if watermarkAggeliaImages.length === 0}
          <div class="py-16 text-center">
            <div
              class="mx-auto mb-4 flex h-24 w-24 items-center justify-center rounded-full bg-purple-100"
            >
              <span class="text-3xl">🎨</span>
            </div>
            <h3 class="text-foreground-700 mb-2 text-lg font-medium">
              No advanced watermarked images yet
            </h3>
            <p class="text-foreground-500 mb-6">
              Advanced edited images with watermarks will appear here.
            </p>
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
            {#each watermarkAggeliaImages as image}
              <div class="group relative">
                <button
                  class="bg-background-100 aspect-square w-full overflow-hidden rounded-xl border-2 border-indigo-400 shadow-lg shadow-indigo-100 transition-all duration-200 hover:scale-105 hover:shadow-xl"
                  onclick={() => openWatermarkedImage(image.filename, true)}
                  disabled={processingWatermarksVar}
                >
                  {#if image.loading}
                    <div class="bg-background-100 flex h-full w-full items-center justify-center">
                      <div class="text-center">
                        <div
                          class="mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-3 border-indigo-500 border-t-transparent"
                        ></div>
                        <p class="text-foreground-500 text-xs">Loading...</p>
                      </div>
                    </div>
                  {:else if image.dataUrl}
                    <img
                      src={image.dataUrl}
                      alt={image.filename}
                      loading="lazy"
                      class="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
                    />
                  {:else}
                    <div class="flex h-full w-full items-center justify-center bg-red-100">
                      <div class="text-center text-red-500">
                        <span class="mb-2 block text-2xl">❌</span>
                        <p class="text-xs">Failed to load</p>
                      </div>
                    </div>
                  {/if}

                  <!-- Advanced + Watermark indicator -->
                  <div
                    class="absolute top-2 right-2 flex items-center space-x-1 rounded-full bg-indigo-500 px-2 py-1 text-xs text-white shadow-lg"
                  >
                    <span>🎨🏷️</span>
                    <span class="hidden sm:inline">Pro</span>
                  </div>

                  <!-- Filename -->
                  <div
                    class="absolute right-0 bottom-0 left-0 bg-gradient-to-t from-indigo-900/80 via-indigo-800/60 to-transparent p-3 pt-6"
                  >
                    <p class="truncate text-xs font-medium text-white" title={image.filename}>
                      {image.filename}
                    </p>
                  </div>
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    <!-- Completion Section -->
    <div
      class="rounded-xl border border-green-200 bg-gradient-to-r from-green-50 to-emerald-50 p-8"
    >
      <div class="text-center">
        <div
          class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-green-500"
        >
          <span class="text-2xl">🎉</span>
        </div>
        <h3 class="mb-3 text-2xl font-bold text-green-900">Workflow Complete!</h3>
        <p class="mx-auto mb-6 max-w-2xl text-green-700">
          Congratulations! Your images have been processed through all workflow steps. The WATERMARK
          folders contain your final, publication-ready images with professional watermarks applied.
        </p>
        <div
          class="flex flex-col items-center justify-center space-y-3 text-green-800 sm:flex-row sm:space-y-0 sm:space-x-4"
        >
          <a href="/properties/{property.id}" class="btn-primary flex items-center space-x-2">
            <span>👁️</span>
            <span>View Property Overview</span>
          </a>
          <a href="/" class="btn-secondary flex items-center space-x-2">
            <span>🏠</span>
            <span>Back to Dashboard</span>
          </a>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
</style>
