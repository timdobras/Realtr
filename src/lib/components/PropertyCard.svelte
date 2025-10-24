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
  let showActions = $state(false);
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
</script>

<div
  class="bg-background-50 border-background-200 hover:border-background-300 group rounded-lg border transition-colors"
  onmouseenter={() => (showActions = true)}
  onmouseleave={() => (showActions = false)}
>
  <a href="/properties/{property.id}" class="block p-4">
    <div class="mb-3">
      <h3 class="text-foreground-900 group-hover:text-accent-600 mb-1 font-medium transition-colors">
        {property.name}
      </h3>
      <p class="text-foreground-500 text-xs">{property.city}</p>
    </div>

    <div class="flex items-center justify-between">
      <span
        class="rounded px-2 py-1 text-xs font-medium {property.completed
          ? 'bg-green-100 text-green-700'
          : 'bg-orange-100 text-orange-700'}"
      >
        {property.completed ? 'Completed' : 'In Progress'}
      </span>

      <span class="text-foreground-500 text-xs">
        {formatDisplayDate(property.updated_at)}
      </span>
    </div>
  </a>

  <!-- Error Message -->
  {#if openFolderError}
    <div class="border-background-200 border-t px-4 py-3">
      <p class="text-xs text-red-700">{openFolderError}</p>
    </div>
  {/if}

  <!-- Actions (shown on hover) -->
  {#if showActions}
    <div class="border-background-200 border-t px-4 py-3">
      <div class="flex items-center justify-between gap-2">
        <button
          onclick={toggleStatus}
          disabled={isUpdating}
          class="flex-1 rounded-md px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-50 {property.completed
            ? 'bg-orange-100 text-orange-700 hover:bg-orange-200'
            : 'bg-green-100 text-green-700 hover:bg-green-200'}"
        >
          {#if isUpdating}
            Processing...
          {:else}
            {property.completed ? 'Reopen' : 'Complete'}
          {/if}
        </button>

        <button
          onclick={openPropertyFolder}
          class="text-foreground-600 hover:bg-background-100 hover:text-foreground-900 rounded-md p-1.5 transition-colors"
          title="Open Folder"
        >
          <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
            />
          </svg>
        </button>

        <button
          onclick={() => (showDeleteConfirm = true)}
          class="text-foreground-600 hover:text-red-600 hover:bg-red-50 rounded-md p-1.5 transition-colors"
          title="Delete"
        >
          <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
            />
          </svg>
        </button>
      </div>
    </div>
  {/if}
</div>

<!-- Delete Confirmation Modal -->
{#if showDeleteConfirm}
  <div class="bg-opacity-50 fixed inset-0 z-50 flex items-center justify-center bg-black p-4">
    <div class="bg-background-50 border-background-200 w-full max-w-sm rounded-lg border shadow-xl">
      <div class="p-5">
        <h3 class="text-foreground-900 mb-1 text-base font-semibold">Delete Property</h3>
        <p class="text-foreground-600 mb-4 text-sm">
          Delete "<strong>{property.name}</strong>"? This cannot be undone.
        </p>

        <div class="flex items-center justify-end gap-2">
          <button
            onclick={() => (showDeleteConfirm = false)}
            disabled={isDeleting}
            class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 rounded-md border px-3 py-2 text-sm font-medium transition-colors"
          >
            Cancel
          </button>
          <button
            onclick={confirmDelete}
            disabled={isDeleting}
            class="rounded-md bg-red-600 px-3 py-2 text-sm font-medium text-white transition-colors hover:bg-red-700 disabled:opacity-50"
          >
            {#if isDeleting}
              Deleting...
            {:else}
              Delete
            {/if}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
