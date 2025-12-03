<script lang="ts">
  import { onMount } from 'svelte';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Set, SetProperty } from '$lib/types/database';
  import { isValidDate } from '$lib/utils/dateUtils';

  interface Props {
    set: Set;
    open: boolean;
    onClose: () => void;
  }

  let { set, open = $bindable(), onClose }: Props = $props();

  let properties = $state<SetProperty[]>([]);
  let isLoading = $state(true);
  let error = $state('');
  let copySuccess = $state(false);

  $effect(() => {
    if (open && set.id) {
      loadSetProperties();
    }
  });

  async function loadSetProperties() {
    try {
      isLoading = true;
      error = '';
      properties = await DatabaseService.getSetProperties(set.id!);
    } catch (err) {
      console.error('Failed to load set properties:', err);
      error = 'Failed to load properties';
    } finally {
      isLoading = false;
    }
  }

  function formatSetDate(timestamp: number): string {
    if (!isValidDate(timestamp)) {
      return 'Unknown';
    }
    const date = new Date(timestamp);
    return date.toLocaleDateString('en-US', {
      weekday: 'long',
      year: 'numeric',
      month: 'long',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  async function copyZipPath() {
    try {
      await navigator.clipboard.writeText(set.zip_path);
      copySuccess = true;
      setTimeout(() => (copySuccess = false), 2000);
    } catch (err) {
      console.error('Failed to copy path:', err);
    }
  }

  function handleClose() {
    open = false;
    onClose();
  }
</script>

{#if open}
  <div class="bg-opacity-50 fixed inset-0 z-50 flex items-center justify-center bg-black">
    <div
      class="bg-background-50 border-background-200 mx-4 max-h-[80vh] w-full max-w-2xl overflow-hidden border"
    >
      <!-- Header -->
      <div class="border-background-200 flex items-center justify-between border-b p-4">
        <h3 class="text-foreground-900 text-lg font-semibold">Set Details</h3>
        <button onclick={handleClose} class="text-foreground-400 hover:text-foreground-600 p-1">
          <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>

      <!-- Content -->
      <div class="max-h-[calc(80vh-120px)] overflow-y-auto p-4">
        <!-- Set Info -->
        <div class="mb-6 space-y-3">
          <div class="bg-background-100 rounded p-3">
            <p class="text-foreground-600 text-xs font-medium uppercase">Name</p>
            <p class="text-foreground-900 mt-1 font-medium">{set.name}</p>
          </div>

          <div class="bg-background-100 rounded p-3">
            <p class="text-foreground-600 text-xs font-medium uppercase">Created</p>
            <p class="text-foreground-900 mt-1 text-sm">{formatSetDate(set.created_at)}</p>
          </div>

          <div class="bg-background-100 rounded p-3">
            <div class="flex items-center justify-between">
              <div>
                <p class="text-foreground-600 text-xs font-medium uppercase">ZIP Location</p>
                <p class="text-foreground-900 mt-1 font-mono text-xs break-all">{set.zip_path}</p>
              </div>
              <button
                onclick={copyZipPath}
                class="text-foreground-500 hover:text-foreground-700 ml-2 flex-shrink-0 p-1"
                title="Copy path"
              >
                {#if copySuccess}
                  <svg
                    class="h-4 w-4 text-green-500"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M5 13l4 4L19 7"
                    />
                  </svg>
                {:else}
                  <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                    />
                  </svg>
                {/if}
              </button>
            </div>
          </div>
        </div>

        <!-- Properties List -->
        <div>
          <h4 class="text-foreground-900 mb-3 text-sm font-semibold">
            Included Properties ({set.property_count})
          </h4>

          {#if isLoading}
            <div class="text-foreground-500 flex items-center gap-2 py-4 text-sm">
              <div
                class="border-foreground-300 h-4 w-4 animate-spin rounded-full border-2 border-t-transparent"
              ></div>
              <span>Loading properties...</span>
            </div>
          {:else if error}
            <div class="border border-red-200 bg-red-50 px-3 py-2">
              <p class="text-sm text-red-600">{error}</p>
            </div>
          {:else if properties.length === 0}
            <div class="bg-background-100 py-4 text-center">
              <p class="text-foreground-500 text-sm">No properties found</p>
            </div>
          {:else}
            <div class="border-background-200 divide-background-200 divide-y border">
              {#each properties as property}
                <div class="flex items-center justify-between px-3 py-2">
                  <div>
                    <p class="text-foreground-900 text-sm font-medium">{property.propertyName}</p>
                    <p class="text-foreground-500 text-xs">{property.propertyCity}</p>
                  </div>
                  {#if property.propertyCode}
                    <span
                      class="bg-accent-100 text-accent-700 rounded px-2 py-0.5 text-xs font-medium"
                    >
                      {property.propertyCode}
                    </span>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      <!-- Footer -->
      <div class="border-background-200 flex justify-end border-t p-4">
        <button
          onclick={handleClose}
          class="bg-accent-500 hover:bg-accent-600 px-4 py-2 text-sm font-medium text-white transition-colors"
        >
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
