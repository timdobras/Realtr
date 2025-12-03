<script lang="ts">
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Set } from '$lib/types/database';
  import { formatDate, isValidDate } from '$lib/utils/dateUtils';
  import ConfirmDialog from './ConfirmDialog.svelte';

  interface Props {
    set: Set;
    onDelete: () => void;
    onViewDetails: () => void;
  }

  let { set, onDelete, onViewDetails }: Props = $props();

  let isDeleting = $state(false);
  let showDeleteConfirm = $state(false);
  let showActions = $state(false);
  let openFolderError = $state('');

  async function openSetsFolder() {
    try {
      openFolderError = '';
      const result = await DatabaseService.openSetsFolder();
      if (!result.success) {
        openFolderError = result.error || 'Failed to open folder';
        setTimeout(() => (openFolderError = ''), 3000);
      }
    } catch (error) {
      console.error('Failed to open sets folder:', error);
      openFolderError = 'Failed to open folder';
      setTimeout(() => (openFolderError = ''), 3000);
    }
  }

  async function confirmDelete() {
    try {
      isDeleting = true;
      const result = await DatabaseService.deleteSet(set.id!, true);
      if (result.success) {
        showDeleteConfirm = false;
        onDelete();
      } else {
        openFolderError = result.error || 'Failed to delete set';
        setTimeout(() => (openFolderError = ''), 3000);
      }
    } catch (error) {
      console.error('Failed to delete set:', error);
      openFolderError = 'Failed to delete set';
      setTimeout(() => (openFolderError = ''), 3000);
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

  function formatSetDate(timestamp: number): string {
    if (!isValidDate(timestamp)) {
      return 'Unknown';
    }
    const date = new Date(timestamp);
    return date.toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  }
</script>

<div
  class="border-background-200 hover:border-background-300 group border transition-colors"
  role="button"
  tabindex="0"
  onmouseenter={() => (showActions = true)}
  onmouseleave={() => (showActions = false)}
>
  <!-- Card Content -->
  <div class="relative p-4">
    <!-- Icon and Title -->
    <div class="flex items-start gap-3">
      <div
        class="bg-accent-100 text-accent-600 flex h-10 w-10 flex-shrink-0 items-center justify-center rounded"
      >
        <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"
          />
        </svg>
      </div>
      <div class="min-w-0 flex-1">
        <h3 class="text-foreground-900 truncate text-sm font-medium">{set.name}</h3>
        <p class="text-foreground-500 text-xs">{formatSetDate(set.created_at)}</p>
      </div>
    </div>

    <!-- Stats -->
    <div class="mt-4 flex items-center gap-4">
      <div class="bg-background-100 flex-1 rounded p-2 text-center">
        <p class="text-foreground-900 text-lg font-semibold">{set.property_count}</p>
        <p class="text-foreground-500 text-xs">Properties</p>
      </div>
    </div>

    <!-- ZIP Path (truncated) -->
    <div class="mt-3">
      <p class="text-foreground-500 truncate font-mono text-xs" title={set.zip_path}>
        {set.zip_path}
      </p>
    </div>

    <div
      class="border-background-200 bg-background-50/95 absolute bottom-0 left-0 flex w-full items-center justify-end gap-1 border-t px-3 py-2 backdrop-blur-sm transition-opacity {showActions
        ? 'opacity-100'
        : 'opacity-0'}"
    >
      <!-- View Details -->
      <button
        onclick={(e) => {
          e.stopPropagation();
          onViewDetails();
        }}
        class="text-foreground-600 hover:bg-background-100 hover:text-foreground-900 p-1.5 transition-colors"
        aria-label="View details"
        title="View details"
      >
        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
      </button>

      <!-- Open Folder -->
      <button
        onclick={(e) => {
          e.stopPropagation();
          openSetsFolder();
        }}
        class="text-foreground-600 hover:bg-background-100 hover:text-foreground-900 p-1.5 transition-colors"
        aria-label="Open folder"
        title="Open sets folder"
      >
        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
          />
        </svg>
      </button>

      <!-- Delete -->
      <button
        onclick={(e) => {
          e.stopPropagation();
          showDeleteConfirm = true;
        }}
        class="text-foreground-600 hover:bg-background-100 p-1.5 transition-colors hover:text-red-600"
        aria-label="Delete set"
        title="Delete set"
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

    <!-- Error Message -->
    {#if openFolderError}
      <div class="border-t border-red-200 bg-red-50 px-3 py-1.5">
        <p class="text-xs text-red-600">{openFolderError}</p>
      </div>
    {/if}
  </div>
</div>

<!-- Delete Confirmation Dialog -->
<ConfirmDialog
  bind:open={showDeleteConfirm}
  title="Delete Set"
  message="Are you sure you want to delete this set? The ZIP file will also be deleted. This action cannot be undone."
  confirmText="Delete"
  destructive={true}
  onConfirm={confirmDelete}
  onCancel={() => (showDeleteConfirm = false)}
/>
