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
  let watermarkConfig: {
    imagePath?: string;
    opacity: number;
    sizeMode: string;
    positionAnchor: string;
  } | null = null;
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

      const result: any = await invoke('copy_and_watermark_images', {
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
      const result: any = await invoke('clear_watermark_folders', {
        folderPath: property!.folder_path
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
      const result: any = await invoke('open_images_in_folder', {
        folderPath: fromAggelia
          ? `${property!.folder_path}/WATERMARK/AGGELIA`
          : `${property!.folder_path}/WATERMARK`,
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
        class="border-accent-500 mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-t-transparent"
      ></div>
      <p class="text-foreground-600 font-medium">Loading watermark data...</p>
    </div>
  </div>
{:else if error}
  <div class="p-6">
    <div class="rounded-lg border border-red-200 bg-red-50 p-4">
      <div class="flex items-center space-x-3">
        <svg class="h-5 w-5 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
        <p class="font-medium text-red-800">{error}</p>
      </div>
    </div>
  </div>
{:else if property}
  <div class="min-h-full space-y-8 p-6">
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
    {#if processingWatermarksVar || processingProgress > 0}
      <div class="bg-accent-50 border-accent-200 rounded-xl border p-6">
        <div class="mb-4 flex items-center space-x-4">
          <div
            class="border-accent-500 h-6 w-6 animate-spin rounded-full border-2 border-t-transparent"
          ></div>
          <div class="flex-1">
            <p class="text-accent-900 font-semibold">
              {processingStatus || 'Processing watermarks...'}
            </p>
            <p class="text-accent-700 text-sm">
              Please wait while we apply watermarks to your images.
            </p>
          </div>
        </div>
        <div class="bg-accent-200 h-3 w-full overflow-hidden rounded-full">
          <div
            class="bg-accent-500 h-full rounded-full transition-all duration-300 ease-out"
            style="width: {processingProgress}%"
          ></div>
        </div>
        <div class="text-accent-700 mt-3 flex justify-between text-sm">
          <span>Processing watermarks...</span>
          <span class="font-medium">{Math.round(processingProgress)}%</span>
        </div>
      </div>
    {/if}

    <!-- Statistics Dashboard -->
    <div class="grid grid-cols-1 gap-6 md:grid-cols-3">
      <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
        <div class="flex items-center space-x-3">
          <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-orange-100">
            <svg
              class="h-6 w-6 text-orange-600"
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
            <p class="text-foreground-900 text-2xl font-bold">{watermarkImages.length}</p>
            <p class="text-foreground-600 text-sm font-medium">Watermarked Images</p>
          </div>
        </div>
      </div>

      <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
        <div class="flex items-center space-x-3">
          <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-purple-100">
            <svg
              class="h-6 w-6 text-purple-600"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v1a2 2 0 002 2h6a2 2 0 012 2v8a4 4 0 01-4 4H7z"
              />
            </svg>
          </div>
          <div>
            <p class="text-foreground-900 text-2xl font-bold">{watermarkAggeliaImages.length}</p>
            <p class="text-foreground-600 text-sm font-medium">Advanced + Watermark</p>
          </div>
        </div>
      </div>

      <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
        <div class="flex items-center space-x-3">
          <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-green-100">
            <svg
              class="h-6 w-6 text-green-600"
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
          <div>
            <p class="text-foreground-900 text-2xl font-bold">{totalWatermarkedImages}</p>
            <p class="text-foreground-600 text-sm font-medium">Total Ready</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Watermark Configuration Display -->
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
              d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
            />
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
            />
          </svg>
        </div>
        <div>
          <h2 class="text-foreground-900 text-lg font-semibold">Watermark Configuration</h2>
          <p class="text-foreground-600 text-sm">Current watermark settings for your images</p>
        </div>
      </div>

      {#if watermarkConfig?.imagePath}
        <div class="flex items-center space-x-6">
          <div
            class="border-background-300 bg-background-100 flex h-20 w-20 items-center justify-center overflow-hidden rounded-xl border-2 shadow-inner"
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
            <div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
              <div class="bg-background-100 border-background-200 rounded-lg border p-4">
                <div class="mb-2 flex items-center space-x-2">
                  <svg
                    class="h-4 w-4 text-green-600"
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
                  <p class="text-foreground-500 text-xs font-medium tracking-wide uppercase">
                    Status
                  </p>
                </div>
                <p class="text-sm font-semibold text-green-600">Configured</p>
              </div>
              <div class="bg-background-100 border-background-200 rounded-lg border p-4">
                <div class="mb-2 flex items-center space-x-2">
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
                      d="M13 10V3L4 14h7v7l9-11h-7z"
                    />
                  </svg>
                  <p class="text-foreground-500 text-xs font-medium tracking-wide uppercase">
                    Opacity
                  </p>
                </div>
                <p class="text-foreground-900 text-sm font-semibold">
                  {Math.round(watermarkConfig.opacity * 100)}%
                </p>
              </div>
              <div class="bg-background-100 border-background-200 rounded-lg border p-4">
                <div class="mb-2 flex items-center space-x-2">
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
                      d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4"
                    />
                  </svg>
                  <p class="text-foreground-500 text-xs font-medium tracking-wide uppercase">
                    Size Mode
                  </p>
                </div>
                <p class="text-foreground-900 text-sm font-semibold capitalize">
                  {watermarkConfig.sizeMode}
                </p>
              </div>
              <div class="bg-background-100 border-background-200 rounded-lg border p-4">
                <div class="mb-2 flex items-center space-x-2">
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
                      d="M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z"
                    />
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M15 11a3 3 0 11-6 0 3 3 0 016 0z"
                    />
                  </svg>
                  <p class="text-foreground-500 text-xs font-medium tracking-wide uppercase">
                    Position
                  </p>
                </div>
                <p class="text-foreground-900 text-sm font-semibold capitalize">
                  {watermarkConfig.positionAnchor.replace('-', ' ')}
                </p>
              </div>
            </div>
          </div>
        </div>
      {:else}
        <div class="rounded-xl border border-yellow-200 bg-yellow-50 p-6">
          <div class="flex items-center space-x-4">
            <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-yellow-100">
              <svg
                class="h-6 w-6 text-yellow-600"
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
            <div class="flex-1">
              <h3 class="mb-1 font-semibold text-yellow-900">Watermark Not Configured</h3>
              <p class="mb-4 text-sm text-yellow-800">
                Please configure your watermark image and opacity in settings before applying
                watermarks to your images.
              </p>
              <a
                href="/settings"
                class="inline-flex items-center space-x-2 rounded-lg bg-yellow-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-yellow-700"
              >
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
                  />
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                  />
                </svg>
                <span>Go to Settings</span>
              </a>
            </div>
          </div>
        </div>
      {/if}
    </div>

    <!-- Action Controls -->
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
              d="M13 10V3L4 14h7v7l9-11h-7z"
            />
          </svg>
        </div>
        <div>
          <h2 class="text-foreground-900 text-lg font-semibold">Watermark Actions</h2>
          <p class="text-foreground-600 text-sm">
            Apply or manage watermarks on your processed images
          </p>
        </div>
      </div>

      <div
        class="flex flex-col space-y-6 lg:flex-row lg:items-center lg:justify-between lg:space-y-0 lg:space-x-8"
      >
        <div class="flex-1">
          <p class="text-foreground-700 mb-2 font-medium">
            Apply watermarks to all processed images
          </p>
          <p class="text-foreground-500 text-sm">
            This will create watermarked copies in WATERMARK and WATERMARK/AGGELIA folders, ready
            for publication.
          </p>
        </div>

        <div class="flex flex-col space-y-3 sm:flex-row sm:space-y-0 sm:space-x-4">
          <button
            onclick={applyWatermarksToAllImages}
            disabled={processingWatermarksVar || !watermarkConfig?.imagePath}
            class="bg-accent-500 hover:bg-accent-600 flex items-center justify-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            {#if processingWatermarksVar}
              <div
                class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
              ></div>
              <span>Processing...</span>
            {:else}
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"
                />
              </svg>
              <span>Apply Watermarks</span>
            {/if}
          </button>

          {#if totalWatermarkedImages > 0}
            <button
              onclick={clearWatermarkFolders}
              disabled={processingWatermarksVar}
              class="flex items-center justify-center space-x-2 rounded-lg border border-red-200 bg-red-50 px-4 py-2 text-sm font-medium text-red-700 transition-colors hover:bg-red-100 disabled:cursor-not-allowed disabled:opacity-50"
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
        </div>
      </div>
    </div>

    <!-- WATERMARK Images Section -->
    <section class="bg-background-50 border-background-200 rounded-xl border shadow-sm">
      <div class="p-6">
        <div class="mb-6 flex items-center justify-between">
          <div class="flex items-center space-x-3">
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
                  d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"
                />
              </svg>
            </div>
            <div>
              <h2 class="text-foreground-900 text-xl font-semibold">
                WATERMARK Folder ({watermarkImages.length})
              </h2>
              <p class="text-foreground-600 text-sm">
                Publication-ready images with watermarks applied
              </p>
            </div>
          </div>

          {#if watermarkImages.length > 0}
            <div class="flex items-center space-x-2 text-sm text-orange-600">
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
              <span>Click to view</span>
            </div>
          {/if}
        </div>

        {#if watermarkImages.length === 0}
          <div class="py-16 text-center">
            <div
              class="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-orange-100"
            >
              <svg
                class="h-10 w-10 text-orange-600"
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
            <h3 class="text-foreground-900 mb-2 text-lg font-semibold">
              No watermarked images yet
            </h3>
            <p class="text-foreground-500 mx-auto mb-6 max-w-md">
              Click "Apply Watermarks" above to create watermarked copies of your processed images.
            </p>
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {#each watermarkImages as image}
              <div class="group relative">
                <button
                  class="bg-background-100 aspect-square w-full overflow-hidden rounded-xl border-2 border-orange-300 shadow-lg shadow-orange-100 transition-all duration-200 hover:scale-105 hover:shadow-xl"
                  onclick={() => openWatermarkedImage(image.filename, false)}
                  disabled={processingWatermarksVar}
                >
                  {#if image.loading}
                    <div class="bg-background-100 flex h-full w-full items-center justify-center">
                      <div class="text-center">
                        <div
                          class="mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-2 border-orange-500 border-t-transparent"
                        ></div>
                        <p class="text-foreground-500 text-xs font-medium">Loading...</p>
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
                    <div class="flex h-full w-full items-center justify-center bg-red-50">
                      <div class="text-center text-red-500">
                        <svg
                          class="mx-auto mb-2 h-8 w-8"
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
                        <p class="text-xs font-medium">Failed to load</p>
                      </div>
                    </div>
                  {/if}

                  <!-- Watermark indicator -->
                  <div
                    class="absolute top-2 right-2 flex items-center space-x-1 rounded-lg bg-orange-500 px-2 py-1 text-xs text-white shadow-lg"
                  >
                    <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"
                      />
                    </svg>
                    <span class="hidden sm:inline">Watermarked</span>
                  </div>

                  <!-- Filename overlay -->
                  <div
                    class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-orange-900/80 via-orange-800/60 to-transparent p-3 pt-8 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                  >
                    <p class="truncate text-xs font-medium text-white" title={image.filename}>
                      {image.filename}
                    </p>
                  </div>

                  <!-- View indicator on hover -->
                  <div
                    class="absolute inset-0 flex items-center justify-center bg-black/20 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                  >
                    <div class="rounded-full bg-white/90 p-3">
                      <svg
                        class="text-foreground-900 h-5 w-5"
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
                    </div>
                  </div>
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    <!-- WATERMARK/AGGELIA Images Section -->
    <section class="bg-background-50 border-background-200 rounded-xl border shadow-sm">
      <div class="p-6">
        <div class="mb-6 flex items-center justify-between">
          <div class="flex items-center space-x-3">
            <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-purple-100">
              <svg
                class="h-5 w-5 text-purple-600"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v1a2 2 0 002 2h6a2 2 0 012 2v8a4 4 0 01-4 4H7z"
                />
              </svg>
            </div>
            <div>
              <h2 class="text-foreground-900 text-xl font-semibold">
                WATERMARK/AGGELIA Folder ({watermarkAggeliaImages.length})
              </h2>
              <p class="text-foreground-600 text-sm">
                Advanced edited images with professional watermarks
              </p>
            </div>
          </div>

          {#if watermarkAggeliaImages.length > 0}
            <div class="flex items-center space-x-2 text-sm text-purple-600">
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v1a2 2 0 002 2h6a2 2 0 012 2v8a4 4 0 01-4 4H7z"
                />
              </svg>
              <span>Premium quality</span>
            </div>
          {/if}
        </div>

        {#if watermarkAggeliaImages.length === 0}
          <div class="py-16 text-center">
            <div
              class="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-purple-100"
            >
              <svg
                class="h-10 w-10 text-purple-600"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v1a2 2 0 002 2h6a2 2 0 012 2v8a4 4 0 01-4 4H7z"
                />
              </svg>
            </div>
            <h3 class="text-foreground-900 mb-2 text-lg font-semibold">
              No advanced watermarked images yet
            </h3>
            <p class="text-foreground-500 mx-auto mb-6 max-w-md">
              Advanced edited images with applied watermarks will appear here once processing is
              complete.
            </p>
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {#each watermarkAggeliaImages as image}
              <div class="group relative">
                <button
                  class="bg-background-100 aspect-square w-full overflow-hidden rounded-xl border-2 border-purple-400 shadow-lg shadow-purple-100 transition-all duration-200 hover:scale-105 hover:shadow-xl"
                  onclick={() => openWatermarkedImage(image.filename, true)}
                  disabled={processingWatermarksVar}
                >
                  {#if image.loading}
                    <div class="bg-background-100 flex h-full w-full items-center justify-center">
                      <div class="text-center">
                        <div
                          class="mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-2 border-purple-500 border-t-transparent"
                        ></div>
                        <p class="text-foreground-500 text-xs font-medium">Loading...</p>
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
                    <div class="flex h-full w-full items-center justify-center bg-red-50">
                      <div class="text-center text-red-500">
                        <svg
                          class="mx-auto mb-2 h-8 w-8"
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
                        <p class="text-xs font-medium">Failed to load</p>
                      </div>
                    </div>
                  {/if}

                  <!-- Advanced + Watermark indicator -->
                  <div
                    class="absolute top-2 right-2 flex items-center space-x-1 rounded-lg bg-purple-500 px-2 py-1 text-xs text-white shadow-lg"
                  >
                    <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v1a2 2 0 002 2h6a2 2 0 012 2v8a4 4 0 01-4 4H7z"
                      />
                    </svg>
                    <span class="hidden sm:inline">Pro</span>
                  </div>

                  <!-- Filename overlay -->
                  <div
                    class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-purple-900/80 via-purple-800/60 to-transparent p-3 pt-8 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                  >
                    <p class="truncate text-xs font-medium text-white" title={image.filename}>
                      {image.filename}
                    </p>
                  </div>

                  <!-- View indicator on hover -->
                  <div
                    class="absolute inset-0 flex items-center justify-center bg-black/20 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                  >
                    <div class="rounded-full bg-white/90 p-3">
                      <svg
                        class="text-foreground-900 h-5 w-5"
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
                    </div>
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
      class="rounded-xl border border-green-200 bg-gradient-to-r from-green-50 to-emerald-50 p-8 text-center"
    >
      <div
        class="mx-auto mb-6 flex h-16 w-16 items-center justify-center rounded-full bg-green-500"
      >
        <svg class="h-8 w-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
      </div>

      <h3 class="mb-3 text-2xl font-bold text-green-900">Workflow Complete!</h3>
      <p class="mx-auto mb-8 max-w-2xl text-green-700">
        Congratulations! Your images have been processed through all workflow steps. The WATERMARK
        folders contain your final, publication-ready images with professional watermarks applied.
      </p>

      <div
        class="flex flex-col items-center justify-center space-y-4 sm:flex-row sm:space-y-0 sm:space-x-6"
      >
        <a
          href="/properties/{property.id}"
          class="inline-flex items-center space-x-2 rounded-lg bg-green-600 px-6 py-3 font-medium text-white transition-colors hover:bg-green-700"
        >
          <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
          <span>View Property Overview</span>
        </a>

        <a
          href="/"
          class="bg-background-200 text-foreground-700 hover:bg-background-300 inline-flex items-center space-x-2 rounded-lg px-6 py-3 font-medium transition-colors"
        >
          <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"
            />
          </svg>
          <span>Back to Dashboard</span>
        </a>
      </div>
    </div>
  </div>
{/if}
