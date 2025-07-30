<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';
  import { dndzone } from 'svelte-dnd-action';
  import { flip } from 'svelte/animate';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
  export const prerender = false;

  interface ImageItem {
    id: string; // Use filename as stable ID
    filename: string;
    dataUrl: string;
    loading: boolean;
    newName?: string;
  }

  let propertyId: number | null = null;
  let property: Property | null = null;
  let internetImages: ImageItem[] = [];
  let error = '';
  let loading = true;
  let renamingImages = false;
  let baseFileName = '';
  let dragDisabled = false;
  let isDragging = false; // Track drag state

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

      await loadInternetImages();
    } catch (e) {
      error = `Failed to load property: ${e}`;
    } finally {
      loading = false;
    }
  });

  async function loadInternetImages() {
    if (!property) return;

    try {
      const response = await invoke('list_internet_images', {
        folderPath: property.folder_path
      });

      if (Array.isArray(response)) {
        // Sort filenames numerically before processing
        const sortedFilenames = response.sort((a, b) => {
          // Extract numeric part from filename
          const getNumericValue = (filename) => {
            const match = filename.match(/^(\d+)/);
            return match ? parseInt(match[1]) : Infinity;
          };

          const numA = getNumericValue(a);
          const numB = getNumericValue(b);

          return numA - numB;
        });

        internetImages = sortedFilenames.map((filename, index) => ({
          id: filename, // Use filename as stable ID
          filename,
          dataUrl: '',
          loading: true,
          newName: `${index + 1}`
        }));

        // Load thumbnails
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
      error = `Failed to load images: ${e}`;
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

  // Improved drag and drop handlers
  function handleDndConsider(e) {
    isDragging = true;
    dragDisabled = false;
    internetImages = e.detail.items;
    // Don't update names during dragging to reduce re-renders
  }

  function handleDndFinalize(e) {
    isDragging = false;
    dragDisabled = false;
    internetImages = e.detail.items;
    // Update names only after drag is complete
    updateNewNames();
  }

  // Debounced name update function
  let updateTimeout;
  function updateNewNames() {
    clearTimeout(updateTimeout);
    updateTimeout = setTimeout(() => {
      internetImages = internetImages.map((image, index) => ({
        ...image,
        newName: baseFileName ? `${baseFileName}_${index + 1}` : `${index + 1}`
      }));
    }, 50); // Small delay to batch updates
  }

  function updateNewNamesFromBase() {
    if (!isDragging) {
      updateNewNames();
    }
  }

  // Move functions for buttons
  function moveUp(index: number) {
    if (index > 0 && !isDragging) {
      const newImages = [...internetImages];
      [newImages[index - 1], newImages[index]] = [newImages[index], newImages[index - 1]];
      internetImages = newImages;
      updateNewNames();
    }
  }

  function moveDown(index: number) {
    if (index < internetImages.length - 1 && !isDragging) {
      const newImages = [...internetImages];
      [newImages[index], newImages[index + 1]] = [newImages[index + 1], newImages[index]];
      internetImages = newImages;
      updateNewNames();
    }
  }

  async function applyRenaming() {
    if (!property || internetImages.length === 0) return;

    const confirmMessage = `Are you sure you want to rename ${internetImages.length} images? This action cannot be undone.\n\nExamples:\n${internetImages
      .slice(0, 3)
      .map((img) => `${img.filename} → ${img.newName}.${img.filename.split('.').pop()}`)
      .join('\n')}`;

    if (!confirm(confirmMessage)) {
      return;
    }

    try {
      renamingImages = true;
      dragDisabled = true; // Disable drag during rename

      const renameMap = internetImages.map((image, index) => {
        const extension = image.filename.split('.').pop() || 'jpg';
        return {
          old_name: image.filename,
          new_name: `${image.newName}.${extension}`
        };
      });

      const result = await invoke('rename_internet_images', {
        folderPath: property.folder_path,
        renameMap
      });

      if (result.success) {
        await loadInternetImages();
      } else {
        error = result.error || 'Failed to rename images';
      }
    } catch (e) {
      error = `Failed to rename images: ${e}`;
    } finally {
      renamingImages = false;
      dragDisabled = false;
    }
  }

  async function openImageInEditor(filename: string, event) {
    // Prevent opening during drag
    if (isDragging) {
      event.preventDefault();
      return;
    }

    if (!property) return;

    try {
      const result = await invoke('open_image_in_editor', {
        folderPath: property.folder_path,
        filename,
        isFromInternet: true
      });

      if (!result.success) {
        error = result.error || 'Failed to open image in editor';
      }
    } catch (e) {
      error = `Failed to open image: ${e}`;
    }
  }
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
    <!-- Images Grid -->
    <div
      class="bg-background-100 border-background-200 flex w-full flex-row gap-4 rounded-lg border shadow-sm"
    >
      <button
        onclick={applyRenaming}
        disabled={renamingImages || internetImages.length === 0 || isDragging}
        class="btn-primary hover:bg-background-200 w-full cursor-pointer p-6 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {#if renamingImages}
          <span class="flex items-center justify-center space-x-2">
            <div
              class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
            ></div>
            <span>Renaming...</span>
          </span>
        {:else}
          ✅ Apply Renaming ({internetImages.length})
        {/if}
      </button>
    </div>
    <section class="bg-background-100 border-background-200 rounded-xl border shadow-sm">
      <div class="p-6">
        <div class="mb-6 flex items-center justify-between">
          <h2 class="text-foreground-900 text-xl font-semibold">
            INTERNET Folder Images ({internetImages.length})
          </h2>
          {#if internetImages.length > 0}
            <div class="text-foreground-500 text-sm">
              {isDragging ? 'Dragging...' : 'Drag images to reorder them'}
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
          <div
            class="grid grid-cols-2 gap-4 md:grid-cols-4 lg:grid-cols-6"
            use:dndzone={{
              items: internetImages,
              dragDisabled,
              flipDurationMs: isDragging ? 0 : 300, // Disable flip during drag
              dropTargetStyle: {}
            }}
            onconsider={handleDndConsider}
            onfinalize={handleDndFinalize}
          >
            {#each internetImages as image, index (image.id)}
              <div
                class="bg-background-200 border-background-300 group overflow-hidden rounded-lg border-2 transition-all duration-200 hover:border-blue-300 hover:shadow-lg {isDragging
                  ? 'pointer-events-none'
                  : ''}"
                animate:flip={{ duration: isDragging ? 0 : 300 }}
              >
                <!-- Image Preview -->
                <div class="relative">
                  <button
                    class="bg-background-100 group-hover:bg-background-50 flex h-48 w-full items-center justify-center transition-colors"
                    onclick={(e) => openImageInEditor(image.filename, e)}
                    disabled={isDragging}
                  >
                    {#if image.loading}
                      <div class="text-center">
                        <div
                          class="mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-3 border-blue-500 border-t-transparent"
                        ></div>
                        <p class="text-foreground-500 text-xs">Loading...</p>
                      </div>
                    {:else if image.dataUrl}
                      <img
                        src={image.dataUrl}
                        alt={image.filename}
                        loading="lazy"
                        class="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
                      />
                    {:else}
                      <div class="text-center text-red-500">
                        <span class="mb-2 block text-2xl">❌</span>
                        <p class="text-xs">Failed to load</p>
                      </div>
                    {/if}
                  </button>

                  <!-- Order Badge -->
                  <div
                    class="absolute top-3 left-3 flex h-8 w-8 items-center justify-center rounded-full bg-blue-500 text-sm font-bold text-white shadow-lg"
                  >
                    {index + 1}
                  </div>
                </div>

                <!-- Image Info -->
                <div class="p-4">
                  <p
                    class="text-foreground-800 mb-3 truncate text-sm font-medium"
                    title={image.filename}
                  >
                    {image.filename}
                  </p>

                  <!-- New Name Input -->
                  <div class="space-y-2">
                    <label class="text-foreground-600 text-xs font-medium">New name:</label>
                    <div class="flex items-center space-x-2">
                      <input
                        type="text"
                        bind:value={image.newName}
                        class="border-background-300 flex-1 rounded-lg border px-3 py-2 text-sm focus:border-blue-500 focus:ring-2 focus:ring-blue-500"
                        placeholder="Enter name"
                        disabled={isDragging}
                      />
                      <span class="text-foreground-500 font-mono text-xs">
                        .{image.filename.split('.').pop()}
                      </span>
                    </div>
                  </div>

                  <!-- Move Controls -->
                  <div class="mt-4 flex items-center justify-between">
                    <div class="flex space-x-1">
                      <button
                        onclick={() => moveUp(index)}
                        disabled={index === 0 || isDragging}
                        class="text-foreground-400 hover:text-foreground-600 border-background-300 hover:bg-background-50 flex h-8 w-8 items-center justify-center rounded-lg border transition-colors disabled:cursor-not-allowed disabled:opacity-30"
                        title="Move up"
                      >
                        ↑
                      </button>
                      <button
                        onclick={() => moveDown(index)}
                        disabled={index === internetImages.length - 1 || isDragging}
                        class="text-foreground-400 hover:text-foreground-600 border-background-300 hover:bg-background-50 flex h-8 w-8 items-center justify-center rounded-lg border transition-colors disabled:cursor-not-allowed disabled:opacity-30"
                        title="Move down"
                      >
                        ↓
                      </button>
                    </div>

                    <button
                      onclick={(e) => openImageInEditor(image.filename, e)}
                      class="text-xs font-medium text-blue-600 transition-colors hover:text-blue-700"
                      disabled={isDragging}
                    >
                      Open in Editor
                    </button>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    <!-- Next Step -->

    <div class="bg-background-100 rounded-lg p-6 shadow">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-foreground-900 font-semibold">Ready for the next step?</h3>
          <p class="text-foreground-600 mt-1 text-sm">
            Once you've ordered and renamed your images, proceed to Step 3 for copying to AGGELIA.
          </p>
        </div>
        <a
          href="/properties/{property.id}/step3"
          class="btn-primary {internetImages.length === 0 ? 'cursor-not-allowed opacity-50' : ''}"
          class:disabled={internetImages.length === 0}
        >
          Step 3: Copy to AGGELIA →
        </a>
      </div>
    </div>
  </div>
{/if}

<style>
</style>
