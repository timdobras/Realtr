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
      
      internetImages = sortedFilenames.map(filename => ({
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
        
        aggeliaImages = sortedFilenames.map(filename => ({
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
    return ['jpg', 'jpeg'].includes(ext) ? 'image/jpeg'
         : ext === 'png' ? 'image/png'
         : ext === 'gif' ? 'image/gif'
         : ext === 'webp' ? 'image/webp'
         : ext === 'bmp' ? 'image/bmp'
         : 'image/jpeg';
  }

  // Select all available images
  function selectAllImages() {
    internetImages = internetImages.map(img => ({
      ...img,
      selected: img.inAggelia ? img.selected : true
    }));
  }

  // Deselect all images
  function deselectAllImages() {
    internetImages = internetImages.map(img => ({
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

    const selectedImages = internetImages.filter(img => img.selected);
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

      const filenames = selectedImages.map(img => img.filename);

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
      const result = await DatabaseService.openWithConfiguredEditor(propertyId || 0, filename, 'complex', 'aggelia');
      
      if (!result.success) {
        error = result.error || 'Failed to open image in advanced editor';
      }
    } catch (e) {
      error = `Failed to open image: ${e}`;
    }
  }

  async function clearAggeliaFolder() {
    if (!confirm('Are you sure you want to clear all images from the AGGELIA folder? This action cannot be undone.')) {
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
  $: selectedCount = internetImages.filter(img => img.selected).length;
  $: availableCount = internetImages.filter(img => !img.inAggelia).length;
  $: allAvailableSelected = availableCount > 0 && internetImages.filter(img => !img.inAggelia && img.selected).length === availableCount;
  $: anySelected = selectedCount > 0;
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
    <!-- Progress Section -->
    {#if copyingImages}
      <div class="bg-blue-50 rounded-lg p-4 border border-blue-200">
        <div class="flex items-center space-x-3">
          <div class="animate-spin w-5 h-5 border-2 border-blue-500 border-t-transparent rounded-full"></div>
          <div>
            <p class="font-medium text-blue-900">Copying images to AGGELIA folder...</p>
            <p class="text-sm text-blue-700">Please wait while images are being copied.</p>
          </div>
        </div>
      </div>
    {/if}

    <!-- Action Controls -->
    <div class="bg-background-100 rounded-xl shadow-sm p-6 border border-background-200">
      <h2 class="text-lg font-semibold mb-6 text-foreground-900">Selection Controls</h2>
      
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center space-x-6">
          <!-- Selection Buttons -->
          <div class="flex items-center space-x-3">
            <button
              onclick={selectAllImages}
              disabled={availableCount === 0 || allAvailableSelected}
              class="px-4 py-2 text-sm font-medium text-blue-700 bg-blue-100 border border-blue-300 rounded-lg hover:bg-blue-200 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              ✅ Select All ({availableCount})
            </button>
            
            <button
              onclick={deselectAllImages}
              disabled={!anySelected}
              class="px-4 py-2 text-sm font-medium text-gray-700 bg-gray-100 border border-gray-300 rounded-lg hover:bg-gray-200 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              ❌ Deselect All
            </button>
          </div>
          
          <div class="px-3 py-1 bg-blue-100 text-blue-800 rounded-full text-sm font-medium">
            {selectedCount} selected
          </div>
        </div>

        <div class="flex space-x-3">
          <button
            onclick={copySelectedToAggelia}
            disabled={copyingImages || selectedCount === 0}
            class="btn-primary disabled:opacity-50 disabled:cursor-not-allowed flex items-center space-x-2 px-4 py-2 bg-blue-700 cursor-pointer rounded-lg"
          >
            <span>📁→✨</span>
            <span>Copy to AGGELIA ({selectedCount})</span>
          </button>
          
          <button
            onclick={clearAggeliaFolder}
            disabled={copyingImages || aggeliaImages.length === 0}
            class="bg-red-600 hover:bg-red-700 text-white font-medium px-4 py-2 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center space-x-2"
          >
            <span>🗑️</span>
            <span>Clear AGGELIA</span>
          </button>
        </div>
      </div>

      <!-- Selection Stats -->
      <div class="grid grid-cols-3 gap-4 pt-4 border-t border-background-300">
        <div class="text-center">
          <div class="text-2xl font-bold text-foreground-900">{internetImages.length}</div>
          <div class="text-sm text-foreground-600">Total Images</div>
        </div>
        <div class="text-center">
          <div class="text-2xl font-bold text-green-600">{internetImages.filter(img => img.inAggelia).length}</div>
          <div class="text-sm text-foreground-600">In AGGELIA</div>
        </div>
        <div class="text-center">
          <div class="text-2xl font-bold text-blue-600">{availableCount}</div>
          <div class="text-sm text-foreground-600">Available</div>
        </div>
      </div>
    </div>


    <!-- INTERNET Images Section -->
    <section class="bg-background-100 rounded-xl shadow-sm border border-background-200">
      <div class="p-6">
        <div class="flex items-center justify-between mb-6">
          <h2 class="text-xl font-semibold text-foreground-900">
            INTERNET Folder Images ({internetImages.length})
          </h2>
          {#if internetImages.length > 0}
            <div class="text-sm text-foreground-500">
              Click images to select • Green badges indicate already copied
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
          <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6 gap-4">
            {#each internetImages as image, index}
              <div class="relative group">
                <button 
                  class="w-full aspect-square bg-white rounded-xl overflow-hidden border-2 transition-all duration-200 {
                    image.inAggelia ? 'border-green-400 shadow-green-100 shadow-lg' :
                    image.selected ? 'border-blue-400 shadow-blue-100 shadow-lg scale-105' : 
                    'border-background-300 hover:border-background-400 hover:shadow-md'
                  }" 
                  onclick={() => image.inAggelia ? openImageInAdvancedEditor(image.filename, false) : toggleImageSelection(index)}
                  disabled={copyingImages}
                >
                  {#if image.loading}
                    <div class="w-full h-full bg-background-100 flex items-center justify-center">
                      <div class="text-center">
                        <div class="animate-spin w-6 h-6 border-3 border-blue-500 border-t-transparent rounded-full mx-auto mb-2"></div>
                        <p class="text-xs text-foreground-500">Loading...</p>
                      </div>
                    </div>
                  {:else if image.dataUrl}
                    <img
                      src={image.dataUrl}
                      alt={image.filename}
                      loading="lazy"
                      class="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                    />
                  {:else}
                    <div class="w-full h-full bg-red-100 flex items-center justify-center">
                      <div class="text-center text-red-500">
                        <span class="text-2xl block mb-2">❌</span>
                        <p class="text-xs">Failed to load</p>
                      </div>
                    </div>
                  {/if}

                  <!-- Status Indicators -->
                  {#if image.inAggelia}
                    <div class="absolute top-2 left-2 bg-green-500 text-white text-xs px-2 py-1 rounded-full shadow-lg flex items-center space-x-1">
                      <span>✨</span>
                      <span class="hidden sm:inline">In AGGELIA</span>
                    </div>
                  {:else if image.selected}
                    <div class="absolute top-2 left-2 w-6 h-6 bg-blue-500 text-white rounded-full shadow-lg flex items-center justify-center">
                      <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
                        <path fillRule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clipRule="evenodd" />
                      </svg>
                    </div>
                  {:else}
                    <div class="absolute top-2 left-2 w-6 h-6 bg-white/80 border-2 border-background-300 rounded-full shadow-sm opacity-0 group-hover:opacity-100 transition-opacity"></div>
                  {/if}

                  <!-- Filename -->
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
      </div>
    </section>

    <!-- AGGELIA Images Section -->
    <section class="bg-background-100 rounded-xl shadow-sm border border-background-200">
      <div class="p-6">
        <div class="flex items-center justify-between mb-6">
          <h2 class="text-xl font-semibold text-foreground-900">
            AGGELIA Folder - Advanced Editing ({aggeliaImages.length})
          </h2>
          {#if aggeliaImages.length > 0}
            <div class="text-sm text-foreground-500">
              Click images to open in advanced editor
            </div>
          {/if}
        </div>
        
        {#if aggeliaImages.length === 0}
          <div class="text-center py-16">
            <div class="w-24 h-24 bg-purple-100 rounded-full flex items-center justify-center mx-auto mb-4">
              <span class="text-3xl">✨</span>
            </div>
            <h3 class="text-lg font-medium text-foreground-700 mb-2">No images in AGGELIA folder yet</h3>
            <p class="text-foreground-500 mb-6">Select images above and copy them to AGGELIA for advanced editing.</p>
          </div>
        {:else}
          <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6 gap-4">
            {#each aggeliaImages as image}
              <div class="relative group">
                <button 
                  class="w-full aspect-square bg-white rounded-xl overflow-hidden border-2 border-purple-400 shadow-purple-100 shadow-lg hover:shadow-xl hover:scale-105 transition-all duration-200" 
                  onclick={() => openImageInAdvancedEditor(image.filename, true)}
                  disabled={copyingImages}
                >
                  {#if image.loading}
                    <div class="w-full h-full bg-background-100 flex items-center justify-center">
                      <div class="text-center">
                        <div class="animate-spin w-6 h-6 border-3 border-purple-500 border-t-transparent rounded-full mx-auto mb-2"></div>
                        <p class="text-xs text-foreground-500">Loading...</p>
                      </div>
                    </div>
                  {:else if image.dataUrl}
                    <img
                      src={image.dataUrl}
                      alt={image.filename}
                      loading="lazy"
                      class="w-full h-full object-cover group-hover:scale-105 transition-transform duration-300"
                    />
                  {:else}
                    <div class="w-full h-full bg-red-100 flex items-center justify-center">
                      <div class="text-center text-red-500">
                        <span class="text-2xl block mb-2">❌</span>
                        <p class="text-xs">Failed to load</p>
                      </div>
                    </div>
                  {/if}

                  <!-- Advanced editing indicator -->
                  <div class="absolute top-2 right-2 bg-purple-500 text-white text-xs px-2 py-1 rounded-full shadow-lg flex items-center space-x-1">
                    <span>🎨</span>
                    <span class="hidden sm:inline">Advanced</span>
                  </div>

                  <!-- Filename -->
                  <div class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-purple-900/80 via-purple-800/60 to-transparent p-3 pt-6">
                    <p class="text-white text-xs font-medium truncate" title={image.filename}>
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
    <div class="bg-gradient-to-r from-green-50 to-emerald-50 rounded-xl p-6 border border-green-200">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="font-semibold text-green-900 mb-2">Ready for the final step?</h3>
          <p class="text-sm text-green-700">Once you've edited your images in AGGELIA, proceed to Step 4 for watermarking.</p>
        </div>
        <a 
          href="/properties/{property.id}/step4" 
          class="btn-primary {aggeliaImages.length === 0 ? 'opacity-50 cursor-not-allowed pointer-events-none' : ''}"
        >
          Step 4: Add Watermark →
        </a>
      </div>
    </div>  <!-- ✅ Correct closing tag -->
  </div>
{/if}

<style>

</style>
