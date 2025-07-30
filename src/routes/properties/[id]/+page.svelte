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
  let error = '';
  let loading = true;
  let folderMessage = '';
  let folderMessageType: 'success' | 'error' | '' = '';

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

      await loadOriginalImages();
    } catch (e) {
      error = `Failed to load property: ${e}`;
    } finally {
      loading = false;
    }
  });

  async function loadOriginalImages() {
    if (!property) return;

    try {
      // Get list of image filenames
      const response = await invoke('list_original_images', {
        folderPath: property.folder_path
      });

      if (Array.isArray(response)) {
        // Initialize array with filenames and loading states
        originalImages = response.map((filename) => ({
          filename,
          dataUrl: '',
          loading: true
        }));

        // Load each image as base64
        for (let i = 0; i < originalImages.length; i++) {
          const image = originalImages[i];
          try {
            const base64Data = await invoke('get_image_as_base64', {
              folderPath: property.folder_path,
              filename: image.filename
            });

            // Determine MIME type based on file extension
            const ext = image.filename.split('.').pop()?.toLowerCase() || '';
            const mimeType = ['jpg', 'jpeg'].includes(ext)
              ? 'image/jpeg'
              : ext === 'png'
                ? 'image/png'
                : ext === 'gif'
                  ? 'image/gif'
                  : ext === 'webp'
                    ? 'image/webp'
                    : ext === 'bmp'
                      ? 'image/bmp'
                      : 'image/jpeg'; // default

            // Update the image with base64 data
            originalImages[i] = {
              ...image,
              dataUrl: `data:${mimeType};base64,${base64Data}`,
              loading: false
            };
          } catch (e) {
            console.error(`Failed to load image ${image.filename}:`, e);
            originalImages[i] = {
              ...image,
              dataUrl: '',
              loading: false
            };
          }
        }
      } else {
        originalImages = [];
      }
    } catch (e) {
      error = `Failed to load original images: ${e}`;
    }
  }

  async function openImage(filename: string) {
    if (!property) return;

    try {
      const result = await DatabaseService.openImagesInFolder(property.folder_path, filename);

      if (!result.success) {
        error = result.error || 'Failed to open image';
      }
    } catch (e) {
      error = `Failed to open image: ${e}`;
      console.error('Error opening image:', e);
    }
  }

  async function openPropertyFolder() {
    if (!property) return;

    try {
      const result = await invoke('open_property_folder', {
        folderPath: property.folder_path
      });

      if (result.success) {
        showFolderMessage('Folder opened successfully!', 'success');
      } else {
        showFolderMessage(result.error || 'Failed to open folder', 'error');
      }
    } catch (error) {
      console.error('Failed to open property folder:', error);
      showFolderMessage('Failed to open folder', 'error');
    }
  }

  async function copyFolderPath() {
    if (!property) return;

    try {
      // Get the full absolute path
      const result = await invoke('get_full_property_path', {
        folderPath: property.folder_path
      });

      if (result.success && result.data?.full_path) {
        await navigator.clipboard.writeText(result.data.full_path);
        showFolderMessage('Path copied to clipboard!', 'success');
      } else {
        // Fallback: copy the relative path
        await navigator.clipboard.writeText(property.folder_path);
        showFolderMessage('Relative path copied to clipboard!', 'success');
      }
    } catch (error) {
      console.error('Failed to copy path:', error);
      try {
        // Fallback: try copying relative path
        await navigator.clipboard.writeText(property.folder_path);
        showFolderMessage('Relative path copied to clipboard!', 'success');
      } catch (fallbackError) {
        showFolderMessage('Failed to copy path to clipboard', 'error');
      }
    }
  }

  function showFolderMessage(message: string, type: 'success' | 'error') {
    folderMessage = message;
    folderMessageType = type;

    // Clear message after 3 seconds
    setTimeout(() => {
      folderMessage = '';
      folderMessageType = '';
    }, 3000);
  }
</script>

