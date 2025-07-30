<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
  import { formatDate, isValidDate } from '$lib/utils/dateUtils';

  interface Props {
    property: Property;
    onUpdate: () => void;
    onDelete: () => void;
  }

  let { property, onUpdate, onDelete }: Props = $props();

  let isUpdating = $state(false);
  let isDeleting = $state(false);
  let showDeleteConfirm = $state(false);
  let openFolderError = $state('');

  async function toggleStatus() {
    try {
      isUpdating = true;
      await DatabaseService.updatePropertyStatus(property.id!, !property.completed);
      onUpdate();
    } catch (error) {
      console.error('Failed to update property status:', error);
    } finally {
      isUpdating = false;
    }
  }

  async function openPropertyFolder() {
    try {
      openFolderError = '';
      const result = await invoke('open_property_folder', {
        folderPath: property.folder_path
      });

      if (!result.success) {
        openFolderError = result.error || 'Failed to open folder';
        // Clear error after 3 seconds
        setTimeout(() => (openFolderError = ''), 3000);
      }
    } catch (error) {
      console.error('Failed to open property folder:', error);
      openFolderError = 'Failed to open folder';
      setTimeout(() => (openFolderError = ''), 3000);
    }
  }

  async function confirmDelete() {
    try {
      isDeleting = true;
      await DatabaseService.deleteProperty(property.id!);
      showDeleteConfirm = false;
      onDelete();
    } catch (error) {
      console.error('Failed to delete property:', error);
    } finally {
      isDeleting = false;
    }
  }

  function formatDisplayDate(timestamp: number): string {
    if (!isValidDate(timestamp)) {
      return 'Unknown date';
    }
    return formatDate(timestamp);
  }

  // Calculate workflow progress (this could be enhanced with actual step completion data)
  const workflowProgress = $derived(property.completed ? 100 : 25); // Basic estimation
</script>

<div
  class="bg-background-100 border-background-200 overflow-hidden rounded-xl border shadow-sm transition-all duration-200 hover:shadow-lg"
