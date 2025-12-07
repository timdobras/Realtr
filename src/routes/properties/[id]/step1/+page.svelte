<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
  import { showSuccess, showError } from '$lib/stores/notification';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import LazyImage from '$lib/components/LazyImage.svelte';
  export const prerender = false;

  let property: Property | null = $state(null);
  // Only store filenames, not data URLs - LazyImage handles loading
  let originalImageFilenames: string[] = $state([]);
  let internetImageFilenames: string[] = $state([]);
  let error = $state('');
  let loading = $state(true);
  let copyingImages = $state(false);
  let copyProgress = $state({ current: 0, total: 0 });
  let showClearConfirm = $state(false);

  // Get the id from the URL params
  let propertyId = $derived(Number($page.params.id));

  onMount(async () => {
    if (isNaN(propertyId) || propertyId < 1) {
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
      // Load original images (just filenames)
      await loadOriginalImages();
      // Load INTERNET folder images (just filenames)
      await loadInternetImages();

      // Pre-generate thumbnails in parallel for faster display
      pregenerateThumbnails();
    } catch (e) {
      error = `Failed to load images: ${e}`;
    }
  }

  // Pre-generate thumbnails in background (don't await - fire and forget)
  function pregenerateThumbnails() {
    if (!property) return;
    // Pre-generate INTERNET folder thumbnails in parallel
    DatabaseService.pregenerateGalleryThumbnails(property.folder_path, property.status, 'INTERNET');
  }

  async function loadOriginalImages() {
    if (!property) return;

    const response = await invoke('list_original_images', {
      folderPath: property.folder_path,
      status: property.status
    });

    if (Array.isArray(response)) {
      originalImageFilenames = response as string[];
    }
  }

  async function loadInternetImages() {
    if (!property) return;

    try {
      const response = await invoke('list_internet_images', {
        folderPath: property.folder_path,
        status: property.status
      });

      if (Array.isArray(response)) {
        internetImageFilenames = response as string[];
      }
    } catch (e) {
      // INTERNET folder might not exist yet, that's ok
      internetImageFilenames = [];
    }
  }

  async function copyAllToInternet() {
    if (!property) return;

    try {
      copyingImages = true;
      copyProgress = { current: 0, total: originalImageFilenames.length };

      const result: any = await invoke('copy_images_to_internet', {
        folderPath: property.folder_path,
        status: property.status
      });

      if (result.success) {
        // Reload INTERNET images after copying
        await loadInternetImages();
        showSuccess(`Copied ${originalImageFilenames.length} images to INTERNET folder`);
      } else {
        showError(result.error || 'Failed to copy images to INTERNET folder');
      }
    } catch (e) {
      showError(`Failed to copy images: ${e}`);
    } finally {
      copyingImages = false;
    }
  }

  async function openImageInEditor(filename: string, isFromInternet: boolean = false) {
    if (!property) return;

    try {
      const result: any = await invoke('open_image_in_editor', {
        folderPath: property.folder_path,
        status: property.status,
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

  function clearInternetFolder() {
    if (!property) return;
    showClearConfirm = true;
  }

  async function doClearInternet() {
    if (!property) return;
    showClearConfirm = false;

    try {
      const result: any = await invoke('clear_internet_folder', {
        folderPath: property.folder_path,
        status: property.status
      });

      if (result.success) {
        internetImageFilenames = [];
        showSuccess('INTERNET folder cleared');
      } else {
        showError(result.error || 'Failed to clear INTERNET folder');
      }
    } catch (e) {
      showError(`Failed to clear folder: ${e}`);
    }
  }
</script>

{#if loading}
  <div class="flex h-64 items-center justify-center">
    <div class="text-foreground-500 flex items-center gap-2 text-sm">
      <div class="border-foreground-300 h-4 w-4 animate-spin border-2 border-t-transparent"></div>
      <span>Loading...</span>
    </div>
  </div>
{:else if error}
  <div class="p-6">
    <div class="border-background-300 bg-background-100 border px-3 py-2">
      <p class="text-foreground-900 text-sm">{error}</p>
    </div>
  </div>
{:else if property}
  <div class="min-h-full space-y-5 p-5">
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
      <div class="bg-background-100 border-background-300 border p-4">
        <div class="flex items-center space-x-3">
          <div
            class="border-foreground-300 h-5 w-5 animate-spin border-2 border-t-transparent"
          ></div>
          <div>
            <p class="text-foreground-900 text-sm font-semibold">
              Copying images to INTERNET folder...
            </p>
            <p class="text-foreground-600 text-xs">
              {copyProgress.current} of {copyProgress.total} images copied
            </p>
          </div>
        </div>
        <div class="bg-background-200 mt-3 h-1 w-full overflow-hidden">
          <div
            class="bg-foreground-900 h-full transition-all duration-300"
            style="width: {copyProgress.total > 0
              ? (copyProgress.current / copyProgress.total) * 100
              : 0}%"
          ></div>
        </div>
      </div>
    {/if}

    <!-- Statistics Summary -->
    <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">
          Original Images
        </p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">{originalImageFilenames.length}</p>
      </div>

      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">
          INTERNET Folder
        </p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">{internetImageFilenames.length}</p>
      </div>
    </div>

    <!-- INTERNET Images Section -->
    <section class="bg-background-50 border-background-200 border p-4">
      <div class="mb-4 flex items-center justify-between">
        <div>
          <h2 class="text-foreground-900 text-lg font-semibold">
            INTERNET Folder ({internetImageFilenames.length})
          </h2>
          <p class="text-foreground-600 text-sm">Click images to open in your editor</p>
        </div>

        <div class="flex items-center space-x-3">
          {#if internetImageFilenames.length > 0}
            <button
              onclick={clearInternetFolder}
              disabled={copyingImages}
              class="bg-background-100 border-background-300 text-foreground-700 hover:bg-background-200 flex items-center space-x-2 border px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50"
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
            class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 flex items-center space-x-2 border px-4 py-2 text-sm font-medium transition-colors"
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

      {#if internetImageFilenames.length === 0}
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
            disabled={copyingImages || originalImageFilenames.length === 0}
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
              <span>Copy All to INTERNET ({originalImageFilenames.length} images)</span>
            {/if}
          </button>
        </div>
      {:else}
        <div class="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
          {#each internetImageFilenames as filename}
            <LazyImage
              folderPath={property.folder_path}
              status={property.status}
              subfolder="INTERNET"
              {filename}
              alt={filename}
              class="aspect-square cursor-pointer border border-background-200 hover:border-background-300 transition-colors"
              onclick={() => openImageInEditor(filename, true)}
            />
          {/each}
        </div>
      {/if}
    </section>

    <!-- Next Step Navigation -->
    <div class="bg-background-50 border-background-200 border p-4">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-foreground-900 font-semibold">Ready for the next step?</h3>
          <p class="text-foreground-600 text-sm">
            Once you've finished editing your images, proceed to step 2 for ordering and renaming.
          </p>
        </div>

        <a
          href="/properties/{property.id}/step2"
          class="inline-flex items-center space-x-2 px-6 py-3 font-medium transition-colors {internetImageFilenames.length ===
          0
            ? 'bg-background-200 text-foreground-500 cursor-not-allowed'
            : 'bg-accent-500 hover:bg-accent-600 text-white'}"
          class:pointer-events-none={internetImageFilenames.length === 0}
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

  <!-- Clear Confirmation Dialog -->
  {#if showClearConfirm}
    <ConfirmDialog
      title="Clear INTERNET Folder"
      message="Are you sure you want to clear all images from the INTERNET folder? This action cannot be undone."
      confirmText="Clear"
      destructive={true}
      onConfirm={doClearInternet}
      onCancel={() => (showClearConfirm = false)}
    />
  {/if}
{/if}
