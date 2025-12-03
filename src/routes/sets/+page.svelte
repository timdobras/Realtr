<script lang="ts">
  import { onMount } from 'svelte';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Set } from '$lib/types/database';
  import SetCard from '$lib/components/SetCard.svelte';
  import SetDetailsModal from '$lib/components/SetDetailsModal.svelte';

  let sets = $state<Set[]>([]);
  let isLoading = $state(true);
  let error = $state('');
  let openFolderError = $state('');

  // Details modal state
  let selectedSet = $state<Set | null>(null);
  let showDetailsModal = $state(false);

  onMount(async () => {
    await loadSets();
  });

  async function loadSets() {
    try {
      isLoading = true;
      error = '';
      sets = await DatabaseService.getSets();
    } catch (err) {
      console.error('Error loading sets:', err);
      error = 'Failed to load sets';
    } finally {
      isLoading = false;
    }
  }

  async function openSetsFolder() {
    try {
      openFolderError = '';
      const result = await DatabaseService.openSetsFolder();
      if (!result.success) {
        openFolderError = result.error || 'Failed to open folder';
        setTimeout(() => (openFolderError = ''), 3000);
      }
    } catch (err) {
      console.error('Failed to open sets folder:', err);
      openFolderError = 'Failed to open folder';
      setTimeout(() => (openFolderError = ''), 3000);
    }
  }

  function handleViewDetails(set: Set) {
    selectedSet = set;
    showDetailsModal = true;
  }

  function handleCloseDetails() {
    showDetailsModal = false;
    selectedSet = null;
  }

  async function handleSetDeleted() {
    await loadSets();
  }
</script>

<div class="bg-background-0 min-h-full">
  <!-- Header -->
  <div class="bg-background-50 border-background-200 border-b">
    <div class="px-6 py-4">
      <div class="mx-auto">
        <div class="flex items-center justify-between">
          <div>
            <h1 class="text-foreground-900 text-xl font-semibold">Sets</h1>
            <p class="text-foreground-600 mt-0.5 text-sm">Completed property archives</p>
          </div>
          <button
            onclick={openSetsFolder}
            class="bg-background-200 hover:bg-background-300 text-foreground-700 flex items-center gap-2 px-4 py-2 text-sm font-medium transition-colors"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
              />
            </svg>
            <span>Open Folder</span>
          </button>
        </div>
      </div>
    </div>
  </div>

  <div class="mx-auto space-y-4 p-6">
    <!-- Open Folder Error -->
    {#if openFolderError}
      <div class="border border-red-200 bg-red-50 px-3 py-2">
        <p class="text-sm text-red-600">{openFolderError}</p>
      </div>
    {/if}

    <!-- Error Message -->
    {#if error}
      <div class="border-background-300 bg-background-100 border px-3 py-2">
        <p class="text-foreground-900 text-sm">{error}</p>
      </div>
    {/if}

    <!-- Sets Grid -->
    {#if isLoading}
      <div class="text-foreground-500 flex items-center gap-2 text-sm">
        <div class="border-foreground-300 h-4 w-4 animate-spin border-2 border-t-transparent"></div>
        <span>Loading sets...</span>
      </div>
    {:else if sets.length === 0}
      <div class="bg-background-50 border-background-200 border py-10 text-center">
        <svg
          class="text-foreground-300 mx-auto mb-3 h-10 w-10"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"
          />
        </svg>
        <p class="text-foreground-500 mb-1 text-sm">No sets yet</p>
        <p class="text-foreground-400 text-xs">
          Complete a set from the Dashboard to create your first archive
        </p>
      </div>
    {:else}
      <!-- Stats Bar -->
      <div class="bg-background-50 border-background-200 flex items-center gap-4 border px-4 py-2">
        <span class="text-foreground-500 text-sm">
          <span class="text-foreground-900 font-medium">{sets.length}</span>
          {sets.length === 1 ? 'set' : 'sets'}
        </span>
        <span class="text-foreground-300">|</span>
        <span class="text-foreground-500 text-sm">
          <span class="text-foreground-900 font-medium"
            >{sets.reduce((sum, s) => sum + s.property_count, 0)}</span
          >
          total properties
        </span>
      </div>

      <!-- Sets Grid -->
      <div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
        {#each sets as set}
          <SetCard {set} onDelete={handleSetDeleted} onViewDetails={() => handleViewDetails(set)} />
        {/each}
      </div>
    {/if}
  </div>
</div>

<!-- Set Details Modal -->
{#if selectedSet}
  <SetDetailsModal set={selectedSet} bind:open={showDetailsModal} onClose={handleCloseDetails} />
{/if}
