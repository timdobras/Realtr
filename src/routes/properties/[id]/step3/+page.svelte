<script lang="ts">
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
  import { onMount } from 'svelte';
  import { showSuccess, showError } from '$lib/stores/notification';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  export const prerender = false;

  interface ImageItem {
    filename: string;
    dataUrl: string;
    loading: boolean;
    selected: boolean;
    inAggelia: boolean;
  }

  let property: Property | null = $state(null);
  let internetImages: ImageItem[] = $state([]);
  let aggeliaImages: ImageItem[] = $state([]);
  let error = $state('');
  let loading = $state(true);
  let copyingImages = $state(false);
  let showClearConfirm = $state(false);

  // Get the id from the URL params
  let propertyId = $derived(Number($page.params.id));

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
      folderPath: property.folder_path,
      status: property.status
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
        folderPath: property.folder_path,
        status: property.status
      });

      const aggeliaFiles = Array.isArray(aggeliaFileList) ? aggeliaFileList : [];

      // Load thumbnails and mark AGGELIA status
      for (let i = 0; i < internetImages.length; i++) {
        const image = internetImages[i];

        image.inAggelia = aggeliaFiles.includes(image.filename);

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
  }

  async function loadAggeliaImages() {
    if (!property) return;

    try {
      const response = await invoke('list_aggelia_images', {
        folderPath: property.folder_path,
        status: property.status
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
              status: property.status,
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

    try {
      copyingImages = true;
      error = '';

      const filenames = selectedImages.map((img) => img.filename);

      const result: any = await invoke('copy_images_to_aggelia', {
        folderPath: property.folder_path,
        status: property.status,
        filenames
      });

      if (result.success) {
        const count = selectedImages.length;
        await loadImages();
        // Reset selections after copying
        deselectAllImages();
        showSuccess(`Copied ${count} images to AGGELIA folder`);
      } else {
        showError(result.error || 'Failed to copy images to AGGELIA');
      }
    } catch (e) {
      showError(`Failed to copy images: ${e}`);
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

  function clearAggeliaFolder() {
    if (!property) return;
    showClearConfirm = true;
  }

  async function doClearAggelia() {
    if (!property) return;
    showClearConfirm = false;

    try {
      const result: any = await invoke('clear_aggelia_folder', {
        folderPath: property.folder_path,
        status: property.status
      });

      if (result.success) {
        await loadImages();
        showSuccess('AGGELIA folder cleared');
      } else {
        showError(result.error || 'Failed to clear AGGELIA folder');
      }
    } catch (e) {
      showError(`Failed to clear folder: ${e}`);
    }
  }

  // Get counts for UI
  let selectedCount = $derived(internetImages.filter((img) => img.selected).length);
  let availableCount = $derived(internetImages.filter((img) => !img.inAggelia).length);
  let allAvailableSelected = $derived(
    availableCount > 0 &&
      internetImages.filter((img) => !img.inAggelia && img.selected).length === availableCount
  );
  let anySelected = $derived(selectedCount > 0);
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

    <!-- Progress -->
    {#if copyingImages}
      <div class="bg-background-100 border-background-300 border p-4">
        <div class="flex items-center space-x-3">
          <div
            class="border-foreground-300 h-5 w-5 animate-spin border-2 border-t-transparent"
          ></div>
          <div>
            <p class="text-foreground-900 font-semibold">Copying images to AGGELIA folder...</p>
            <p class="text-foreground-600 text-sm">Please wait while images are being copied.</p>
          </div>
        </div>
      </div>
    {/if}

    <!-- Statistics -->
    <div class="grid grid-cols-1 gap-4 md:grid-cols-3">
      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">Total Images</p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">{internetImages.length}</p>
      </div>

      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">In AGGELIA</p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">
          {internetImages.filter((img) => img.inAggelia).length}
        </p>
      </div>

      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">Available</p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">{availableCount}</p>
      </div>
    </div>

    <!-- Selection Controls -->
    <div class="bg-background-50 border-background-200 border p-4">
      <div class="mb-3">
        <h2 class="text-foreground-900 text-sm font-semibold">Selection Controls</h2>
        <p class="text-foreground-600 text-xs">
          Choose which images to copy to AGGELIA for advanced editing
        </p>
      </div>

      <div
        class="flex flex-col space-y-3 md:flex-row md:items-center md:justify-between md:space-y-0"
      >
        <div class="flex flex-wrap items-center gap-3">
          <button
            onclick={selectAllImages}
            disabled={availableCount === 0 || allAvailableSelected}
            class="bg-background-100 border-background-300 text-foreground-700 hover:bg-background-200 border px-4 py-2 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            Select All ({availableCount})
          </button>

          <button
            onclick={deselectAllImages}
            disabled={!anySelected}
            class="bg-background-100 border-background-300 text-foreground-700 hover:bg-background-200 border px-4 py-2 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            Deselect All
          </button>

          <div class="bg-background-100 text-foreground-700 border-background-200 border px-3 py-2">
            <span class="text-sm font-medium">{selectedCount} selected</span>
          </div>
        </div>

        <div class="flex flex-wrap items-center gap-3">
          <button
            onclick={copySelectedToAggelia}
            disabled={copyingImages || selectedCount === 0}
            class="bg-accent-500 hover:bg-accent-600 px-4 py-2 text-sm font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            {#if copyingImages}
              <div class="h-4 w-4 animate-spin border-2 border-white border-t-transparent"></div>
              <span>Copying...</span>
            {:else}
              <span>Copy to AGGELIA ({selectedCount})</span>
            {/if}
          </button>

          {#if aggeliaImages.length > 0}
            <button
              onclick={clearAggeliaFolder}
              disabled={copyingImages}
              class="bg-background-200 text-foreground-700 hover:bg-background-300 border-background-300 border px-4 py-2 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
            >
              <span>Clear AGGELIA</span>
            </button>
          {/if}
        </div>
      </div>
    </div>

    <!-- INTERNET Images -->
    <section class="bg-background-50 border-background-200 border">
      <div class="p-4">
        <div class="mb-3">
          <h2 class="text-foreground-900 text-sm font-semibold">
            INTERNET Folder ({internetImages.length})
          </h2>
          <p class="text-foreground-600 text-xs">Click to select images for AGGELIA</p>
        </div>

        {#if internetImages.length === 0}
          <div class="py-8 text-center">
            <p class="text-foreground-500 mb-3 text-sm">No images in INTERNET folder</p>
            <p class="text-foreground-600 mb-4 text-xs">
              Complete Step 1 and Step 2 before copying images to AGGELIA.
            </p>
            <a
              href="/properties/{property.id}/step1"
              class="bg-accent-500 hover:bg-accent-600 inline-flex items-center space-x-2 px-4 py-2 text-sm font-medium text-white transition-colors"
            >
              <span>Back to Step 1</span>
            </a>
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {#each internetImages as image, index}
              <button
                class="bg-background-100 border-background-200 group relative aspect-square w-full overflow-hidden border transition-opacity hover:opacity-75 {image.selected
                  ? 'border-accent-500'
                  : ''} {image.inAggelia ? 'opacity-50' : ''}"
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
                        class="border-foreground-300 mx-auto mb-2 h-5 w-5 animate-spin border-2 border-t-transparent"
                      ></div>
                      <p class="text-foreground-500 text-xs font-medium">Loading...</p>
                    </div>
                  </div>
                {:else if image.dataUrl}
                  <img
                    src={image.dataUrl}
                    alt={image.filename}
                    loading="lazy"
                    class="h-full w-full object-cover"
                  />
                {:else}
                  <div class="bg-background-100 flex h-full w-full items-center justify-center">
                    <p class="text-foreground-500 text-xs">Failed</p>
                  </div>
                {/if}

                <!-- Selected indicator -->
                {#if image.selected}
                  <div class="bg-accent-500 absolute top-2 left-2 h-5 w-5">
                    <svg class="h-5 w-5 text-white" fill="currentColor" viewBox="0 0 20 20">
                      <path
                        fill-rule="evenodd"
                        d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
                        clip-rule="evenodd"
                      />
                    </svg>
                  </div>
                {/if}

                <!-- Filename on hover -->
                <div
                  class="bg-foreground-900/75 absolute inset-x-0 bottom-0 p-2 opacity-0 transition-opacity group-hover:opacity-100"
                >
                  <p class="truncate text-xs text-white" title={image.filename}>
                    {image.filename}
                    {#if image.inAggelia}
                      <span class="text-foreground-300 text-xs"> (In AGGELIA)</span>
                    {/if}
                  </p>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    <!-- AGGELIA Images -->
    <section class="bg-background-50 border-background-200 border">
      <div class="p-4">
        <div class="mb-3">
          <h2 class="text-foreground-900 text-sm font-semibold">
            AGGELIA Folder ({aggeliaImages.length})
          </h2>
          <p class="text-foreground-600 text-xs">Click images to open in your advanced editor</p>
        </div>

        {#if aggeliaImages.length === 0}
          <div class="py-8 text-center">
            <p class="text-foreground-500 mb-3 text-sm">AGGELIA folder is empty</p>
            <p class="text-foreground-600 text-xs">
              Select images from the INTERNET folder above and copy them to AGGELIA.
            </p>
          </div>
        {:else}
          <div class="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            {#each aggeliaImages as image}
              <button
                class="bg-background-100 border-background-200 group relative aspect-square w-full overflow-hidden border transition-opacity hover:opacity-75"
                onclick={() => openImageInAdvancedEditor(image.filename, true)}
                disabled={copyingImages}
              >
                {#if image.loading}
                  <div class="bg-background-100 flex h-full w-full items-center justify-center">
                    <div class="text-center">
                      <div
                        class="border-foreground-300 mx-auto mb-2 h-5 w-5 animate-spin border-2 border-t-transparent"
                      ></div>
                      <p class="text-foreground-500 text-xs font-medium">Loading...</p>
                    </div>
                  </div>
                {:else if image.dataUrl}
                  <img
                    src={image.dataUrl}
                    alt={image.filename}
                    loading="lazy"
                    class="h-full w-full object-cover"
                  />
                {:else}
                  <div class="bg-background-100 flex h-full w-full items-center justify-center">
                    <p class="text-foreground-500 text-xs">Failed</p>
                  </div>
                {/if}

                <!-- Filename on hover -->
                <div
                  class="bg-foreground-900/75 absolute inset-x-0 bottom-0 p-2 opacity-0 transition-opacity group-hover:opacity-100"
                >
                  <p class="truncate text-xs text-white" title={image.filename}>
                    {image.filename}
                  </p>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    <!-- Next Step -->
    <div class="bg-background-50 border-background-200 border p-4">
      <div
        class="flex flex-col space-y-3 md:flex-row md:items-center md:justify-between md:space-y-0"
      >
        <div>
          <h3 class="text-foreground-900 text-sm font-semibold">Ready for the final step?</h3>
          <p class="text-foreground-600 text-xs">
            Proceed to Step 4 for watermarking once editing is complete.
          </p>
        </div>

        <a
          href="/properties/{property.id}/step4"
          class="inline-flex items-center space-x-2 px-4 py-2 text-sm font-medium transition-colors {aggeliaImages.length ===
          0
            ? 'bg-background-200 text-foreground-500 cursor-not-allowed'
            : 'bg-accent-500 hover:bg-accent-600 text-white'}"
          class:pointer-events-none={aggeliaImages.length === 0}
        >
          <span>Step 4: Add Watermark</span>
        </a>
      </div>
    </div>
  </div>

  <!-- Clear AGGELIA Confirmation Dialog -->
  {#if showClearConfirm}
    <ConfirmDialog
      title="Clear AGGELIA Folder"
      message="Are you sure you want to clear all images from the AGGELIA folder? This action cannot be undone."
      confirmText="Clear"
      destructive={true}
      onConfirm={doClearAggelia}
      onCancel={() => (showClearConfirm = false)}
    />
  {/if}
{/if}
