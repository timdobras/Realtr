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
  class="bg-background-50 border-background-200 hover:border-accent-200 group rounded-xl border shadow-sm transition-all duration-200 hover:shadow-md"
  onmouseenter={() => (showActions = true)}
  onmouseleave={() => (showActions = false)}
>
  <!-- Main Content -->
  <div class="p-6">
    <div class="flex items-start justify-between">
      <div class="min-w-0 flex-1">
        <a href="/properties/{property.id}" class="mb-3 flex items-center space-x-3">
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
                d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"
              />
            </svg>
          </div>
          <div class="min-w-0 flex-1">
            <h3
              class="text-foreground-900 group-hover:text-accent-600 truncate text-lg font-semibold transition-colors"
            >
              {property.name}
            </h3>
            <div class="text-foreground-600 mt-1 flex items-center space-x-2 text-sm">
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
              <span class="truncate">{property.city}</span>
            </div>
          </div>
        </a>

        <!-- Status and Date -->
        <div class="flex items-center justify-between">
          <span
            class="inline-flex items-center rounded-lg border px-3 py-1 text-xs font-medium transition-colors {property.completed
              ? 'border-green-200 bg-green-50 text-green-700'
              : 'border-orange-200 bg-orange-50 text-orange-700'}"
          >
            {property.completed ? 'Completed' : 'In Progress'}
          </span>

          <span class="text-foreground-500 text-xs">
            {formatDisplayDate(property.updated_at)}
          </span>
        </div>
      </div>
    </div>

    <!-- Error Message -->
    {#if openFolderError}
      <div class="mt-4 rounded-lg border border-red-200 bg-red-50 p-3">
        <div class="flex items-center space-x-2">
          <svg class="h-4 w-4 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
          <p class="text-sm text-red-700">{openFolderError}</p>
        </div>
      </div>
    {/if}

    <!-- Actions (shown on hover) -->
    <div
      class="mt-4 transition-all duration-200 {showActions
        ? 'max-h-20 opacity-100'
        : 'max-h-0 overflow-hidden opacity-0'}"
    >
      <div class="border-background-200 flex items-center justify-between border-t pt-4">
        <!-- Quick Status Toggle -->
        <button
          onclick={toggleStatus}
          disabled={isUpdating}
          class="flex items-center space-x-2 rounded-lg px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50 {property.completed
            ? 'bg-orange-100 text-orange-700 hover:bg-orange-200'
            : 'bg-green-100 text-green-700 hover:bg-green-200'}"
        >
          {#if isUpdating}
            <div
              class="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"
            ></div>
          {:else if property.completed}
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 8v4l3 3"
              />
            </svg>
          {:else}
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 12l2 2 4-4"
              />
            </svg>
          {/if}
          <span>{property.completed ? 'Reopen' : 'Complete'}</span>
        </button>

        <!-- Action Buttons -->
        <div class="flex items-center space-x-2">
          <button
            onclick={openPropertyFolder}
            class="text-foreground-600 hover:text-accent-600 hover:bg-accent-50 rounded-lg p-2 transition-colors"
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
            class="text-foreground-600 rounded-lg p-2 transition-colors hover:bg-red-50 hover:text-red-600"
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
    </div>
  </div>
</div>

<!-- Simplified Delete Confirmation Modal -->
{#if showDeleteConfirm}
  <div class="bg-opacity-50 fixed inset-0 z-50 flex items-center justify-center bg-black p-4">
    <div class="bg-background-50 border-background-200 w-full max-w-md rounded-xl border shadow-xl">
      <div class="p-6">
        <div class="mb-4 flex items-center space-x-3">
          <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-red-100">
            <svg class="h-5 w-5 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
          </div>
          <div>
            <h3 class="text-foreground-900 text-lg font-semibold">Delete Property</h3>
            <p class="text-foreground-600 text-sm">This action cannot be undone</p>
          </div>
        </div>

        <p class="text-foreground-700 mb-6 text-sm">
          Are you sure you want to delete "<strong>{property.name}</strong>" from
          <strong>{property.city}</strong>?
        </p>

        <div class="flex items-center justify-end space-x-3">
          <button
            onclick={() => (showDeleteConfirm = false)}
            disabled={isDeleting}
            class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 rounded-lg border px-4 py-2 text-sm font-medium transition-colors"
          >
            Cancel
          </button>
          <button
            onclick={confirmDelete}
            disabled={isDeleting}
            class="flex items-center space-x-2 rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-red-700 disabled:opacity-50"
          >
            {#if isDeleting}
              <div
                class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
              ></div>
              <span>Deleting...</span>
            {:else}
              <span>Delete</span>
            {/if}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
