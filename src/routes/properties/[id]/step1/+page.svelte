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
  <div class="text-foreground-600 p-6 text-center">Loading property data...</div>
{:else if error}
  <div class="p-6 font-semibold text-red-600">{error}</div>
{:else if property}
  <div class="min-h-full space-y-6 p-6">
    <!-- Progress Section -->
    {#if copyingImages}
      <div class="rounded-lg border border-blue-200 bg-blue-50 p-4">
        <div class="flex items-center space-x-3">
          <div
            class="h-5 w-5 animate-spin rounded-full border-2 border-blue-500 border-t-transparent"
          ></div>
          <div>
            <p class="font-medium text-blue-900">Copying images to INTERNET folder...</p>
            <p class="text-sm text-blue-700">
              {copyProgress.current} of {copyProgress.total} images copied
            </p>
          </div>
        </div>
      </div>
    {/if}

    <!-- Action Buttons -->
    <!-- <div class="bg-background-100 rounded-lg shadow p-6">
      <h2 class="text-lg font-semibold mb-4">Actions</h2>
      <div class="flex space-x-4">
        <button
          onclick={copyAllToInternet}
          disabled={copyingImages || originalImages.length === 0}
          class="btn-primary disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Copy All to INTERNET ({originalImages.length} images)
        </button>
        
        <button
          onclick={clearInternetFolder}
          disabled={copyingImages || internetImages.length === 0}
          class="bg-red-600 hover:bg-red-700 text-white font-medium px-4 py-2 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          🗑️ Clear INTERNET Folder
        </button>
      </div>
    </div> -->

    <!-- Original Images Section -->
    <!-- <section class="bg-background-100 rounded-lg shadow p-6">
      <h2 class="text-xl font-semibold mb-4">Original Images ({originalImages.length})</h2>
      
      {#if originalImages.length === 0}
        <p class="text-foreground-500">No original images found.</p>
      {:else}
        <div class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4">
          {#each originalImages as image}
            <button 
              class="relative cursor-pointer group rounded-lg overflow-hidden border border-gray-200 hover:shadow-lg transition-shadow" 
              onclick={() => openImageInEditor(image.filename, false)}
            >
              {#if image.loading}
                <div class="w-full h-32 bg-gray-200 flex items-center justify-center">
                  <div class="animate-spin w-4 h-4 border-2 border-blue-500 border-t-transparent rounded-full"></div>
                </div>
              {:else if image.dataUrl}
                <img
                  src={image.dataUrl}
                  alt={image.filename}
                  loading="lazy"
                  class="w-full h-32 object-cover group-hover:scale-105 transition-transform duration-300"
                />
              {:else}
                <div class="w-full h-32 bg-red-100 flex items-center justify-center">
                  <span class="text-red-600 text-xs">Error</span>
                </div>
              {/if}
              
              <div class="absolute bottom-0 left-0 right-0 bg-black bg-opacity-70 text-white text-xs p-1 truncate">
                {image.filename}
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </section> -->

    <!-- INTERNET Images Section -->
    <section class="bg-background-100 rounded-lg p-6 shadow">
      <div class="mb-8 flex w-full flex-row items-center justify-between">
        <h2 class="text-xl font-semibold">INTERNET Folder ({internetImages.length})</h2>
        <button
          class=" bg-background-100 hover:bg-background-300 flex cursor-pointer items-center justify-center rounded p-2 px-4"
          onclick={loadInternetImages}>Refresh</button
        >
      </div>

      {#if internetImages.length === 0}
        <div class="py-8 text-center">
          <span class="mb-4 block text-4xl">📁</span>
          <p class=" mb-2">No images in INTERNET folder yet.</p>
          <p class="text-foreground-700 mb-6 text-sm">
            Click "Copy All to INTERNET" to copy original images here for editing.
          </p>
          <button
            onclick={copyAllToInternet}
            disabled={copyingImages || originalImages.length === 0}
            class="cursor-pointer rounded bg-blue-700 px-4 py-2 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Copy All to INTERNET
          </button>
        </div>
      {:else}
        <div class="flex w-full flex-row flex-wrap gap-4">
          {#each internetImages as image}
            <button
              class="group bg-background-100 relative aspect-square h-64 flex-shrink-0 cursor-pointer overflow-hidden rounded-lg transition-shadow hover:shadow-lg"
              onclick={() => openImageInEditor(image.filename, true)}
            >
              {#if image.loading}
                <div class="flex h-32 w-full items-center justify-center bg-gray-200">
                  <div
                    class="h-4 w-4 animate-spin rounded-full border-2 border-green-500 border-t-transparent"
                  ></div>
                </div>
              {:else if image.dataUrl}
                <img
                  src={image.dataUrl}
                  alt={image.filename}
                  loading="lazy"
                  class="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
                />
              {:else}
                <div class="flex h-32 w-full items-center justify-center bg-red-100">
                  <span class="text-xs text-red-600">Error</span>
                </div>
              {/if}

              <div
                class="bg-background-200 bg-opacity-80 absolute right-0 bottom-0 left-0 truncate p-1 text-xs text-white"
              >
                {image.filename}
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </section>

    <!-- Next Step Navigation -->
    <div class="bg-background-100 rounded-lg p-6 shadow">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-foreground-900 font-semibold">Ready for the next step?</h3>
          <p class="text-foreground-600 mt-1 text-sm">
            Once you've edited your images, proceed to step 2 for ordering and renaming.
          </p>
        </div>
        <a
          href="/properties/{property.id}/step2"
          class="btn-primary {internetImages.length === 0 ? 'cursor-not-allowed opacity-50' : ''}"
          class:disabled={internetImages.length === 0}
        >
          Step 2: Order & Rename →
        </a>
      </div>
    </div>
  </div>
{/if}
