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

  // Workflow steps data
  const workflowSteps = [
    {
      number: 1,
      title: 'Copy to INTERNET',
      description: 'Copy originals to INTERNET folder',
      href: `/properties/${propertyId}/step1`,
      icon: `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"/>
      </svg>`,
      color: 'accent'
    },
    {
      number: 2,
      title: 'Order & Rename',
      description: 'Order and rename images',
      href: `/properties/${propertyId}/step2`,
      icon: `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4"/>
      </svg>`,
      color: 'green'
    },
    {
      number: 3,
      title: 'Copy to AGGELIA',
      description: 'Copy edited images to AGGELIA',
      href: `/properties/${propertyId}/step3`,
      icon: `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"/>
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 5a2 2 0 012-2h4a2 2 0 012 2v6H8V5z"/>
      </svg>`,
      color: 'orange'
    },
    {
      number: 4,
      title: 'Add Watermark',
      description: 'Apply watermark to final images',
      href: `/properties/${propertyId}/step4`,
      icon: `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"/>
      </svg>`,
      color: 'purple'
    }
  ];
</script>

{#if loading}
  <div class="flex h-64 items-center justify-center">
    <div class="text-center">
      <div
        class="border-accent-500 mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-t-transparent"
      ></div>
      <p class="text-foreground-600 font-medium">Loading property data...</p>
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
  <div class="space-y-8 p-6">
    <!-- Property Details -->
    <section class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-6 flex items-start justify-between">
        <div class="flex flex-1 items-center space-x-4">
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
                d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"
              />
            </svg>
          </div>
          <div>
            <h1 class="text-foreground-900 mb-2 text-2xl font-bold">{property.name}</h1>
            <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
              <div class="flex items-center space-x-2">
                <svg
                  class="text-foreground-600 h-4 w-4"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z"
                  />
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M15 11a3 3 0 11-6 0 3 3 0 016 0z"
                  />
                </svg>
                <span class="text-foreground-700 font-medium">City:</span>
                <span class="text-foreground-600">{property.city}</span>
              </div>
              <div class="flex items-center space-x-2">
                <svg
                  class="text-foreground-600 h-4 w-4"
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
                <span class="text-foreground-700 font-medium">Status:</span>
                <span
                  class="inline-flex items-center rounded-lg border px-2.5 py-1 text-xs font-medium {property.completed
                    ? 'border-green-200 bg-green-50 text-green-700'
                    : 'border-orange-200 bg-orange-50 text-orange-700'}"
                >
                  {property.completed ? 'Completed' : 'In Progress'}
                </span>
              </div>
            </div>
          </div>
        </div>

        <!-- Action Buttons -->
        <div class="flex items-center space-x-3">
          <button
            onclick={openPropertyFolder}
            class="border-accent-200 bg-accent-50 text-accent-700 hover:bg-accent-100 flex items-center space-x-2 rounded-lg border px-4 py-2 text-sm font-medium transition-all duration-200"
            title="Open Property Folder"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
            <span>Open Folder</span>
          </button>

          <button
            onclick={copyFolderPath}
            class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 flex items-center space-x-2 rounded-lg border px-4 py-2 text-sm font-medium transition-all duration-200"
            title="Copy Folder Path"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
              />
            </svg>
            <span>Copy Path</span>
          </button>
        </div>
      </div>

      <!-- Folder Path Display -->
      <div class="bg-background-100 border-background-200 mb-4 rounded-lg border p-4">
        <div class="flex items-start space-x-3">
          <svg
            class="text-foreground-600 mt-0.5 h-5 w-5"
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
          </svg>
          <div class="min-w-0 flex-1">
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
          class="mb-4 rounded-lg border p-4 {folderMessageType === 'success'
            ? 'border-green-200 bg-green-50'
            : 'border-red-200 bg-red-50'}"
        >
          <div class="flex items-center space-x-3">
            {#if folderMessageType === 'success'}
              <svg
                class="h-5 w-5 text-green-600"
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
            {:else}
              <svg
                class="h-5 w-5 text-red-600"
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
            {/if}
            <span
              class="text-sm font-medium {folderMessageType === 'success'
                ? 'text-green-800'
                : 'text-red-800'}">{folderMessage}</span
            >
          </div>
        </div>
      {/if}

      <!-- Notes -->
      {#if property.notes}
        <div class="bg-background-100 border-background-200 mb-4 rounded-lg border p-4">
          <div class="flex items-start space-x-3">
            <svg
              class="text-foreground-600 mt-0.5 h-5 w-5"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
              />
            </svg>
            <div>
              <p class="text-foreground-500 mb-2 text-xs font-medium tracking-wide uppercase">
                Notes
              </p>
              <p class="text-foreground-700 whitespace-pre-wrap">{property.notes}</p>
            </div>
          </div>
        </div>
      {/if}

      <!-- Timestamps -->
      <div
        class="border-background-200 text-foreground-500 flex items-center space-x-6 border-t pt-4 text-xs"
      >
        <div class="flex items-center space-x-2">
          <div class="h-1.5 w-1.5 rounded-full bg-green-400"></div>
          <span>Created: {formatDate(property.created_at)}</span>
        </div>
        {#if property.updated_at !== property.created_at}
          <div class="flex items-center space-x-2">
            <div class="bg-accent-400 h-1.5 w-1.5 rounded-full"></div>
            <span>Updated: {formatDate(property.updated_at)}</span>
          </div>
        {/if}
      </div>
    </section>

    <!-- Workflow Steps Navigation -->
    <section class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-6 flex items-center space-x-3">
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
              d="M9 5H7a2 2 0 00-2 2v10a2 2 0 002 2h8a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"
            />
          </svg>
        </div>
        <div>
          <h2 class="text-foreground-900 text-xl font-semibold">Workflow Steps</h2>
          <p class="text-foreground-600 text-sm">
            Complete each step to process your property photos
          </p>
        </div>
      </div>

      <div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
        {#each workflowSteps as step}
          <a
            href={step.href}
            class="group flex items-center space-x-4 rounded-xl border p-4 transition-all duration-200 hover:shadow-md {step.color ===
            'accent'
              ? 'border-accent-200 bg-accent-50 hover:bg-accent-100'
              : step.color === 'green'
                ? 'border-green-200 bg-green-50 hover:bg-green-100'
                : step.color === 'orange'
                  ? 'border-orange-200 bg-orange-50 hover:bg-orange-100'
                  : 'border-purple-200 bg-purple-50 hover:bg-purple-100'}"
          >
            <div
              class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-lg {step.color ===
              'accent'
                ? 'bg-accent-100 text-accent-600'
                : step.color === 'green'
                  ? 'bg-green-100 text-green-600'
                  : step.color === 'orange'
                    ? 'bg-orange-100 text-orange-600'
                    : 'bg-purple-100 text-purple-600'}"
            >
              {@html step.icon}
            </div>
            <div class="min-w-0 flex-1">
              <div class="mb-1 flex items-center space-x-2">
                <p
                  class="text-sm font-semibold {step.color === 'accent'
                    ? 'text-accent-900'
                    : step.color === 'green'
                      ? 'text-green-900'
                      : step.color === 'orange'
                        ? 'text-orange-900'
                        : 'text-purple-900'}"
                >
                  Step {step.number}
                </p>
              </div>
              <p
                class="text-sm font-medium {step.color === 'accent'
                  ? 'text-accent-800'
                  : step.color === 'green'
                    ? 'text-green-800'
                    : step.color === 'orange'
                      ? 'text-orange-800'
                      : 'text-purple-800'} group-hover:underline"
              >
                {step.title}
              </p>
              <p
                class="text-xs {step.color === 'accent'
                  ? 'text-accent-700'
                  : step.color === 'green'
                    ? 'text-green-700'
                    : step.color === 'orange'
                      ? 'text-orange-700'
                      : 'text-purple-700'}"
              >
                {step.description}
              </p>
            </div>
          </a>
        {/each}
      </div>
    </section>

    <!-- Original Images Gallery -->
    <section class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
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
              Original Images ({originalImages.length})
            </h2>
            <p class="text-foreground-600 text-sm">Click images to open in system viewer</p>
          </div>
        </div>

        {#if originalImages.length > 0}
          <div class="text-foreground-500 flex items-center space-x-2 text-sm">
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
            <span>Click to open</span>
          </div>
        {/if}
      </div>

      {#if originalImages.length === 0}
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
                d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
              />
            </svg>
          </div>
          <h3 class="text-foreground-900 mb-2 text-lg font-semibold">No original images found</h3>
          <p class="text-foreground-500 mx-auto mb-6 max-w-md">
            Upload some images to this property folder to get started with your workflow.
          </p>
          <button
            onclick={openPropertyFolder}
            class="bg-accent-500 hover:bg-accent-600 inline-flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
              />
            </svg>
            <span>Open Property Folder</span>
          </button>
        </div>
      {:else}
        <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
          {#each originalImages as image}
            <div class="group relative">
              <button
                class="border-background-200 bg-background-100 hover:border-accent-200 aspect-square w-full overflow-hidden rounded-xl border transition-all duration-200 hover:scale-[1.02] hover:shadow-lg"
                onclick={() => openImage(image.filename)}
              >
                {#if image.loading}
                  <!-- Loading state -->
                  <div class="bg-background-100 flex h-full w-full items-center justify-center">
                    <div class="text-center">
                      <div
                        class="border-accent-500 mx-auto mb-2 h-6 w-6 animate-spin rounded-full border-2 border-t-transparent"
                      ></div>
                      <span class="text-foreground-500 text-xs font-medium">Loading...</span>
                    </div>
                  </div>
                {:else if image.dataUrl}
                  <!-- Actual image -->
                  <img
                    src={image.dataUrl}
                    alt={image.filename}
                    loading="lazy"
                    class="h-full w-full object-cover transition-transform duration-300"
                  />
                {:else}
                  <!-- Error fallback -->
                  <div class="flex h-full w-full items-center justify-center bg-red-50">
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
                      <span class="text-xs font-medium">Failed to load</span>
                    </div>
                  </div>
                {/if}

                <!-- Filename overlay -->
                <div
                  class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black via-black/60 to-transparent p-3 pt-8 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                >
                  <p class="truncate text-xs font-medium text-white" title={image.filename}>
                    {image.filename}
                  </p>
                </div>

                <!-- Click indicator -->
                <div
                  class="absolute inset-0 flex items-center justify-center bg-black/20 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                >
                  <div class="rounded-full bg-white/90 p-2">
                    <svg
                      class="h-5 w-5 text-black"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                      />
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                      />
                    </svg>
                  </div>
                </div>
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  </div>
{/if}
