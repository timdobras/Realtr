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
        originalImages = response.map(filename => ({
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
            const mimeType = ['jpg', 'jpeg'].includes(ext) ? 'image/jpeg'
                          : ext === 'png' ? 'image/png'
                          : ext === 'gif' ? 'image/gif'
                          : ext === 'webp' ? 'image/webp'
                          : ext === 'bmp' ? 'image/bmp'
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
      const result = await DatabaseService.openImagesInFolder(
        property.folder_path,
        filename
      );
      
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
  <div class="flex items-center justify-center h-64">
    <div class="text-center">
      <div class="animate-spin w-8 h-8 border-4 border-blue-500 border-t-transparent rounded-full mx-auto mb-4"></div>
      <p class="text-foreground-600">Loading property data...</p>
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
    <!-- Property Details -->
    <section class="bg-background-100 rounded-xl shadow-sm border border-background-200 p-6">
      <div class="flex items-start justify-between mb-4">
        <div class="flex-1">
          <h1 class="text-3xl font-bold text-foreground-900 mb-3">{property.name}</h1>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-4">
            <div class="flex items-center space-x-2">
              <span class="text-lg">📍</span>
              <span class="font-medium text-foreground-700">City:</span>
              <span class="text-foreground-600">{property.city}</span>
            </div>
            <div class="flex items-center space-x-2">
              <span class="text-lg">{property.completed ? '✅' : '🔄'}</span>
              <span class="font-medium text-foreground-700">Status:</span>
              <span class="px-2 py-1 text-xs font-medium rounded-full {property.completed 
                ? 'bg-green-100 text-green-700' 
                : 'bg-orange-100 text-orange-700'}">
                {property.completed ? 'Completed' : 'In Progress'}
              </span>
            </div>
          </div>
        </div>
        
        <!-- Action Buttons -->
        <div class="flex flex-col space-y-2">
          <button
            onclick={openPropertyFolder}
            class="flex items-center space-x-2 px-4 py-2 bg-blue-100 text-blue-700 rounded-lg hover:bg-blue-200 transition-all duration-200 text-sm font-medium border border-blue-200"
            title="Open Property Folder"
          >
            <span>📁</span>
            <span>Open Folder</span>
          </button>
          
          <button
            onclick={copyFolderPath}
            class="flex items-center space-x-2 px-4 py-2 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 transition-all duration-200 text-sm font-medium border border-gray-200"
            title="Copy Folder Path"
          >
            <span>📋</span>
            <span>Copy Path</span>
          </button>
        </div>
      </div>

      <!-- Folder Path Display -->
      <div class="bg-background-50 border border-background-200 rounded-lg p-4 mb-4">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-xs text-foreground-500 uppercase tracking-wide font-medium mb-1">Folder Path</p>
            <p class="text-sm text-foreground-700 font-mono break-all">{property.folder_path}</p>
          </div>
        </div>
      </div>

      <!-- Success/Error Message -->
      {#if folderMessage}
        <div class="mb-4 p-3 rounded-lg border {folderMessageType === 'success' 
          ? 'bg-green-50 border-green-200 text-green-800' 
          : 'bg-red-50 border-red-200 text-red-800'}">
          <div class="flex items-center space-x-2">
            <span>{folderMessageType === 'success' ? '✅' : '❌'}</span>
            <span class="text-sm font-medium">{folderMessage}</span>
          </div>
        </div>
      {/if}

      <!-- Notes -->
      {#if property.notes}
        <div class="bg-background-50 border border-background-200 rounded-lg p-4 mb-4">
          <p class="text-xs text-foreground-500 uppercase tracking-wide font-medium mb-2">Notes</p>
          <p class="text-foreground-700 whitespace-pre-wrap">{property.notes}</p>
        </div>
      {/if}

      <!-- Timestamps -->
      <div class="flex items-center space-x-6 text-xs text-foreground-500 pt-4 border-t border-background-200">
        <div class="flex items-center space-x-2">
          <span class="w-1.5 h-1.5 bg-green-400 rounded-full"></span>
          <span>Created: {formatDate(property.created_at)}</span>
        </div>
        {#if property.updated_at !== property.created_at}
          <div class="flex items-center space-x-2">
            <span class="w-1.5 h-1.5 bg-blue-400 rounded-full"></span>
            <span>Updated: {formatDate(property.updated_at)}</span>
          </div>
        {/if}
      </div>
    </section>

    <!-- Workflow Steps Navigation -->
    <section class="bg-background-100 rounded-xl shadow-sm border border-background-200 p-6">
      <h2 class="text-lg font-semibold text-foreground-900 mb-4">Workflow Steps</h2>
      <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
        <a 
          href="/properties/{property.id}/step1" 
          class="flex items-center space-x-3 p-4 bg-blue-50 border border-blue-200 rounded-lg hover:bg-blue-100 transition-colors"
        >
          <span class="text-2xl">📁</span>
          <div>
            <p class="font-medium text-blue-900">Step 1</p>
            <p class="text-sm text-blue-700">Copy to INTERNET</p>
          </div>
        </a>
        
        <a 
          href="/properties/{property.id}/step2" 
          class="flex items-center space-x-3 p-4 bg-purple-50 border border-purple-200 rounded-lg hover:bg-purple-100 transition-colors"
        >
          <span class="text-2xl">🔢</span>
          <div>
            <p class="font-medium text-purple-900">Step 2</p>
            <p class="text-sm text-purple-700">Order & Rename</p>
          </div>
        </a>
        
        <a 
          href="/properties/{property.id}/step3" 
          class="flex items-center space-x-3 p-4 bg-indigo-50 border border-indigo-200 rounded-lg hover:bg-indigo-100 transition-colors"
        >
          <span class="text-2xl">✏️</span>
          <div>
            <p class="font-medium text-indigo-900">Step 3</p>
            <p class="text-sm text-indigo-700">Copy to AGGELIA</p>
          </div>
        </a>
        
        <a 
          href="/properties/{property.id}/step4" 
          class="flex items-center space-x-3 p-4 bg-amber-50 border border-amber-200 rounded-lg hover:bg-amber-100 transition-colors"
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
    <section class="bg-background-100 rounded-xl shadow-sm border border-background-200 p-6">
      <div class="flex items-center justify-between mb-6">
        <h2 class="text-xl font-semibold text-foreground-900">
          Original Images ({originalImages.length})
        </h2>
        {#if originalImages.length > 0}
          <div class="text-sm text-foreground-500">
            Click images to open in system viewer
          </div>
        {/if}
      </div>

      {#if originalImages.length === 0}
        <div class="text-center py-16">
          <div class="w-24 h-24 bg-background-200 rounded-full flex items-center justify-center mx-auto mb-4">
            <span class="text-3xl">📷</span>
          </div>
          <h3 class="text-lg font-medium text-foreground-700 mb-2">No original images found</h3>
          <p class="text-foreground-500 mb-6">Upload some images to get started with your workflow.</p>
        </div>
      {:else}
        <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6 gap-4">
          {#each originalImages as image}
            <div class="relative group">
              <button 
                class="w-full aspect-square bg-background-100 rounded-xl overflow-hidden border border-background-300 hover:border-background-400 hover:shadow-lg transition-all duration-200" 
                onclick={() => openImage(image.filename)}
              >
                {#if image.loading}
                  <!-- Loading state -->
                  <div class="w-full h-full bg-background-100 flex items-center justify-center">
                    <div class="text-center">
                      <div class="animate-spin w-6 h-6 border-3 border-blue-500 border-t-transparent rounded-full mx-auto mb-2"></div>
                      <span class="text-xs text-foreground-500">Loading...</span>
                    </div>
                  </div>
                {:else if image.dataUrl}
                  <!-- Actual image -->
                  <img
                    src={image.dataUrl}
                    alt={image.filename}
                    loading="lazy"
                    class="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                  />
                {:else}
                  <!-- Error fallback -->
                  <div class="w-full h-full bg-red-100 flex items-center justify-center">
                    <div class="text-center text-red-500">
                      <span class="text-2xl block mb-2">❌</span>
                      <span class="text-xs">Failed to load</span>
                    </div>
                  </div>
                {/if}
                
                <!-- Filename overlay -->
                <div class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/80 via-black/60 to-transparent p-3 pt-6">
                  <p class="text-white text-xs font-medium truncate" title={image.filename}>
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
