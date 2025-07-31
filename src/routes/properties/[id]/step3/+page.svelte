<script lang="ts">
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
  import { onMount } from 'svelte';
  export const prerender = false;

  interface ImageItem {
    filename: string;
    dataUrl: string;
    loading: boolean;
    selected: boolean;
    inAggelia: boolean;
  }

  let propertyId: number | null = null;
  let property: Property | null = null;
  let internetImages: ImageItem[] = [];
  let aggeliaImages: ImageItem[] = [];
  let error = '';
  let loading = true;
  let copyingImages = false;

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
      await loadInternetImages();
      await loadAggeliaImages();
    } catch (e) {
      error = `Failed to load images: ${e}`;
    }
  }

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

  async function loadInternetImages() {
    if (!property) return;

    const response = await invoke('list_internet_images', {
      folderPath: property.folder_path
    });

    if (Array.isArray(response)) {
      // Sort filenames numerically
      const sortedFilenames = sortImagesByNumericFilename(response);

      internetImages = sortedFilenames.map((filename) => ({
        filename,
        dataUrl: '',
        loading: true,
        selected: false,
        inAggelia: false
      }));

      // Check which images are already in AGGELIA
      const aggeliaFileList = await invoke('list_aggelia_images', {
        folderPath: property.folder_path
      });

      const aggeliaFiles = Array.isArray(aggeliaFileList) ? aggeliaFileList : [];

      // Load thumbnails and mark AGGELIA status
      for (let i = 0; i < internetImages.length; i++) {
        const image = internetImages[i];

        image.inAggelia = aggeliaFiles.includes(image.filename);

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
  }

  async function loadAggeliaImages() {
    if (!property) return;

    try {
      const response = await invoke('list_aggelia_images', {
        folderPath: property.folder_path
      });

      if (Array.isArray(response)) {
        // Sort filenames numerically
        const sortedFilenames = sortImagesByNumericFilename(response);

        aggeliaImages = sortedFilenames.map((filename) => ({
          filename,
          dataUrl: '',
          loading: true,
          selected: false,
          inAggelia: true
        }));

        // Load thumbnails
        for (let i = 0; i < aggeliaImages.length; i++) {
          const image = aggeliaImages[i];
          try {
            const base64Data = await invoke('get_aggelia_image_as_base64', {
              folderPath: property.folder_path,
              filename: image.filename
            });

            const ext = image.filename.split('.').pop()?.toLowerCase() || '';
            const mimeType = getMimeType(ext);

            aggeliaImages[i] = {
              ...image,
              dataUrl: `data:${mimeType};base64,${base64Data}`,
              loading: false
            };
          } catch (e) {
            aggeliaImages[i] = { ...image, dataUrl: '', loading: false };
          }
        }
      }
    } catch (e) {
      aggeliaImages = [];
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

  // Select all available images
  function selectAllImages() {
    internetImages = internetImages.map((img) => ({
      ...img,
      selected: img.inAggelia ? img.selected : true
    }));
  }

  // Deselect all images
  function deselectAllImages() {
    internetImages = internetImages.map((img) => ({
      ...img,
      selected: false
    }));
  }

  function toggleImageSelection(index: number) {
    if (!internetImages[index].inAggelia) {
      internetImages = internetImages.map((img, i) => ({
        ...img,
        selected: i === index ? !img.selected : img.selected
      }));
    }
  }

  async function copySelectedToAggelia() {
    if (!property) return;

    const selectedImages = internetImages.filter((img) => img.selected);
    if (selectedImages.length === 0) {
      error = 'Please select at least one image to copy to AGGELIA';
      return;
    }

    const confirmMessage = `Copy ${selectedImages.length} selected images to AGGELIA folder? This will prepare them for advanced editing.`;

    if (!confirm(confirmMessage)) {
      return;
    }

    try {
      copyingImages = true;
      error = '';

      const filenames = selectedImages.map((img) => img.filename);

      const result = await invoke('copy_images_to_aggelia', {
        folderPath: property.folder_path,
        filenames
      });

      if (result.success) {
        await loadImages();
        // Reset selections after copying
        deselectAllImages();
      } else {
        error = result.error || 'Failed to copy images to AGGELIA';
      }
    } catch (e) {
      error = `Failed to copy images: ${e}`;
    } finally {
      copyingImages = false;
    }
  }

  async function openImageInAdvancedEditor(filename: string, fromAggelia: boolean = false) {
    if (!property) return;

    try {
      const result = await DatabaseService.openWithConfiguredEditor(
        propertyId || 0,
        filename,
        'complex',
        'aggelia'
      );

      if (!result.success) {
        error = result.error || 'Failed to open image in advanced editor';
      }
    } catch (e) {
      error = `Failed to open image: ${e}`;
    }
  }

  async function clearAggeliaFolder() {
    if (
      !confirm(
        'Are you sure you want to clear all images from the AGGELIA folder? This action cannot be undone.'
      )
    ) {
      return;
    }

    try {
      const result = await invoke('clear_aggelia_folder', {
        folderPath: property.folder_path
      });

      if (result.success) {
        await loadImages();
      } else {
        error = result.error || 'Failed to clear AGGELIA folder';
      }
    } catch (e) {
      error = `Failed to clear folder: ${e}`;
    }
  }

  // Get counts for UI
  $: selectedCount = internetImages.filter((img) => img.selected).length;
  $: availableCount = internetImages.filter((img) => !img.inAggelia).length;
  $: allAvailableSelected =
    availableCount > 0 &&
    internetImages.filter((img) => !img.inAggelia && img.selected).length === availableCount;
  $: anySelected = selectedCount > 0;
</script>

{#if loading}
  <div class="flex h-64 items-center justify-center">
    <div class="text-center">
      <div
        class="border-accent-500 mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-t-transparent"
      ></div>
      <p class="text-foreground-600 font-medium">Loading images...</p>
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
          <h1 class="text-foreground-900 text-2xl font-bold">Step 3: Copy to AGGELIA</h1>
          <p class="text-foreground-600">
            Select and copy images to AGGELIA folder for advanced editing
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
              <li>• Select images from INTERNET folder that need advanced editing</li>
              <li>• Copy selected images to AGGELIA folder for complex edits</li>
              <li>• Click AGGELIA images to open in your advanced editor</li>
              <li>• Images with green badges are already in AGGELIA</li>
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
            <p class="text-accent-900 font-semibold">Copying images to AGGELIA folder...</p>
            <p class="text-accent-700 text-sm">
              Please wait while images are being copied for advanced editing.
            </p>
          </div>
        </div>
      </div>
    {/if}

    <!-- Statistics Summary -->
    <div class="grid grid-cols-1 gap-6 md:grid-cols-3">
      <div class="bg-background-50 border-background-200 rounded-xl border p-6">
        <div class="flex items-center space-x-3">
          <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-blue-100">
            <svg
              class="h-5 w-5 text-blue-600"
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
            <p class="text-foreground-500 text-sm font-medium">Total Images</p>
            <p class="text-foreground-900 text-2xl font-bold">{internetImages.length}</p>
          </div>
        </div>
      </div>

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
                d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
          </div>
          <div>
            <p class="text-foreground-500 text-sm font-medium">In AGGELIA</p>
            <p class="text-foreground-900 text-2xl font-bold">
              {internetImages.filter((img) => img.inAggelia).length}
            </p>
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
                d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 100 4m0-4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 100 4m0-4v2m0-6V4"
              />
            </svg>
          </div>
          <div>
            <p class="text-foreground-500 text-sm font-medium">Available</p>
            <p class="text-foreground-900 text-2xl font-bold">{availableCount}</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Selection Controls -->
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
              d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
        </div>
        <div>
          <h2 class="text-foreground-900 text-lg font-semibold">Selection Controls</h2>
          <p class="text-foreground-600 text-sm">
            Choose which images to copy to AGGELIA for advanced editing
          </p>
        </div>
      </div>

      <div class="mb-6 flex items-center justify-between">
        <div class="flex items-center space-x-4">
          <!-- Selection Buttons -->
          <div class="flex items-center space-x-3">
            <button
              onclick={selectAllImages}
              disabled={availableCount === 0 || allAvailableSelected}
              class="flex items-center space-x-2 rounded-lg border border-green-200 bg-green-50 px-4 py-2 text-sm font-medium text-green-700 transition-colors hover:bg-green-100 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
              <span>Select All ({availableCount})</span>
            </button>

            <button
              onclick={deselectAllImages}
              disabled={!anySelected}
              class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 flex items-center space-x-2 rounded-lg border px-4 py-2 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
              <span>Deselect All</span>
            </button>
          </div>

          <div
            class="bg-accent-100 text-accent-800 inline-flex items-center space-x-2 rounded-lg px-3 py-1.5"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 12l2 2 4-4"
              />
            </svg>
            <span class="text-sm font-medium">{selectedCount} selected</span>
          </div>
        </div>

        <div class="flex items-center space-x-3">
          <button
            onclick={copySelectedToAggelia}
            disabled={copyingImages || selectedCount === 0}
            class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
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
              <span>Copy to AGGELIA ({selectedCount})</span>
            {/if}
          </button>

          {#if aggeliaImages.length > 0}
            <button
              onclick={clearAggeliaFolder}
              disabled={copyingImages}
              class="flex items-center space-x-2 rounded-lg border border-red-200 bg-red-50 px-4 py-2 text-sm font-medium text-red-700 transition-colors hover:bg-red-100 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                />
              </svg>
              <span>Clear AGGELIA</span>
            </button>
          {/if}
        </div>
      </div>
    </div>

    <!-- INTERNET Images Section -->
    <section class="bg-background-50 border-background-200 rounded-xl border shadow-sm">
      <div class="p-6">
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
                  d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
                />
              </svg>
            </div>
            <div>
              <h2 class="text-foreground-900 text-xl font-semibold">
                INTERNET Folder ({internetImages.length})
              </h2>
              <p class="text-foreground-600 text-sm">
                Click to select images • Green badges indicate already copied
              </p>
            </div>
          </div>

          {#if internetImages.length > 0}
            <div class="text-foreground-500 flex items-center space-x-2 text-sm">
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M15 15l-2 5L9 9l11 4-5 2zm0 0l5 5M7.188 2.239l.777 2.897M5.136 7.965l-2.898-.777M13.95 4.05l-2.122 2.122m-5.657 5.656l-2.12 2.122"
                />
              </svg>
              <span>Click to select</span>
            </div>
          {/if}
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
            <h3 class="text-foreground-900 mb-2 text-lg font-semibold">
              No images in INTERNET folder
            </h3>
            <p class="text-foreground-500 mx-auto mb-6 max-w-md">
              You need to complete Step 1 and Step 2 before copying images to AGGELIA.
            </p>
            <a
              href="/properties/{property.id}/step1"
              class="bg-accent-500 hover:bg-accent-600 inline-flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors"
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
          <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {#each internetImages as image, index}
              <div class="group relative">
                <button
                  class="bg-background-100 aspect-square w-full overflow-hidden rounded-xl border-2 transition-all duration-200 {image.inAggelia
                    ? 'cursor-pointer border-green-400 shadow-lg hover:shadow-xl'
                    : image.selected
                      ? 'border-accent-400 scale-105 shadow-lg'
                      : 'border-background-300 hover:border-background-400 hover:shadow-md'}"
                  onclick={() =>
                    image.inAggelia
                      ? openImageInAdvancedEditor(image.filename, false)
                      : toggleImageSelection(index)}
                  disabled={copyingImages}
                >
                  {#if image.loading}
                    <div class="bg-background-100 flex h-full w-full items-center justify-center">
                      <div class="text-center">
                        <div
                          class="border-accent-500 mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-2 border-t-transparent"
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

                  <!-- Status Indicators -->
                  {#if image.inAggelia}
                    <div
                      class="absolute top-2 left-2 flex items-center space-x-1 rounded-lg bg-green-500 px-2 py-1 text-xs text-white shadow-lg"
                    >
                      <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M9 12l2 2 4-4"
                        />
                      </svg>
                      <span class="hidden sm:inline">In AGGELIA</span>
                    </div>
                  {:else if image.selected}
                    <div
                      class="bg-accent-500 absolute top-2 left-2 flex h-6 w-6 items-center justify-center rounded-full text-white shadow-lg"
                    >
                      <svg class="h-4 w-4" fill="currentColor" viewBox="0 0 20 20">
                        <path
                          fill-rule="evenodd"
                          d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
                          clip-rule="evenodd"
                        />
                      </svg>
                    </div>
                  {:else}
                    <div
                      class="border-background-300 absolute top-2 left-2 h-6 w-6 rounded-full border-2 bg-white/80 opacity-0 shadow-sm transition-opacity group-hover:opacity-100"
                    ></div>
                  {/if}

                  <!-- Filename overlay -->
                  <div
                    class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black via-black/60 to-transparent p-3 pt-8 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                  >
                    <p class="truncate text-xs font-medium text-white" title={image.filename}>
                      {image.filename}
                    </p>
                  </div>

                  <!-- Action indicator on hover -->
                  <div
                    class="absolute inset-0 flex items-center justify-center bg-black/20 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                  >
                    <div class="rounded-full bg-white/90 p-2">
                      {#if image.inAggelia}
                        <svg
                          class="text-foreground-900 h-4 w-4"
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
                      {:else}
                        <svg
                          class="text-foreground-900 h-4 w-4"
                          fill="none"
                          stroke="currentColor"
                          viewBox="0 0 24 24"
                        >
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M9 12l2 2 4-4"
                          />
                        </svg>
                      {/if}
                    </div>
                  </div>
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    <!-- AGGELIA Images Section -->
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
                AGGELIA Folder - Advanced Editing ({aggeliaImages.length})
              </h2>
              <p class="text-foreground-600 text-sm">
                Click images to open in your advanced editor
              </p>
            </div>
          </div>

          {#if aggeliaImages.length > 0}
            <div class="flex items-center space-x-2 text-sm text-purple-600">
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v1a2 2 0 002 2h6a2 2 0 012 2v8a4 4 0 01-4 4H7z"
                />
              </svg>
              <span>Advanced editing</span>
            </div>
          {/if}
        </div>

        {#if aggeliaImages.length === 0}
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
            <h3 class="text-foreground-900 mb-2 text-lg font-semibold">AGGELIA folder is empty</h3>
            <p class="text-foreground-500 mx-auto mb-6 max-w-md">
              Select images from the INTERNET folder above and copy them to AGGELIA for advanced
              editing.
            </p>
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {#each aggeliaImages as image}
              <div class="group relative">
                <button
                  class="bg-background-100 aspect-square w-full overflow-hidden rounded-xl border-2 border-purple-400 shadow-lg shadow-purple-100 transition-all duration-200 hover:scale-105 hover:shadow-xl"
                  onclick={() => openImageInAdvancedEditor(image.filename, true)}
                  disabled={copyingImages}
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

                  <!-- Advanced editing indicator -->
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
                    <span class="hidden sm:inline">Advanced</span>
                  </div>

                  <!-- Filename overlay -->
                  <div
                    class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-purple-900/80 via-purple-800/60 to-transparent p-3 pt-8 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                  >
                    <p class="truncate text-xs font-medium text-white" title={image.filename}>
                      {image.filename}
                    </p>
                  </div>

                  <!-- Edit indicator on hover -->
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
      </div>
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
            <h3 class="text-foreground-900 font-semibold">Ready for the final step?</h3>
            <p class="text-foreground-600 text-sm">
              Once you've completed your advanced editing, proceed to Step 4 for watermarking.
            </p>
          </div>
        </div>

        <a
          href="/properties/{property.id}/step4"
          class="inline-flex items-center space-x-2 rounded-lg px-6 py-3 font-medium transition-colors {aggeliaImages.length ===
          0
            ? 'bg-background-200 text-foreground-500 cursor-not-allowed'
            : 'bg-accent-500 hover:bg-accent-600 text-white'}"
          class:pointer-events-none={aggeliaImages.length === 0}
        >
          <span>Step 4: Add Watermark</span>
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