{#if loading}
  <div class="flex h-64 items-center justify-center">
    <div class="text-center">
      <div
        class="mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-blue-500 border-t-transparent"
      ></div>
      <p class="text-foreground-600">Loading property data...</p>
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
    <!-- Property Details -->
    <section class="bg-background-100 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-4 flex items-start justify-between">
        <div class="flex-1">
          <h1 class="text-foreground-900 mb-3 text-3xl font-bold">{property.name}</h1>
          <div class="mb-4 grid grid-cols-1 gap-4 md:grid-cols-2">
            <div class="flex items-center space-x-2">
              <span class="text-lg">📍</span>
              <span class="text-foreground-700 font-medium">City:</span>
              <span class="text-foreground-600">{property.city}</span>
            </div>
            <div class="flex items-center space-x-2">
              <span class="text-lg">{property.completed ? '✅' : '🔄'}</span>
              <span class="text-foreground-700 font-medium">Status:</span>
              <span
                class="rounded-full px-2 py-1 text-xs font-medium {property.completed
                  ? 'bg-green-100 text-green-700'
                  : 'bg-orange-100 text-orange-700'}"
              >
                {property.completed ? 'Completed' : 'In Progress'}
              </span>
            </div>
          </div>
        </div>

        <!-- Action Buttons -->
        <div class="flex flex-col space-y-2">
          <button
            onclick={openPropertyFolder}
            class="flex items-center space-x-2 rounded-lg border border-blue-200 bg-blue-100 px-4 py-2 text-sm font-medium text-blue-700 transition-all duration-200 hover:bg-blue-200"
            title="Open Property Folder"
          >
            <span>📁</span>
            <span>Open Folder</span>
          </button>

          <button
            onclick={copyFolderPath}
            class="flex items-center space-x-2 rounded-lg border border-gray-200 bg-gray-100 px-4 py-2 text-sm font-medium text-gray-700 transition-all duration-200 hover:bg-gray-200"
            title="Copy Folder Path"
          >
            <span>📋</span>
            <span>Copy Path</span>
          </button>
        </div>
      </div>

      <!-- Folder Path Display -->
      <div class="bg-background-50 border-background-200 mb-4 rounded-lg border p-4">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-foreground-500 mb-1 text-xs font-medium tracking-wide uppercase">
              Folder Path
            </p>
            <p class="text-foreground-700 font-mono text-sm break-all">{property.folder_path}</p>
          </div>
        </div>
      </div>

      <!-- Success/Error Message -->
      {#if folderMessage}
        <div
          class="mb-4 rounded-lg border p-3 {folderMessageType === 'success'
            ? 'border-green-200 bg-green-50 text-green-800'
            : 'border-red-200 bg-red-50 text-red-800'}"
        >
          <div class="flex items-center space-x-2">
            <span>{folderMessageType === 'success' ? '✅' : '❌'}</span>
            <span class="text-sm font-medium">{folderMessage}</span>
          </div>
        </div>
      {/if}

      <!-- Notes -->
      {#if property.notes}
        <div class="bg-background-50 border-background-200 mb-4 rounded-lg border p-4">
          <p class="text-foreground-500 mb-2 text-xs font-medium tracking-wide uppercase">Notes</p>
          <p class="text-foreground-700 whitespace-pre-wrap">{property.notes}</p>
        </div>
      {/if}

      <!-- Timestamps -->
      <div
        class="text-foreground-500 border-background-200 flex items-center space-x-6 border-t pt-4 text-xs"
      >
        <div class="flex items-center space-x-2">
          <span class="h-1.5 w-1.5 rounded-full bg-green-400"></span>
          <span>Created: {formatDate(property.created_at)}</span>
        </div>
        {#if property.updated_at !== property.created_at}
          <div class="flex items-center space-x-2">
            <span class="h-1.5 w-1.5 rounded-full bg-blue-400"></span>
            <span>Updated: {formatDate(property.updated_at)}</span>
          </div>
        {/if}
      </div>
    </section>

    <!-- Workflow Steps Navigation -->
    <section class="bg-background-100 border-background-200 rounded-xl border p-6 shadow-sm">
      <h2 class="text-foreground-900 mb-4 text-lg font-semibold">Workflow Steps</h2>
      <div class="grid grid-cols-1 gap-4 md:grid-cols-4">
        <a
          href="/properties/{property.id}/step1"
          class="flex items-center space-x-3 rounded-lg border border-blue-200 bg-blue-50 p-4 transition-colors hover:bg-blue-100"
        >
          <span class="text-2xl">📁</span>
          <div>
            <p class="font-medium text-blue-900">Step 1</p>
            <p class="text-sm text-blue-700">Copy to INTERNET</p>
          </div>
        </a>

        <a
          href="/properties/{property.id}/step2"
          class="flex items-center space-x-3 rounded-lg border border-purple-200 bg-purple-50 p-4 transition-colors hover:bg-purple-100"
        >
          <span class="text-2xl">🔢</span>
          <div>
            <p class="font-medium text-purple-900">Step 2</p>
            <p class="text-sm text-purple-700">Order & Rename</p>
          </div>
        </a>

        <a
          href="/properties/{property.id}/step3"
          class="flex items-center space-x-3 rounded-lg border border-indigo-200 bg-indigo-50 p-4 transition-colors hover:bg-indigo-100"
        >
          <span class="text-2xl">✏️</span>
          <div>
            <p class="font-medium text-indigo-900">Step 3</p>
            <p class="text-sm text-indigo-700">Copy to AGGELIA</p>
          </div>
        </a>

        <a
          href="/properties/{property.id}/step4"
          class="flex items-center space-x-3 rounded-lg border border-amber-200 bg-amber-50 p-4 transition-colors hover:bg-amber-100"
        >
          <span class="text-2xl">🏷️</span>
          <div>
            <p class="font-medium text-amber-900">Step 4</p>
            <p class="text-sm text-amber-700">Add Watermark</p>
          </div>
        </a>
      </div>
    </section>

    <!-- Original Images Gallery -->
    <section class="bg-background-100 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-6 flex items-center justify-between">
        <h2 class="text-foreground-900 text-xl font-semibold">
          Original Images ({originalImages.length})
        </h2>
        {#if originalImages.length > 0}
          <div class="text-foreground-500 text-sm">Click images to open in system viewer</div>
        {/if}
      </div>

      {#if originalImages.length === 0}
        <div class="py-16 text-center">
          <div
            class="bg-background-200 mx-auto mb-4 flex h-24 w-24 items-center justify-center rounded-full"
          >
            <span class="text-3xl">📷</span>
          </div>
          <h3 class="text-foreground-700 mb-2 text-lg font-medium">No original images found</h3>
          <p class="text-foreground-500 mb-6">
            Upload some images to get started with your workflow.
          </p>
        </div>
      {:else}
        <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
          {#each originalImages as image}
            <div class="group relative">
              <button
                class="bg-background-100 border-background-300 hover:border-background-400 aspect-square w-full overflow-hidden rounded-xl border transition-all duration-200 hover:shadow-lg"
                onclick={() => openImage(image.filename)}
              >
                {#if image.loading}
                  <!-- Loading state -->
                  <div class="bg-background-100 flex h-full w-full items-center justify-center">
                    <div class="text-center">
                      <div
                        class="mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-3 border-blue-500 border-t-transparent"
                      ></div>
                      <span class="text-foreground-500 text-xs">Loading...</span>
                    </div>
                  </div>
                {:else if image.dataUrl}
                  <!-- Actual image -->
                  <img
                    src={image.dataUrl}
                    alt={image.filename}
                    loading="lazy"
                    class="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
                  />
                {:else}
                  <!-- Error fallback -->
                  <div class="flex h-full w-full items-center justify-center bg-red-100">
                    <div class="text-center text-red-500">
                      <span class="mb-2 block text-2xl">❌</span>
                      <span class="text-xs">Failed to load</span>
                    </div>
                  </div>
                {/if}

                <!-- Filename overlay -->
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
    </section>
  </div>
{/if}
