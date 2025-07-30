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
    return ['jpg', 'jpeg'].includes(ext) ? 'image/jpeg'
         : ext === 'png' ? 'image/png'
         : ext === 'gif' ? 'image/gif'
         : ext === 'webp' ? 'image/webp'
         : ext === 'bmp' ? 'image/bmp'
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

    const confirmMessage = `Are you sure you want to rename ${internetImages.length} images? This action cannot be undone.\n\nExamples:\n${internetImages.slice(0, 3).map(img => `${img.filename} → ${img.newName}.${img.filename.split('.').pop()}`).join('\n')}`;
    
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
  <div class="flex items-center justify-center h-64">
    <div class="text-center">
      <div class="animate-spin w-8 h-8 border-4 border-blue-500 border-t-transparent rounded-full mx-auto mb-4"></div>
      <p class="text-foreground-600">Loading images...</p>
    </div>
  </div>
{:else if error}
  <div class="p-6">
    <div class="bg-red-50 border border-red-200 rounded-lg p-4">
      <div class="flex items-center space-x-2">
        <span class="text-red-600">❌</span>
        <p class="text-red-800 font-medium">{error}</p>
      </div>
    </div>
  </div>
{:else if property}
  <div class="space-y-6 p-6">
    <!-- Images Grid -->        
     <div class="flex flex-row gap-4 w-full bg-background-100 rounded-lg shadow-sm border border-background-200 ">
         
          <button
            onclick={applyRenaming}
            disabled={renamingImages || internetImages.length === 0 || isDragging}
            class="btn-primary w-full p-6 disabled:opacity-50 disabled:cursor-not-allowed hover:bg-background-200 cursor-pointer"
          >
            {#if renamingImages}
              <span class="flex items-center justify-center space-x-2">
                <div class="animate-spin w-4 h-4 border-2 border-white border-t-transparent rounded-full"></div>
                <span>Renaming...</span>
              </span>
            {:else}
              ✅ Apply Renaming ({internetImages.length})
            {/if}
          </button>
        </div>
    <section class="bg-background-100 rounded-xl shadow-sm border border-background-200">
      <div class="p-6">
        <div class="flex items-center justify-between mb-6">
          <h2 class="text-xl font-semibold text-foreground-900">
            INTERNET Folder Images ({internetImages.length})
          </h2>
          {#if internetImages.length > 0}
            <div class="text-sm text-foreground-500">
              {isDragging ? 'Dragging...' : 'Drag images to reorder them'}
            </div>
          {/if}
        </div>
        
        {#if internetImages.length === 0}
          <div class="text-center py-16">
            <div class="w-24 h-24 bg-background-200 rounded-full flex items-center justify-center mx-auto mb-4">
              <span class="text-3xl">📁</span>
            </div>
            <h3 class="text-lg font-medium text-foreground-700 mb-2">No images in INTERNET folder</h3>
            <p class="text-foreground-500 mb-6">Go back to Step 1 to copy images first.</p>
            <a href="/properties/{property.id}/step1" class="btn-primary">
              ← Back to Step 1
            </a>
          </div>
        {:else}
          <div 
            class="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4"
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
                class="bg-background-200 rounded-lg border-2 border-background-300 hover:border-blue-300 transition-all duration-200 overflow-hidden group hover:shadow-lg {isDragging ? 'pointer-events-none' : ''}"
                animate:flip={{ duration: isDragging ? 0 : 300 }}
              >
                <!-- Image Preview -->
                <div class="relative">
                  <button 
                    class="w-full h-48 bg-background-100 flex items-center justify-center group-hover:bg-background-50 transition-colors"
                    onclick={(e) => openImageInEditor(image.filename, e)}
                    disabled={isDragging}
                  >
                    {#if image.loading}
                      <div class="text-center">
                        <div class="animate-spin w-6 h-6 border-3 border-blue-500 border-t-transparent rounded-full mx-auto mb-2"></div>
                        <p class="text-xs text-foreground-500">Loading...</p>
                      </div>
                    {:else if image.dataUrl}
                      <img
                        src={image.dataUrl}
                        alt={image.filename}
                        loading="lazy"
                        class="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                      />
                    {:else}
                      <div class="text-center text-red-500">
                        <span class="text-2xl block mb-2">❌</span>
                        <p class="text-xs">Failed to load</p>
                      </div>
                    {/if}
                  </button>

                  <!-- Order Badge -->
                  <div class="absolute top-3 left-3 w-8 h-8 bg-blue-500 text-white rounded-full flex items-center justify-center font-bold text-sm shadow-lg">
                    {index + 1}
                  </div>

                </div>

                <!-- Image Info -->
                <div class="p-4">
                  <p class="text-sm font-medium text-foreground-800 truncate mb-3" title={image.filename}>
                    {image.filename}
                  </p>
                  
                  <!-- New Name Input -->
                  <div class="space-y-2">
                    <label class="text-xs text-foreground-600 font-medium">New name:</label>
                    <div class="flex items-center space-x-2">
                      <input
                        type="text"
                        bind:value={image.newName}
                        class="flex-1 text-sm px-3 py-2 border border-background-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                        placeholder="Enter name"
                        disabled={isDragging}
                      />
                      <span class="text-xs text-foreground-500 font-mono">
                        .{image.filename.split('.').pop()}
                      </span>
                    </div>
                  </div>

                  <!-- Move Controls -->
                  <div class="flex items-center justify-between mt-4">
                    <div class="flex space-x-1">
                      <button
                        onclick={() => moveUp(index)}
                        disabled={index === 0 || isDragging}
                        class="w-8 h-8 text-foreground-400 hover:text-foreground-600 disabled:opacity-30 disabled:cursor-not-allowed border border-background-300 rounded-lg hover:bg-background-50 transition-colors flex items-center justify-center"
                        title="Move up"
                      >
                        ↑
                      </button>
                      <button
                        onclick={() => moveDown(index)}
                        disabled={index === internetImages.length - 1 || isDragging}
                        class="w-8 h-8 text-foreground-400 hover:text-foreground-600 disabled:opacity-30 disabled:cursor-not-allowed border border-background-300 rounded-lg hover:bg-background-50 transition-colors flex items-center justify-center"
                        title="Move down"
                      >
                        ↓
                      </button>
                    </div>
                    
                    <button
                      onclick={(e) => openImageInEditor(image.filename, e)}
                      class="text-xs text-blue-600 hover:text-blue-700 font-medium transition-colors"
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

        <div class="bg-background-100 rounded-lg shadow p-6">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="font-semibold text-foreground-900">Ready for the next step?</h3>
          <p class="text-sm text-foreground-600 mt-1">Once you've ordered and renamed your images, proceed to Step 3 for copying to AGGELIA.</p>
        </div>
        <a 
          href="/properties/{property.id}/step3" 
          class="btn-primary {internetImages.length === 0 ? 'opacity-50 cursor-not-allowed' : ''}"
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
