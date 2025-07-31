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
          newName: baseFileName ? `${baseFileName}_${index + 1}` : `${index + 1}`
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

  function autoNumberAll() {
    internetImages = internetImages.map((image, index) => ({
      ...image,
      newName: baseFileName ? `${baseFileName}_${index + 1}` : `${index + 1}`
    }));
  }
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
              d="M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4"
            />
          </svg>
        </div>
        <div>
          <h1 class="text-foreground-900 text-2xl font-bold">Step 2: Order & Rename</h1>
          <p class="text-foreground-600">
            Arrange your images in the desired order and rename them
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
              <li>• Drag images to reorder them or use arrow buttons</li>
              <li>• Set individual names for each image</li>
              <li>• Use base filename to apply consistent naming</li>
              <li>• Apply renaming when satisfied with the order</li>
            </ul>
          </div>
        </div>
      </div>
    </div> -->

    <!-- Naming Controls -->
    <!-- <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-4 flex items-center space-x-3">
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
          <h2 class="text-foreground-900 text-lg font-semibold">Naming Options</h2>
          <p class="text-foreground-600 text-sm">Set up consistent naming for your images</p>
        </div>
      </div>

      <div class="grid grid-cols-1 gap-6 md:grid-cols-2">
        <div>
          <label class="text-foreground-700 mb-2 block text-sm font-medium">
            Base Filename (Optional)
          </label>
          <input
            type="text"
            bind:value={baseFileName}
            oninput={updateNewNamesFromBase}
            placeholder="e.g., property, apartment, house"
            class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full rounded-lg border px-4 py-3 transition-colors focus:ring-2 focus:outline-none"
            disabled={isDragging || renamingImages}
          />
          <p class="text-foreground-500 mt-2 text-xs">
            Images will be named: {baseFileName || 'image'}_1, {baseFileName || 'image'}_2, etc.
          </p>
        </div>

        <div class="flex items-end">
          <button
            onclick={autoNumberAll}
            disabled={internetImages.length === 0 || isDragging || renamingImages}
            class="bg-background-200 text-foreground-700 hover:bg-background-300 w-full rounded-lg px-4 py-3 font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            Auto-Number All Images
          </button>
        </div>
      </div>
    </div> -->

    <!-- Action Bar -->
    {#if internetImages.length > 0}
      <div class="bg-accent-50 border-accent-200 rounded-xl border p-6">
        <div class="flex items-center justify-between">
          <div class="flex items-center space-x-4">
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
              <p class="text-accent-900 font-semibold">Ready to apply changes?</p>
              <p class="text-accent-700 text-sm">
                Rename {internetImages.length} images with your new naming scheme
              </p>
            </div>
          </div>

          <button
            onclick={applyRenaming}
            disabled={renamingImages || internetImages.length === 0 || isDragging}
            class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-3 rounded-lg px-6 py-3 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            {#if renamingImages}
              <div
                class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
              ></div>
              <span>Renaming...</span>
            {:else}
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M9 12l2 2 4-4"
                />
              </svg>
              <span>Apply Renaming</span>
            {/if}
          </button>
        </div>
      </div>
    {/if}

    <!-- Images Grid -->
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
                Image Gallery ({internetImages.length})
              </h2>
              <p class="text-foreground-600 text-sm">
                {isDragging ? 'Dragging in progress...' : 'Drag images to reorder or use controls'}
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
                  d="M7 16V4m0 0L3 8m4-4l4 4"
                />
              </svg>
              <span>Drag & Drop Enabled</span>
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
              You need to copy images from Step 1 before you can order and rename them.
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
          <div
            class="grid grid-cols-2 gap-6 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5"
            use:dndzone={{
              items: internetImages,
              dragDisabled,
              flipDurationMs: isDragging ? 0 : 300,
              dropTargetStyle: {}
            }}
            onconsider={handleDndConsider}
            onfinalize={handleDndFinalize}
          >
            {#each internetImages as image, index (image.id)}
              <div
                class="bg-background-100 border-background-200 group overflow-hidden rounded-xl border transition-all duration-200 hover:shadow-lg {isDragging
                  ? 'opacity-75'
                  : 'hover:border-accent-200'}"
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
                          class="border-accent-500 mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-2 border-t-transparent"
                        ></div>
                        <p class="text-foreground-500 text-xs font-medium">Loading...</p>
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
                    {/if}
                  </button>

                  <!-- Order Badge -->
                  <div
                    class="bg-accent-500 absolute top-3 left-3 flex h-8 w-8 items-center justify-center rounded-full text-sm font-bold text-white shadow-lg"
                  >
                    {index + 1}
                  </div>

                  <!-- Edit Indicator on Hover -->
                  <div
                    class="absolute inset-0 flex items-center justify-center bg-black/20 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                  >
                    <div class="rounded-full bg-white/90 p-2">
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
                    </div>
                  </div>
                </div>

                <!-- Image Controls -->
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
                        class="border-background-300 bg-background-50 focus:ring-accent-500 focus:border-accent-500 flex-1 rounded-lg border px-3 py-2 text-sm transition-colors focus:ring-2 focus:outline-none"
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
                        class="border-background-300 bg-background-50 text-foreground-600 hover:bg-background-100 hover:text-foreground-900 flex h-8 w-8 items-center justify-center rounded-lg border transition-colors disabled:cursor-not-allowed disabled:opacity-30"
                        title="Move up"
                      >
                        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M7 14l5-5 5 5"
                          />
                        </svg>
                      </button>
                      <button
                        onclick={() => moveDown(index)}
                        disabled={index === internetImages.length - 1 || isDragging}
                        class="border-background-300 bg-background-50 text-foreground-600 hover:bg-background-100 hover:text-foreground-900 flex h-8 w-8 items-center justify-center rounded-lg border transition-colors disabled:cursor-not-allowed disabled:opacity-30"
                        title="Move down"
                      >
                        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M17 10l-5 5-5-5"
                          />
                        </svg>
                      </button>
                    </div>

                    <button
                      onclick={(e) => openImageInEditor(image.filename, e)}
                      class="text-accent-600 hover:text-accent-700 flex items-center space-x-1 text-xs font-medium transition-colors"
                      disabled={isDragging}
                    >
                      <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
                        />
                      </svg>
                      <span>Edit</span>
                    </button>
                  </div>
                </div>
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
            <h3 class="text-foreground-900 font-semibold">Ready for the next step?</h3>
            <p class="text-foreground-600 text-sm">
              Once you've ordered and renamed your images, proceed to Step 3 for copying to AGGELIA.
            </p>
          </div>
        </div>

        <a
          href="/properties/{property.id}/step3"
          class="inline-flex items-center space-x-2 rounded-lg px-6 py-3 font-medium transition-colors {internetImages.length ===
          0
            ? 'bg-background-200 text-foreground-500 cursor-not-allowed'
            : 'bg-accent-500 hover:bg-accent-600 text-white'}"
          class:pointer-events-none={internetImages.length === 0}
        >
          <span>Step 3: Copy to AGGELIA</span>
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
