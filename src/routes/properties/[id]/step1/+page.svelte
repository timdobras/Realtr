<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
  import { formatDate } from '$lib/utils/dateUtils';
  export const prerender = false;

  let propertyId: number | null = null;
  let property: Property | null = null;
  let originalImages: { filename: string; dataUrl: string; loading: boolean }[] = [];
  let internetImages: { filename: string; dataUrl: string; loading: boolean }[] = [];
  let error = '';
  let loading = true;
  let copyingImages = false;
  let copyProgress = { current: 0, total: 0 };

  // Get the id from the URL params
  $: propertyId = Number($page.params.id);

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

      await loadImages();
    } catch (e) {
      error = `Failed to load property: ${e}`;
    } finally {
      loading = false;
    }
  });

  async function loadImages() {
    if (!property) return;

    try {
      // Load original images
      await loadOriginalImages();
      // Load INTERNET folder images
      await loadInternetImages();
    } catch (e) {
      error = `Failed to load images: ${e}`;
    }
  }

  async function loadOriginalImages() {
    if (!property) return;

    const response = await invoke('list_original_images', {
      folderPath: property.folder_path
    });

    if (Array.isArray(response)) {
      originalImages = response.map((filename) => ({
        filename,
        dataUrl: '',
        loading: true
      }));

      // Load thumbnails for original images
      for (let i = 0; i < originalImages.length; i++) {
        const image = originalImages[i];
        try {
          const base64Data = await invoke('get_image_as_base64', {
            folderPath: property.folder_path,
            filename: image.filename
          });

          const ext = image.filename.split('.').pop()?.toLowerCase() || '';
          const mimeType = getMimeType(ext);

          originalImages[i] = {
            ...image,
            dataUrl: `data:${mimeType};base64,${base64Data}`,
            loading: false
          };
        } catch (e) {
          originalImages[i] = { ...image, dataUrl: '', loading: false };
        }
      }
    }
  }

  async function loadInternetImages() {
    if (!property) return;

    try {
      const response = await invoke('list_internet_images', {
        folderPath: property.folder_path
      });

      if (Array.isArray(response)) {
        internetImages = response.map((filename) => ({
          filename,
          dataUrl: '',
          loading: true
        }));

        // Load thumbnails for INTERNET images
        for (let i = 0; i < internetImages.length; i++) {
          const image = internetImages[i];
          try {
            const base64Data = await invoke('get_internet_image_as_base64', {
              folderPath: property.folder_path,
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
      // INTERNET folder might not exist yet, that's ok
      internetImages = [];
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

  async function copyAllToInternet() {
    if (!property) return;

    try {
      copyingImages = true;
      copyProgress = { current: 0, total: originalImages.length };

      const result = await invoke('copy_images_to_internet', {
        folderPath: property.folder_path
      });

      if (result.success) {
        // Reload INTERNET images after copying
        await loadInternetImages();
      } else {
        error = result.error || 'Failed to copy images to INTERNET folder';
      }
    } catch (e) {
      error = `Failed to copy images: ${e}`;
    } finally {
      copyingImages = false;
    }
  }

  async function openImageInEditor(filename: string, isFromInternet: boolean = false) {
    if (!property) return;

    try {
      const result = await invoke('open_image_in_editor', {
        folderPath: property.folder_path,
        filename,
        isFromInternet
      });

      if (!result.success) {
        error = result.error || 'Failed to open image in editor';
      }
    } catch (e) {
      error = `Failed to open image: ${e}`;
    }
  }

  async function clearInternetFolder() {
    if (
      !confirm(
        'Are you sure you want to clear all images from the INTERNET folder? This action cannot be undone.'
      )
    ) {
      return;
    }

    try {
      const result = await invoke('clear_internet_folder', {
        folderPath: property.folder_path
      });

      if (result.success) {
        internetImages = [];
      } else {
        error = result.error || 'Failed to clear INTERNET folder';
      }
    } catch (e) {
      error = `Failed to clear folder: ${e}`;
    }
  }
</script>

{#if loading}
  <div class="flex h-64 items-center justify-center">
    <div class="text-center">
      <div
        class="border-accent-500 mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-t-transparent"
      ></div>
      <p class="text-foreground-600 font-medium">Loading property data...</p>
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
        <p class="font-semibold text-red-800">{error}</p>
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
              d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
            />
          </svg>
        </div>
        <div>
          <h1 class="text-foreground-900 text-2xl font-bold">Step 1: Copy to INTERNET</h1>
          <p class="text-foreground-600">
            Copy your original images to the INTERNET folder for editing
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
              <li>• Original images are duplicated to the INTERNET folder</li>
              <li>• Click on any image to open it in your preferred editor</li>
              <li>• Edit images as needed for web publication</li>
              <li>• Proceed to Step 2 when editing is complete</li>
            </ul>
          </div>
        </div>
      </div>
    </div> -->

    <!-- Progress Section -->
    {#if copyingImages}
      <div class="bg-accent-50 border-accent-200 rounded-xl border p-6">
        <div class="flex items-center space-x-4">
          <div
            class="border-accent-500 h-6 w-6 animate-spin rounded-full border-2 border-t-transparent"
          ></div>
          <div>
            <p class="text-accent-900 font-semibold">Copying images to INTERNET folder...</p>
            <p class="text-accent-700 text-sm">
              {copyProgress.current} of {copyProgress.total} images copied
            </p>
          </div>
        </div>
        <div class="bg-accent-200 mt-4 h-2 w-full overflow-hidden rounded-full">
          <div
            class="bg-accent-500 h-full rounded-full transition-all duration-300"
            style="width: {copyProgress.total > 0
              ? (copyProgress.current / copyProgress.total) * 100
              : 0}%"
          ></div>
        </div>
      </div>
    {/if}

    <!-- Statistics Summary -->
    <div class="grid grid-cols-1 gap-6 md:grid-cols-2">
      <div class="bg-background-50 border-background-200 rounded-xl border p-6">
        <div class="flex items-center space-x-3">
          <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-green-100">
            <svg
              class="h-5 w-5 text-green-600"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
              />
            </svg>
          </div>
          <div>
            <p class="text-foreground-500 text-sm font-medium">Original Images</p>
            <p class="text-foreground-900 text-2xl font-bold">{originalImages.length}</p>
          </div>
        </div>
      </div>

      <div class="bg-background-50 border-background-200 rounded-xl border p-6">
        <div class="flex items-center space-x-3">
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
                d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
              />
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M8 5a2 2 0 012-2h4a2 2 0 012 2v6H8V5z"
              />
            </svg>
          </div>
          <div>
            <p class="text-foreground-500 text-sm font-medium">INTERNET Folder</p>
            <p class="text-foreground-900 text-2xl font-bold">{internetImages.length}</p>
          </div>
        </div>
      </div>
    </div>

    <!-- INTERNET Images Section -->
    <section class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-6 flex items-center justify-between">
        <div class="flex items-center space-x-3">
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
                d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
              />
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M8 5a2 2 0 012-2h4a2 2 0 012 2v6H8V5z"
              />
            </svg>
          </div>
          <div>
            <h2 class="text-foreground-900 text-xl font-semibold">
              INTERNET Folder ({internetImages.length})
            </h2>
            <p class="text-foreground-600 text-sm">Click images to open in your editor</p>
          </div>
        </div>

        <div class="flex items-center space-x-3">
          {#if internetImages.length > 0}
            <button
              onclick={clearInternetFolder}
              disabled={copyingImages}
              class="flex items-center space-x-2 rounded-lg border border-red-200 bg-red-50 px-4 py-2 text-sm font-medium text-red-700 transition-colors hover:bg-red-100 disabled:opacity-50"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                />
              </svg>
              <span>Clear Folder</span>
            </button>
          {/if}

          <button
            onclick={loadInternetImages}
            class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 flex items-center space-x-2 rounded-lg border px-4 py-2 text-sm font-medium transition-colors"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
              />
            </svg>
            <span>Refresh</span>
          </button>
        </div>
      </div>

      {#if internetImages.length === 0}
        <div class="py-16 text-center">
          <div
            class="bg-background-100 mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full"
          >
            <svg
              class="text-foreground-400 h-10 w-10"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
              />
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M8 5a2 2 0 012-2h4a2 2 0 012 2v6H8V5z"
              />
            </svg>
          </div>
          <h3 class="text-foreground-900 mb-2 text-lg font-semibold">INTERNET folder is empty</h3>
          <p class="text-foreground-500 mx-auto mb-6 max-w-md">
            Copy your original images to the INTERNET folder to start editing them for web
            publication.
          </p>
          <button
            onclick={copyAllToInternet}
            disabled={copyingImages || originalImages.length === 0}
            class="bg-accent-500 hover:bg-accent-600 inline-flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            {#if copyingImages}
              <div
                class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
              ></div>
              <span>Copying...</span>
            {:else}
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                />
              </svg>
              <span>Copy All to INTERNET ({originalImages.length} images)</span>
            {/if}
          </button>
        </div>
      {:else}
        <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
          {#each internetImages as image}
            <div class="group relative">
              <button
                class="border-background-200 bg-background-100 hover:border-accent-200 aspect-square w-full overflow-hidden rounded-xl border transition-all duration-200 hover:scale-[1.02] hover:shadow-lg"
                onclick={() => openImageInEditor(image.filename, true)}
              >
                {#if image.loading}
                  <!-- Loading state -->
                  <div class="bg-background-100 flex h-full w-full items-center justify-center">
                    <div class="text-center">
                      <div
                        class="border-accent-500 mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                      <span class="text-foreground-500 text-xs font-medium">Loading...</span>
                    </div>
                  </div>
                {:else if image.dataUrl}
                  <!-- Actual image -->
                  <img
                    src={image.dataUrl}
                    alt={image.filename}
                    loading="lazy"
                    class="h-full w-full object-cover transition-transform duration-300"
                  />
                {:else}
                  <!-- Error fallback -->
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
                      <span class="text-xs font-medium">Failed to load</span>
                    </div>
                  </div>
                {/if}

                <!-- Filename overlay -->
                <div
                  class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black via-black/60 to-transparent p-3 pt-8 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                >
                  <p class="truncate text-xs font-medium text-white" title={image.filename}>
                    {image.filename}
                  </p>
                </div>

                <!-- Edit indicator -->
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
                        d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
                      />
                    </svg>
                  </div>
                </div>
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- Next Step Navigation -->
    <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="flex items-center justify-between">
        <div class="flex items-center space-x-4">
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
                d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
          </div>
          <div>
            <h3 class="text-foreground-900 font-semibold">Ready for the next step?</h3>
            <p class="text-foreground-600 text-sm">
              Once you've finished editing your images, proceed to step 2 for ordering and renaming.
            </p>
          </div>
        </div>

        <a
          href="/properties/{property.id}/step2"
          class="inline-flex items-center space-x-2 rounded-lg px-6 py-3 font-medium transition-colors {internetImages.length ===
          0
            ? 'bg-background-200 text-foreground-500 cursor-not-allowed'
            : 'bg-accent-500 hover:bg-accent-600 text-white'}"
          class:pointer-events-none={internetImages.length === 0}
        >
          <span>Step 2: Order & Rename</span>
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
{/if}
