<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
  import { formatDate } from '$lib/utils/dateUtils';
  import SetCodeModal from '$lib/components/SetCodeModal.svelte';
  export const prerender = false;

  let property = $state<Property | null>(null);
  let originalImages = $state<{ filename: string; dataUrl: string; loading: boolean }[]>([]);
  let error = $state('');
  let loading = $state(true);
  let folderMessage = $state('');
  let folderMessageType = $state<'success' | 'error' | ''>('');
  let showCodeModal = $state(false);

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
        folderPath: property.folder_path,
        status: property.status
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
              status: property.status,
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
      const result = await DatabaseService.openImagesInFolder(
        property.folder_path,
        property.status,
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
      const result: any = await invoke('open_property_folder', {
        folderPath: property.folder_path,
        status: property.status
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
      const result: any = await invoke('get_full_property_path', {
        folderPath: property.folder_path,
        status: property.status
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

  async function refreshProperty() {
    if (isNaN(propertyId) || propertyId < 1) return;
    property = await DatabaseService.getPropertyById(propertyId);
  }

  // Workflow steps data - derived so it updates when propertyId changes
  let workflowSteps = $derived([
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
  ]);
</script>

{#if loading}
  <div class="flex h-64 items-center justify-center">
    <div class="text-foreground-500 flex items-center gap-2 text-sm">
      <div
        class="border-foreground-300 h-4 w-4 animate-spin rounded-full border-2 border-t-transparent"
      ></div>
      <span>Loading...</span>
    </div>
  </div>
{:else if error}
  <div class="p-8">
    <div class="rounded-lg border border-red-300 bg-red-50 px-4 py-3">
      <p class="text-sm text-red-800">{error}</p>
    </div>
  </div>
{:else if property}
  <div class="space-y-4 p-4">
    <!-- Property Details -->
    <section class="bg-background-50 border-background-200 rounded-lg border p-4">
      <div class="mb-3 flex items-start justify-between">
        <div>
          <div class="mb-2 flex items-center gap-3">
            <h1 class="text-foreground-900 text-lg font-semibold">{property.name}</h1>
            {#if property.code}
              <span class="bg-accent-100 text-accent-700 rounded px-2 py-0.5 text-xs font-medium">
                {property.code}
              </span>
            {/if}
          </div>
          <div class="text-foreground-600 flex items-center gap-4 text-sm">
            <span>{property.city}</span>
            <span
              class="inline-flex items-center rounded border px-2 py-0.5 text-xs {property.status === 'DONE'
                ? 'border-green-300 bg-green-50 text-green-700'
                : property.status === 'ARCHIVE'
                  ? 'border-gray-300 bg-gray-50 text-gray-700'
                  : property.status === 'NOT_FOUND'
                    ? 'border-yellow-300 bg-yellow-50 text-yellow-700'
                    : 'border-blue-300 bg-blue-50 text-blue-700'}"
            >
              {property.status === 'DONE'
                ? 'Done'
                : property.status === 'ARCHIVE'
                  ? 'Archived'
                  : property.status === 'NOT_FOUND'
                    ? 'Not Found'
                    : 'New'}
            </span>
          </div>
        </div>

        <!-- Action Buttons -->
        <div class="flex items-center gap-2">
          <button
            onclick={() => (showCodeModal = true)}
            class="bg-background-100 hover:bg-background-200 text-foreground-700 flex items-center gap-1.5 rounded px-3 py-1.5 text-xs transition-colors"
            title={property.code ? 'Edit Code' : 'Add Code'}
          >
            <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M7 20l4-16m2 16l4-16M6 9h14M4 15h14"
              />
            </svg>
            <span>{property.code ? 'Edit Code' : 'Add Code'}</span>
          </button>

          <button
            onclick={openPropertyFolder}
            class="bg-background-100 hover:bg-background-200 text-foreground-700 flex items-center gap-1.5 rounded px-3 py-1.5 text-xs transition-colors"
            title="Open Property Folder"
          >
            <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
              />
            </svg>
            <span>Open Folder</span>
          </button>

          <button
            onclick={copyFolderPath}
            class="bg-background-100 hover:bg-background-200 text-foreground-700 flex items-center gap-1.5 rounded px-3 py-1.5 text-xs transition-colors"
            title="Copy Folder Path"
          >
            <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
      <div class="bg-background-100 mb-3 rounded px-3 py-2">
        <p class="text-foreground-500 mb-1 text-xs">Folder Path</p>
        <p class="text-foreground-700 font-mono text-xs break-all">{property.folder_path}</p>
      </div>

      <!-- Success/Error Message -->
      {#if folderMessage}
        <div
          class="mb-3 rounded border px-3 py-2 text-xs {folderMessageType === 'success'
            ? 'border-green-300 bg-green-50 text-green-800'
            : 'border-red-300 bg-red-50 text-red-800'}"
        >
          {folderMessage}
        </div>
      {/if}

      <!-- Notes -->
      {#if property.notes}
        <div class="bg-background-100 mb-3 rounded px-3 py-2">
          <p class="text-foreground-500 mb-1 text-xs">Notes</p>
          <p class="text-foreground-700 whitespace-pre-wrap text-sm">{property.notes}</p>
        </div>
      {/if}

      <!-- Timestamps -->
      <div class="border-background-200 text-foreground-500 flex items-center gap-4 border-t pt-2 text-xs">
        <span>Created: {formatDate(property.created_at)}</span>
        {#if property.updated_at !== property.created_at}
          <span>Updated: {formatDate(property.updated_at)}</span>
        {/if}
      </div>
    </section>

    <!-- Workflow Steps Navigation -->
    <section class="bg-background-50 border-background-200 rounded-lg border p-4">
      <h2 class="text-foreground-900 mb-3 text-sm font-semibold">Workflow Steps</h2>

      <div class="grid grid-cols-1 gap-2 md:grid-cols-2 lg:grid-cols-4">
        {#each workflowSteps as step}
          <a
            href={step.href}
            class="bg-background-100 hover:bg-background-200 border-background-200 flex items-center gap-3 rounded border p-3 transition-colors"
          >
            <div class="text-foreground-600 flex h-8 w-8 flex-shrink-0 items-center justify-center">
              {@html step.icon}
            </div>
            <div class="min-w-0 flex-1">
              <p class="text-foreground-900 mb-0.5 text-xs font-medium">
                Step {step.number}: {step.title}
              </p>
              <p class="text-foreground-600 text-xs">{step.description}</p>
            </div>
          </a>
        {/each}
      </div>
    </section>

    <!-- Original Images Gallery -->
    <section class="bg-background-50 border-background-200 rounded-lg border p-4">
      <div class="mb-3 flex items-center justify-between">
        <h2 class="text-foreground-900 text-sm font-semibold">
          Original Images ({originalImages.length})
        </h2>
        {#if originalImages.length > 0}
          <span class="text-foreground-500 text-xs">Click to open</span>
        {/if}
      </div>

      {#if originalImages.length === 0}
        <div class="bg-background-100 rounded py-8 text-center">
          <p class="text-foreground-500 mb-3 text-sm">No original images found</p>
          <button
            onclick={openPropertyFolder}
            class="bg-background-200 hover:bg-background-300 text-foreground-700 inline-flex items-center gap-1.5 rounded px-3 py-1.5 text-xs transition-colors"
          >
            <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
              />
            </svg>
            <span>Open Folder</span>
          </button>
        </div>
      {:else}
        <div class="grid grid-cols-3 gap-2 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6">
          {#each originalImages as image}
            <button
              onclick={() => openImage(image.filename)}
              class="bg-background-100 hover:bg-background-200 aspect-square overflow-hidden rounded transition-colors"
              title={image.filename}
            >
              {#if image.loading}
                <div class="flex h-full items-center justify-center">
                  <div
                    class="border-foreground-300 h-3 w-3 animate-spin rounded-full border-2 border-t-transparent"
                  ></div>
                </div>
              {:else if image.dataUrl}
                <img
                  src={image.dataUrl}
                  alt={image.filename}
                  loading="lazy"
                  class="h-full w-full object-cover"
                />
              {:else}
                <div class="flex h-full items-center justify-center text-xs text-red-700">
                  Failed
                </div>
              {/if}
            </button>
          {/each}
        </div>
      {/if}
    </section>
  </div>
{/if}

<!-- Set Code Modal -->
{#if showCodeModal && property}
  <SetCodeModal
    propertyId={property.id!}
    propertyName={property.name}
    currentCode={property.code}
    onClose={() => (showCodeModal = false)}
    onCodeSet={() => {
      showCodeModal = false;
      refreshProperty();
    }}
  />
{/if}
