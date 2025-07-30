<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';

  interface ImageItem {
    filename: string;
    dataUrl: string;
    loading: boolean;
  }

  let propertyId: number | null = null;
  let property: Property | null = null;
  let watermarkImages: ImageItem[] = [];
  let watermarkAggeliaImages: ImageItem[] = [];
  let error = '';
  let loading = true;
  let processingWatermarksVar = false;
  let watermarkConfig: { imagePath?: string; opacity: number } | null = null;
  let processingProgress = 0;
  let processingStatus = '';

  // Get the id from the URL params
  $: propertyId = Number($page.params.id);

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

      await loadWatermarkConfig();
      await loadWatermarkImages();
    } catch (e) {
      error = `Failed to load property: ${e}`;
    } finally {
      loading = false;
    }
  });

  async function loadWatermarkConfig() {
    try {
      const config = await invoke('load_config');
      if (config) {
        watermarkConfig = {
          imagePath: config.watermark_image_path,
          opacity: config.watermark_opacity || 0.15
        };
      }
    } catch (e) {
      console.error('Failed to load watermark config:', e);
    }
  }

  async function loadWatermarkImages() {
    if (!property) return;

    try {
      // Load WATERMARK images
      const watermarkList = await invoke('list_watermark_images', { 
        folderPath: property.folder_path
      });
      
      if (Array.isArray(watermarkList)) {
        const sortedWatermarkList = sortImagesByNumericFilename(watermarkList);
        watermarkImages = sortedWatermarkList.map(filename => ({
          filename,
          dataUrl: '',
          loading: true
        }));

        // Load thumbnails
        for (let i = 0; i < watermarkImages.length; i++) {
          const image = watermarkImages[i];
          try {
            const base64Data = await invoke('get_watermark_image_as_base64', {
              folderPath: property.folder_path,
              filename: image.filename,
              fromAggelia: false
            });

            const ext = image.filename.split('.').pop()?.toLowerCase() || '';
            const mimeType = getMimeType(ext);

            watermarkImages[i] = {
              ...image,
              dataUrl: `data:${mimeType};base64,${base64Data}`,
              loading: false
            };
          } catch (e) {
            watermarkImages[i] = { ...image, dataUrl: '', loading: false };
          }
        }
      }

      // Load WATERMARK/AGGELIA images
      const aggeliaList = await invoke('list_watermark_aggelia_images', { 
        folderPath: property.folder_path
      });
      
      if (Array.isArray(aggeliaList)) {
        const sortedAggeliaList = sortImagesByNumericFilename(aggeliaList);
        watermarkAggeliaImages = sortedAggeliaList.map(filename => ({
          filename,
          dataUrl: '',
          loading: true
        }));

        // Load thumbnails
        for (let i = 0; i < watermarkAggeliaImages.length; i++) {
          const image = watermarkAggeliaImages[i];
          try {
            const base64Data = await invoke('get_watermark_image_as_base64', {
              folderPath: property.folder_path,
              filename: image.filename,
              fromAggelia: true
            });

            const ext = image.filename.split('.').pop()?.toLowerCase() || '';
            const mimeType = getMimeType(ext);

            watermarkAggeliaImages[i] = {
              ...image,
              dataUrl: `data:${mimeType};base64,${base64Data}`,
              loading: false
            };
          } catch (e) {
            watermarkAggeliaImages[i] = { ...image, dataUrl: '', loading: false };
          }
        }
      }
    } catch (e) {
      watermarkImages = [];
      watermarkAggeliaImages = [];
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

  async function applyWatermarksToAllImages() {
    if (!property || !watermarkConfig?.imagePath) {
      error = 'Watermark not configured. Please set up watermark in settings first.';
      return;
    }

    const confirmMessage = 'This will copy images from INTERNET folders to WATERMARK folders and apply watermark to them. This may take a while. Continue?';
    
    if (!confirm(confirmMessage)) {
      return;
    }

    try {
      processingWatermarksVar = true;
      processingProgress = 0;
      processingStatus = 'Initializing watermark process...';
      error = '';

      // Simulate progress updates (in real implementation, this would come from backend)
      const progressInterval = setInterval(() => {
        if (processingProgress < 90) {
          processingProgress += Math.random() * 10;
          if (processingProgress < 30) {
            processingStatus = 'Copying images to WATERMARK folders...';
          } else if (processingProgress < 60) {
            processingStatus = 'Applying watermarks to images...';
          } else if (processingProgress < 90) {
            processingStatus = 'Finalizing watermarked images...';
          }
        }
      }, 500);

      const result = await invoke('copy_and_watermark_images', {
        folderPath: property.folder_path
      });

      clearInterval(progressInterval);
      processingProgress = 100;
      processingStatus = 'Watermark application completed!';

      if (result.success) {
        await loadWatermarkImages();
        setTimeout(() => {
          processingStatus = '';
        }, 2000);
      } else {
        error = result.error || 'Failed to apply watermarks';
        processingStatus = 'Error occurred during processing';
      }
    } catch (e) {
      error = `Failed to apply watermarks: ${e}`;
      processingStatus = 'Error occurred during processing';
    } finally {
      processingWatermarksVar = false;
      if (!error) {
        setTimeout(() => {
          processingProgress = 0;
          processingStatus = '';
        }, 1000);
      }
    }
  }

  async function clearWatermarkFolders() {
    if (!confirm('Are you sure you want to clear all watermarked images? This action cannot be undone.')) {
      return;
    }

    try {
      const result = await invoke('clear_watermark_folders', {
        folderPath: property.folder_path
      });

      if (result.success) {
        await loadWatermarkImages();
      } else {
        error = result.error || 'Failed to clear watermark folders';
      }
    } catch (e) {
      error = `Failed to clear folders: ${e}`;
    }
  }

  async function openWatermarkedImage(filename: string, fromAggelia: boolean = false) {
    if (!property) return;

    try {
      const result = await invoke('open_images_in_folder', {
        folderPath: fromAggelia 
          ? `${property.folder_path}/WATERMARK/AGGELIA`
          : `${property.folder_path}/WATERMARK`,
        selectedImage: filename
      });
      
      if (!result.success) {
        error = result.error || 'Failed to open image';
      }
    } catch (e) {
      error = `Failed to open image: ${e}`;
    }
  }

  // Reactive values for UI
  $: totalWatermarkedImages = watermarkImages.length + watermarkAggeliaImages.length;
  $: workflowProgress = 100; // Step 4 is 100% of workflow
</script>

{#if loading}
  <div class="flex items-center justify-center h-64">
    <div class="text-center">
      <div class="animate-spin w-8 h-8 border-4 border-blue-500 border-t-transparent rounded-full mx-auto mb-4"></div>
      <p class="text-foreground-600">Loading watermark data...</p>
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

    <!-- Processing Progress -->
    {#if processingWatermarksVar || processingProgress > 0}
      <div class="bg-blue-50 rounded-xl p-6 border border-blue-200">
        <div class="flex items-center space-x-4 mb-4">
          <div class="animate-spin w-6 h-6 border-3 border-blue-500 border-t-transparent rounded-full"></div>
          <div class="flex-1">
            <p class="font-medium text-blue-900">{processingStatus || 'Processing watermarks...'}</p>
            <p class="text-sm text-blue-700">Please wait while we apply watermarks to your images.</p>
          </div>
        </div>
        <div class="w-full bg-blue-200 rounded-full h-4">
          <div 
            class="bg-blue-600 h-4 rounded-full transition-all duration-300 ease-out"
            style="width: {processingProgress}%"
          ></div>
        </div>
        <div class="flex justify-between mt-2 text-xs text-blue-600">
          <span>Processing...</span>
          <span>{Math.round(processingProgress)}%</span>
        </div>
      </div>
    {/if}

    <!-- Statistics Dashboard -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <div class="bg-background-100 rounded-xl shadow-sm border border-background-200 p-6">
        <div class="flex items-center space-x-3">
          <div class="w-12 h-12 bg-amber-100 rounded-lg flex items-center justify-center">
            <span class="text-2xl">🏷️</span>
          </div>
          <div>
            <p class="text-2xl font-bold text-foreground-900">{watermarkImages.length}</p>
            <p class="text-sm text-foreground-600">Watermarked Images</p>
          </div>
        </div>
      </div>

      <div class="bg-background-100 rounded-xl shadow-sm border border-background-200 p-6">
        <div class="flex items-center space-x-3">
          <div class="w-12 h-12 bg-purple-100 rounded-lg flex items-center justify-center">
            <span class="text-2xl">🎨</span>
          </div>
          <div>
            <p class="text-2xl font-bold text-foreground-900">{watermarkAggeliaImages.length}</p>
            <p class="text-sm text-foreground-600">Advanced + Watermark</p>
          </div>
        </div>
      </div>

      <div class="bg-background-100 rounded-xl shadow-sm border border-background-200 p-6">
        <div class="flex items-center space-x-3">
          <div class="w-12 h-12 bg-green-100 rounded-lg flex items-center justify-center">
            <span class="text-2xl">📊</span>
          </div>
          <div>
            <p class="text-2xl font-bold text-foreground-900">{totalWatermarkedImages}</p>
            <p class="text-sm text-foreground-600">Total Ready</p>
          </div>
        </div>
      </div>

    </div>

    <!-- Watermark Configuration Display -->
    <div class="bg-background-100 rounded-xl shadow-sm p-6 border border-background-200">
      <h2 class="text-lg font-semibold mb-6 text-foreground-900">Watermark Configuration</h2>
      {#if watermarkConfig?.imagePath}
        <div class="flex items-center space-x-6">
          <div class="w-20 h-20 bg-background-100 border-2 border-background-300 rounded-xl flex items-center justify-center overflow-hidden shadow-inner">
            <img 
              src={`file://${watermarkConfig.imagePath}`} 
              alt="Watermark preview"
              class="max-w-full max-h-full object-contain"
              style="opacity: {watermarkConfig.opacity}"
              onerror={() => {}}
            />
          </div>
          <div class="flex-1">
            <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div class="bg-background-50 rounded-lg p-4">
                <p class="text-xs text-foreground-500 uppercase tracking-wide font-medium">Status</p>
                <p class="text-sm font-semibold text-green-600">✅ Configured</p>
              </div>
              <div class="bg-background-50 rounded-lg p-4">
                <p class="text-xs text-foreground-500 uppercase tracking-wide font-medium">Opacity</p>
                <p class="text-sm font-semibold text-foreground-900">{Math.round(watermarkConfig.opacity * 100)}%</p>
              </div>
              <div class="bg-background-50 rounded-lg p-4">
                <p class="text-xs text-foreground-500 uppercase tracking-wide font-medium">Position</p>
                <p class="text-sm font-semibold text-foreground-900">Center</p>
              </div>
            </div>
          </div>
        </div>
      {:else}
        <div class="bg-yellow-50 border border-yellow-200 rounded-xl p-6">
          <div class="flex items-center space-x-3">
            <div class="w-12 h-12 bg-yellow-100 rounded-lg flex items-center justify-center">
              <span class="text-2xl">⚠️</span>
            </div>
            <div class="flex-1">
              <h3 class="font-medium text-yellow-900 mb-1">Watermark Not Configured</h3>
              <p class="text-sm text-yellow-800 mb-3">
                Please configure your watermark image and opacity before applying watermarks.
              </p>
              <a href="/settings" class="inline-flex items-center space-x-2 bg-yellow-600 hover:bg-yellow-700 text-white px-4 py-2 rounded-lg transition-colors text-sm font-medium">
                <span>⚙️</span>
                <span>Go to Settings</span>
              </a>
            </div>
          </div>
        </div>
      {/if}
    </div>

    <!-- Action Controls -->
    <div class="bg-background-100 rounded-xl shadow-sm p-6 border border-background-200">
      <h2 class="text-lg font-semibold mb-6 text-foreground-900">Watermark Actions</h2>
      
      <div class="flex flex-col lg:flex-row lg:items-center lg:justify-between space-y-4 lg:space-y-0 lg:space-x-6">
        <div class="flex-1">
          <p class="text-foreground-700 mb-2">
            Apply watermarks to all images from INTERNET and INTERNET/AGGELIA folders.
          </p>
          <p class="text-sm text-foreground-500">
            This will create watermarked copies in WATERMARK and WATERMARK/AGGELIA folders.
          </p>
        </div>

        <div class="flex flex-col sm:flex-row space-y-3 sm:space-y-0 sm:space-x-4">
          <button
            onclick={applyWatermarksToAllImages}
            disabled={processingWatermarksVar || !watermarkConfig?.imagePath}
            class="btn-primary disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center space-x-2 px-6 py-3 bg-blue-700 cursor-pointer rounded-lg "
          >
            <span class="text-lg">🏷️</span>
            <span>Apply Watermarks</span>
            {#if processingWatermarksVar}
              <div class="animate-spin w-4 h-4 border-2 border-white border-t-transparent rounded-full ml-2"></div>
            {/if}
          </button>
          
          <button
            onclick={clearWatermarkFolders}
            disabled={processingWatermarksVar || totalWatermarkedImages === 0}
            class="bg-red-600 hover:bg-red-700 text-white font-medium px-6 py-3 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center space-x-2"
          >
            <span>🗑️</span>
            <span>Clear All</span>
          </button>
        </div>
      </div>
    </div>


    <!-- WATERMARK Images Section -->
    <section class="bg-background-100 rounded-xl shadow-sm border border-background-200">
      <div class="p-6">
        <div class="flex items-center justify-between mb-6">
          <h2 class="text-xl font-semibold text-foreground-900">
            WATERMARK Folder ({watermarkImages.length})
          </h2>
          {#if watermarkImages.length > 0}
            <div class="text-sm text-foreground-500">
              Click images to view • Publication ready
            </div>
          {/if}
        </div>
        
        {#if watermarkImages.length === 0}
          <div class="text-center py-16">
            <div class="w-24 h-24 bg-amber-100 rounded-full flex items-center justify-center mx-auto mb-4">
              <span class="text-3xl">🏷️</span>
            </div>
            <h3 class="text-lg font-medium text-foreground-700 mb-2">No watermarked images yet</h3>
            <p class="text-foreground-500 mb-6">Click "Apply Watermarks" to create watermarked copies of your images.</p>
          </div>
        {:else}
          <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6 gap-4">
            {#each watermarkImages as image}
              <div class="relative group">
                <button 
                  class="w-full aspect-square bg-background-100 rounded-xl overflow-hidden border-2 border-amber-300 shadow-amber-100 shadow-lg hover:shadow-xl hover:scale-105 transition-all duration-200" 
                  onclick={() => openWatermarkedImage(image.filename, false)}
                  disabled={processingWatermarksVar}
                >
                  {#if image.loading}
                    <div class="w-full h-full bg-background-100 flex items-center justify-center">
                      <div class="text-center">
                        <div class="animate-spin w-6 h-6 border-3 border-amber-500 border-t-transparent rounded-full mx-auto mb-2"></div>
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

                  <!-- Watermark indicator -->
                  <div class="absolute top-2 right-2 bg-amber-500 text-white text-xs px-2 py-1 rounded-full shadow-lg flex items-center space-x-1">
                    <span>🏷️</span>
                    <span class="hidden sm:inline">Watermarked</span>
                  </div>

                  <!-- Filename -->
                  <div class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-amber-900/80 via-amber-800/60 to-transparent p-3 pt-6">
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

    <!-- WATERMARK/AGGELIA Images Section -->
    <section class="bg-background-100 rounded-xl shadow-sm border border-background-200">
      <div class="p-6">
        <div class="flex items-center justify-between mb-6">
          <h2 class="text-xl font-semibold text-foreground-900">
            WATERMARK/AGGELIA Folder ({watermarkAggeliaImages.length})
          </h2>
          {#if watermarkAggeliaImages.length > 0}
            <div class="text-sm text-foreground-500">
              Advanced edited + watermarked images
            </div>
          {/if}
        </div>
        
        {#if watermarkAggeliaImages.length === 0}
          <div class="text-center py-16">
            <div class="w-24 h-24 bg-purple-100 rounded-full flex items-center justify-center mx-auto mb-4">
              <span class="text-3xl">🎨</span>
            </div>
            <h3 class="text-lg font-medium text-foreground-700 mb-2">No advanced watermarked images yet</h3>
            <p class="text-foreground-500 mb-6">Advanced edited images with watermarks will appear here.</p>
          </div>
        {:else}
          <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6 gap-4">
            {#each watermarkAggeliaImages as image}
              <div class="relative group">
                <button 
                  class="w-full aspect-square bg-background-100 rounded-xl overflow-hidden border-2 border-indigo-400 shadow-indigo-100 shadow-lg hover:shadow-xl hover:scale-105 transition-all duration-200" 
                  onclick={() => openWatermarkedImage(image.filename, true)}
                  disabled={processingWatermarksVar}
                >
                  {#if image.loading}
                    <div class="w-full h-full bg-background-100 flex items-center justify-center">
                      <div class="text-center">
                        <div class="animate-spin w-6 h-6 border-3 border-indigo-500 border-t-transparent rounded-full mx-auto mb-2"></div>
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

                  <!-- Advanced + Watermark indicator -->
                  <div class="absolute top-2 right-2 bg-indigo-500 text-white text-xs px-2 py-1 rounded-full shadow-lg flex items-center space-x-1">
                    <span>🎨🏷️</span>
                    <span class="hidden sm:inline">Pro</span>
                  </div>

                  <!-- Filename -->
                  <div class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-indigo-900/80 via-indigo-800/60 to-transparent p-3 pt-6">
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

    <!-- Completion Section -->
    <div class="bg-gradient-to-r from-green-50 to-emerald-50 rounded-xl p-8 border border-green-200">
      <div class="text-center">
        <div class="w-16 h-16 bg-green-500 rounded-full flex items-center justify-center mx-auto mb-4">
          <span class="text-2xl ">🎉</span>
        </div>
        <h3 class="text-2xl font-bold text-green-900 mb-3">Workflow Complete!</h3>
        <p class="text-green-700 mb-6 max-w-2xl mx-auto">
          Congratulations! Your images have been processed through all workflow steps. 
          The WATERMARK folders contain your final, publication-ready images with professional watermarks applied.
        </p>
        <div class="flex flex-col sm:flex-row items-center justify-center space-y-3 sm:space-y-0 sm:space-x-4 text-green-800">
          <a 
            href="/properties/{property.id}" 
            class="btn-primary flex items-center space-x-2"
          >
            <span>👁️</span>
            <span>View Property Overview</span>
          </a>
          <a 
            href="/" 
            class="btn-secondary flex items-center space-x-2"
          >
            <span>🏠</span>
            <span>Back to Dashboard</span>
          </a>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>

</style>
