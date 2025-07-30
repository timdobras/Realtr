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
        setTimeout(() => openFolderError = '', 3000);
      }
    } catch (error) {
      console.error('Failed to open property folder:', error);
      openFolderError = 'Failed to open folder';
      setTimeout(() => openFolderError = '', 3000);
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

<div class="bg-background-100 rounded-xl shadow-sm border border-background-200 hover:shadow-lg transition-all duration-200 overflow-hidden">
  <!-- Header Section -->
  <div class="p-6 pb-4">
    <div class="flex items-start justify-between mb-3">
      <div class="flex-1">
        <a 
          href="/properties/{property.id}" 
          class="text-lg font-semibold text-foreground-900 hover:text-blue-600 transition-colors block mb-2"
        >
          {property.name}
        </a>
        <div class="flex items-center space-x-2 text-sm text-foreground-600">
          <span>{property.city}</span>
        </div>
      </div>
      
      <!-- Status Badge -->
      <div class="flex flex-col items-end space-y-2">
        <span class="px-3 py-1 text-xs font-semibold rounded-full shadow-sm {property.completed 
          ? 'bg-green-100 text-green-700 border border-green-200' 
          : 'bg-orange-100 text-orange-700 border border-orange-200'}">
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
      <div class="mt-4 bg-red-50 border border-red-200 rounded-lg p-3">
        <p class="text-sm text-red-700 flex items-center space-x-2">
          <span>❌</span>
          <span>{openFolderError}</span>
        </p>
      </div>
    {/if}
  </div>
  
  <!-- Metadata Section -->
  <div class="px-6 py-3 bg-background-50 border-t border-background-200">
    <div class="flex items-center justify-between text-xs text-foreground-500">
      <div class="space-y-1">
        {#if property.updated_at !== property.created_at}
          <div class="flex items-center space-x-2">
            <span class="w-1.5 h-1.5 bg-blue-400 rounded-full"></span>
            <span>Updated: {formatDisplayDate(property.updated_at)}</span>
          </div>
        {:else}
				<div class="flex items-center space-x-2">
          <span class="w-1.5 h-1.5 bg-green-400 rounded-full"></span>
          <span>Created: {formatDisplayDate(property.created_at)}</span>
        </div>
				{/if}
      </div>
      
    </div>
  </div>
  
  <!-- Actions Section -->
  <div class="p-4 bg-background-100 border-t border-background-100">
    <div class="flex items-center justify-between">
      <!-- Status Toggle -->
      <button
        onclick={toggleStatus}
        disabled={isUpdating}
        class="flex items-center space-x-2 px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed {property.completed 
          ? 'bg-orange-100 text-orange-700 hover:bg-orange-200 border border-orange-200' 
          : 'bg-green-100 text-green-700 hover:bg-green-200 border border-green-200'}"
      >
        {#if isUpdating}
          <div class="animate-spin w-4 h-4 border-2 border-current border-t-transparent rounded-full"></div>
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
          class="flex items-center space-x-2 px-3 py-2 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 transition-all duration-200 text-sm font-medium border border-gray-200"
          title="Open Property Folder"
        >
          <span class="hidden sm:inline">Folder</span>
        </button>
        
        <!-- Delete -->
        <button
          onclick={() => showDeleteConfirm = true}
          class="flex items-center space-x-2 px-3 py-2 bg-red-100 text-red-700 rounded-lg hover:bg-red-200 transition-all duration-200 text-sm font-medium border border-red-200"
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
  <div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4">
    <div class="bg-background-100 rounded-xl shadow-2xl max-w-md w-full transform transition-all">
      <!-- Modal Header -->
      <div class="p-6 pb-4">
        <div class="flex items-center space-x-3 mb-4">
          <div class="w-12 h-12 bg-red-100 rounded-full flex items-center justify-center">
            <span class="text-2xl">⚠️</span>
          </div>
          <div>
            <h3 class="text-lg font-semibold text-foreground-900">Delete Property</h3>
            <p class="text-sm text-foreground-600">This action cannot be undone</p>
          </div>
        </div>
        
        <div class="bg-red-50 border border-red-200 rounded-lg p-4">
          <p class="text-foreground-700 text-sm">
            Are you sure you want to delete <strong>"{property.name}"</strong> from <strong>{property.city}</strong>?
          </p>
          <ul class="mt-3 text-xs text-red-700 space-y-1">
            <li>• Property record will be permanently removed</li>
            <li>• Folder structure will remain on disk</li>
            <li>• This action cannot be undone</li>
          </ul>
        </div>
      </div>
      
      <!-- Modal Actions -->
      <div class="p-6 pt-0 flex items-center justify-end space-x-3">
        <button
          onclick={() => showDeleteConfirm = false}
          disabled={isDeleting}
          class="px-4 py-2 text-foreground-700 bg-background-100 border border-background-300 rounded-lg hover:bg-background-200 transition-colors font-medium disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          onclick={confirmDelete}
          disabled={isDeleting}
          class="px-4 py-2 bg-red-600 hover:bg-red-700 text-white rounded-lg transition-colors font-medium disabled:opacity-50 disabled:cursor-not-allowed flex items-center space-x-2"
        >
          {#if isDeleting}
            <div class="animate-spin w-4 h-4 border-2 border-white border-t-transparent rounded-full"></div>
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
