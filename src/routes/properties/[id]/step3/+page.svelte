<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
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
        class="mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-blue-500 border-t-transparent"
      ></div>
      <p class="text-foreground-600">Loading images...</p>
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
    <!-- Progress Section -->
    {#if copyingImages}
      <div class="rounded-lg border border-blue-200 bg-blue-50 p-4">
        <div class="flex items-center space-x-3">
          <div
            class="h-5 w-5 animate-spin rounded-full border-2 border-blue-500 border-t-transparent"
          ></div>
          <div>
            <p class="font-medium text-blue-900">Copying images to AGGELIA folder...</p>
            <p class="text-sm text-blue-700">Please wait while images are being copied.</p>
          </div>
        </div>
      </div>
    {/if}

    <!-- Action Controls -->
    <div class="bg-background-100 border-background-200 rounded-xl border p-6 shadow-sm">
      <h2 class="text-foreground-900 mb-6 text-lg font-semibold">Selection Controls</h2>

      <div class="mb-4 flex items-center justify-between">
        <div class="flex items-center space-x-6">
          <!-- Selection Buttons -->
          <div class="flex items-center space-x-3">
            <button
              onclick={selectAllImages}
              disabled={availableCount === 0 || allAvailableSelected}
              class="rounded-lg border border-blue-300 bg-blue-100 px-4 py-2 text-sm font-medium text-blue-700 transition-colors hover:bg-blue-200 disabled:cursor-not-allowed disabled:opacity-50"
            >
              ✅ Select All ({availableCount})
            </button>

            <button
              onclick={deselectAllImages}
              disabled={!anySelected}
              class="rounded-lg border border-gray-300 bg-gray-100 px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-200 disabled:cursor-not-allowed disabled:opacity-50"
            >
              ❌ Deselect All
            </button>
          </div>

          <div class="rounded-full bg-blue-100 px-3 py-1 text-sm font-medium text-blue-800">
            {selectedCount} selected
          </div>
        </div>

        <div class="flex space-x-3">
          <button
            onclick={copySelectedToAggelia}
            disabled={copyingImages || selectedCount === 0}
            class="btn-primary flex cursor-pointer items-center space-x-2 rounded-lg bg-blue-700 px-4 py-2 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <span>📁→✨</span>
            <span>Copy to AGGELIA ({selectedCount})</span>
          </button>

          <button
            onclick={clearAggeliaFolder}
            disabled={copyingImages || aggeliaImages.length === 0}
            class="flex items-center space-x-2 rounded-lg bg-red-600 px-4 py-2 font-medium text-white transition-colors hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <span>🗑️</span>
            <span>Clear AGGELIA</span>
          </button>
        </div>
      </div>

      <!-- Selection Stats -->
      <div class="border-background-300 grid grid-cols-3 gap-4 border-t pt-4">
        <div class="text-center">
          <div class="text-foreground-900 text-2xl font-bold">{internetImages.length}</div>
          <div class="text-foreground-600 text-sm">Total Images</div>
        </div>
        <div class="text-center">
          <div class="text-2xl font-bold text-green-600">
            {internetImages.filter((img) => img.inAggelia).length}
          </div>
          <div class="text-foreground-600 text-sm">In AGGELIA</div>
        </div>
        <div class="text-center">
          <div class="text-2xl font-bold text-blue-600">{availableCount}</div>
          <div class="text-foreground-600 text-sm">Available</div>
        </div>
      </div>
    </div>

    <!-- INTERNET Images Section -->
    <section class="bg-background-100 border-background-200 rounded-xl border shadow-sm">
      <div class="p-6">
        <div class="mb-6 flex items-center justify-between">
          <h2 class="text-foreground-900 text-xl font-semibold">
            INTERNET Folder Images ({internetImages.length})
          </h2>
          {#if internetImages.length > 0}
            <div class="text-foreground-500 text-sm">
              Click images to select • Green badges indicate already copied
            </div>
          {/if}
        </div>

        {#if internetImages.length === 0}
          <div class="py-16 text-center">
            <div
              class="bg-background-200 mx-auto mb-4 flex h-24 w-24 items-center justify-center rounded-full"
            >
              <span class="text-3xl">📁</span>
            </div>
            <h3 class="text-foreground-700 mb-2 text-lg font-medium">
              No images in INTERNET folder
            </h3>
            <p class="text-foreground-500 mb-6">Go back to Step 1 to copy images first.</p>
            <a href="/properties/{property.id}/step1" class="btn-primary"> ← Back to Step 1 </a>
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
            {#each internetImages as image, index}
              <div class="group relative">
                <button
                  class="aspect-square w-full overflow-hidden rounded-xl border-2 bg-white transition-all duration-200 {image.inAggelia
                    ? 'border-green-400 shadow-lg shadow-green-100'
                    : image.selected
                      ? 'scale-105 border-blue-400 shadow-lg shadow-blue-100'
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
                          class="mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-3 border-blue-500 border-t-transparent"
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

                  <!-- Status Indicators -->
                  {#if image.inAggelia}
                    <div
                      class="absolute top-2 left-2 flex items-center space-x-1 rounded-full bg-green-500 px-2 py-1 text-xs text-white shadow-lg"
                    >
                      <span>✨</span>
                      <span class="hidden sm:inline">In AGGELIA</span>
                    </div>
                  {:else if image.selected}
                    <div
                      class="absolute top-2 left-2 flex h-6 w-6 items-center justify-center rounded-full bg-blue-500 text-white shadow-lg"
                    >
                      <svg class="h-4 w-4" fill="currentColor" viewBox="0 0 20 20">
                        <path
                          fillRule="evenodd"
                          d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
                          clipRule="evenodd"
                        />
                      </svg>
                    </div>
                  {:else}
                    <div
                      class="border-background-300 absolute top-2 left-2 h-6 w-6 rounded-full border-2 bg-white/80 opacity-0 shadow-sm transition-opacity group-hover:opacity-100"
                    ></div>
                  {/if}

                  <!-- Filename -->
                  <div
                    class="absolute right-0 bottom-0 left-0 bg-gradient-to-t from-black/80 via-black/60 to-transparent p-3 pt-6"
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

    <!-- AGGELIA Images Section -->
    <section class="bg-background-100 border-background-200 rounded-xl border shadow-sm">
      <div class="p-6">
        <div class="mb-6 flex items-center justify-between">
          <h2 class="text-foreground-900 text-xl font-semibold">
            AGGELIA Folder - Advanced Editing ({aggeliaImages.length})
          </h2>
          {#if aggeliaImages.length > 0}
            <div class="text-foreground-500 text-sm">Click images to open in advanced editor</div>
          {/if}
        </div>

        {#if aggeliaImages.length === 0}
          <div class="py-16 text-center">
            <div
              class="mx-auto mb-4 flex h-24 w-24 items-center justify-center rounded-full bg-purple-100"
            >
              <span class="text-3xl">✨</span>
            </div>
            <h3 class="text-foreground-700 mb-2 text-lg font-medium">
              No images in AGGELIA folder yet
            </h3>
            <p class="text-foreground-500 mb-6">
              Select images above and copy them to AGGELIA for advanced editing.
            </p>
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
            {#each aggeliaImages as image}
              <div class="group relative">
                <button
                  class="aspect-square w-full overflow-hidden rounded-xl border-2 border-purple-400 bg-white shadow-lg shadow-purple-100 transition-all duration-200 hover:scale-105 hover:shadow-xl"
                  onclick={() => openImageInAdvancedEditor(image.filename, true)}
                  disabled={copyingImages}
                >
                  {#if image.loading}
                    <div class="bg-background-100 flex h-full w-full items-center justify-center">
                      <div class="text-center">
                        <div
                          class="mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-3 border-purple-500 border-t-transparent"
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

                  <!-- Advanced editing indicator -->
                  <div
                    class="absolute top-2 right-2 flex items-center space-x-1 rounded-full bg-purple-500 px-2 py-1 text-xs text-white shadow-lg"
                  >
                    <span>🎨</span>
                    <span class="hidden sm:inline">Advanced</span>
                  </div>

                  <!-- Filename -->
                  <div
                    class="absolute right-0 bottom-0 left-0 bg-gradient-to-t from-purple-900/80 via-purple-800/60 to-transparent p-3 pt-6"
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

    <!-- Next Step Navigation -->
    <div
      class="rounded-xl border border-green-200 bg-gradient-to-r from-green-50 to-emerald-50 p-6"
    >
      <div class="flex items-center justify-between">
        <div>
          <h3 class="mb-2 font-semibold text-green-900">Ready for the final step?</h3>
          <p class="text-sm text-green-700">
            Once you've edited your images in AGGELIA, proceed to Step 4 for watermarking.
          </p>
        </div>
        <a
          href="/properties/{property.id}/step4"
          class="btn-primary {aggeliaImages.length === 0
            ? 'pointer-events-none cursor-not-allowed opacity-50'
            : ''}"
        >
          Step 4: Add Watermark →
        </a>
      </div>
    </div>
    <!-- ✅ Correct closing tag -->
  </div>
{/if}

<style>
</style>