>
  <!-- Header Section -->
  <div class="p-6 pb-4">
    <div class="mb-3 flex items-start justify-between">
      <div class="flex-1">
        <a
          href="/properties/{property.id}"
          class="text-foreground-900 mb-2 block text-lg font-semibold transition-colors hover:text-blue-600"
        >
          {property.name}
        </a>
        <div class="text-foreground-600 flex items-center space-x-2 text-sm">
          <span>{property.city}</span>
        </div>
      </div>

      <!-- Status Badge -->
      <div class="flex flex-col items-end space-y-2">
        <span
          class="rounded-full px-3 py-1 text-xs font-semibold shadow-sm {property.completed
            ? 'border border-green-200 bg-green-100 text-green-700'
            : 'border border-orange-200 bg-orange-100 text-orange-700'}"
        >
          {property.completed ? 'Completed' : 'In Progress'}
        </span>
      </div>
    </div>

    <!-- Notes Section -->
    <!-- {#if property.notes}
      <div class="mt-4">
        <div class="bg-background-50 border border-background-200 rounded-lg p-3">
          <p class="text-sm text-foreground-700 leading-relaxed">
            {property.notes}
          </p>
        </div>
      </div>
    {/if} -->

    <!-- Error Message -->
    {#if openFolderError}
      <div class="mt-4 rounded-lg border border-red-200 bg-red-50 p-3">
        <p class="flex items-center space-x-2 text-sm text-red-700">
          <span>❌</span>
          <span>{openFolderError}</span>
        </p>
      </div>
    {/if}
  </div>

  <!-- Metadata Section -->
  <div class="bg-background-50 border-background-200 border-t px-6 py-3">
    <div class="text-foreground-500 flex items-center justify-between text-xs">
      <div class="space-y-1">
        {#if property.updated_at !== property.created_at}
          <div class="flex items-center space-x-2">
            <span class="h-1.5 w-1.5 rounded-full bg-blue-400"></span>
            <span>Updated: {formatDisplayDate(property.updated_at)}</span>
          </div>
        {:else}
          <div class="flex items-center space-x-2">
            <span class="h-1.5 w-1.5 rounded-full bg-green-400"></span>
            <span>Created: {formatDisplayDate(property.created_at)}</span>
          </div>
        {/if}
      </div>
    </div>
  </div>

  <!-- Actions Section -->
  <div class="bg-background-100 border-background-100 border-t p-4">
    <div class="flex items-center justify-between">
      <!-- Status Toggle -->
      <button
        onclick={toggleStatus}
        disabled={isUpdating}
        class="flex items-center space-x-2 rounded-lg px-4 py-2 text-sm font-medium transition-all duration-200 disabled:cursor-not-allowed disabled:opacity-50 {property.completed
          ? 'border border-orange-200 bg-orange-100 text-orange-700 hover:bg-orange-200'
          : 'border border-green-200 bg-green-100 text-green-700 hover:bg-green-200'}"
      >
        {#if isUpdating}
          <div
            class="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"
          ></div>
          <span>Updating...</span>
        {:else}
          <span>{property.completed ? 'Mark In Progress' : 'Mark Complete'}</span>
        {/if}
      </button>

      <!-- Action Buttons -->
      <div class="flex items-center space-x-2">
        <!-- Open Folder -->
        <button
          onclick={openPropertyFolder}
          class="flex items-center space-x-2 rounded-lg border border-gray-200 bg-gray-100 px-3 py-2 text-sm font-medium text-gray-700 transition-all duration-200 hover:bg-gray-200"
          title="Open Property Folder"
        >
          <span class="hidden sm:inline">Folder</span>
        </button>

        <!-- Delete -->
        <button
          onclick={() => (showDeleteConfirm = true)}
          class="flex items-center space-x-2 rounded-lg border border-red-200 bg-red-100 px-3 py-2 text-sm font-medium text-red-700 transition-all duration-200 hover:bg-red-200"
          title="Delete Property"
        >
          <span class="hidden sm:inline">Delete</span>
        </button>
      </div>
    </div>
  </div>
</div>

<!-- Enhanced Delete Confirmation Modal -->
{#if showDeleteConfirm}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm">
    <div class="bg-background-100 w-full max-w-md transform rounded-xl shadow-2xl transition-all">
      <!-- Modal Header -->
      <div class="p-6 pb-4">
        <div class="mb-4 flex items-center space-x-3">
          <div class="flex h-12 w-12 items-center justify-center rounded-full bg-red-100">
            <span class="text-2xl">⚠️</span>
          </div>
          <div>
            <h3 class="text-foreground-900 text-lg font-semibold">Delete Property</h3>
            <p class="text-foreground-600 text-sm">This action cannot be undone</p>
          </div>
        </div>

        <div class="rounded-lg border border-red-200 bg-red-50 p-4">
          <p class="text-foreground-700 text-sm">
            Are you sure you want to delete <strong>"{property.name}"</strong> from
            <strong>{property.city}</strong>?
          </p>
          <ul class="mt-3 space-y-1 text-xs text-red-700">
            <li>• Property record will be permanently removed</li>
            <li>• Folder structure will remain on disk</li>
            <li>• This action cannot be undone</li>
          </ul>
        </div>
      </div>

      <!-- Modal Actions -->
      <div class="flex items-center justify-end space-x-3 p-6 pt-0">
        <button
          onclick={() => (showDeleteConfirm = false)}
          disabled={isDeleting}
          class="text-foreground-700 bg-background-100 border-background-300 hover:bg-background-200 rounded-lg border px-4 py-2 font-medium transition-colors disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          onclick={confirmDelete}
          disabled={isDeleting}
          class="flex items-center space-x-2 rounded-lg bg-red-600 px-4 py-2 font-medium text-white transition-colors hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {#if isDeleting}
            <div
              class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
            ></div>
            <span>Deleting...</span>
          {:else}
            <span>🗑️</span>
            <span>Delete Property</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
</style>
