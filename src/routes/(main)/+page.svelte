<script lang="ts">
  import { onMount } from 'svelte';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property, CompleteSetResult } from '$lib/types/database';
  import { formatRelativeTime, isToday, isValidDate } from '$lib/utils/dateUtils';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';

  // Reactive state
  let stats = $state({
    totalProperties: 0,
    inProgress: 0,
    completed: 0,
    todayProcessed: 0,
    doneWithCode: 0,
    doneWithoutCode: 0
  });

  let recentProperties = $state<Property[]>([]);
  let isLoading = $state(true);
  let error = $state<string>('');

  // Complete Set state
  let isCompletingSet = $state(false);
  let showCompleteSetConfirm = $state(false);
  let completeSetResult = $state<CompleteSetResult | null>(null);
  let showCompleteSetResult = $state(false);
  let completeSetError = $state<string>('');

  onMount(async () => {
    await loadDashboardData();
  });

  async function loadDashboardData() {
    try {
      isLoading = true;
      error = '';

      // Load all properties
      const properties = await DatabaseService.getProperties();

      // Calculate stats
      const completed = properties.filter((p) => p.status === 'DONE' || p.status === 'ARCHIVE');
      const inProgress = properties.filter((p) => p.status === 'NEW' || p.status === 'NOT_FOUND');

      // Calculate DONE properties with and without codes
      const doneProperties = properties.filter((p) => p.status === 'DONE');
      const doneWithCode = doneProperties.filter((p) => p.code && p.code.trim() !== '').length;
      const doneWithoutCode = doneProperties.length - doneWithCode;

      // Calculate today's processed (completed today)
      const todayProcessed = completed.filter(
        (p) => isValidDate(p.updated_at) && isToday(p.updated_at)
      ).length;

      stats = {
        totalProperties: properties.length,
        inProgress: inProgress.length,
        completed: completed.length,
        todayProcessed,
        doneWithCode,
        doneWithoutCode
      };

      // Get recent properties (last 5)
      recentProperties = properties.slice(0, 25);
    } catch (err) {
      console.error('Error loading dashboard data:', err);
      error = 'Failed to load dashboard data';
    } finally {
      isLoading = false;
    }
  }

  function formatDate(timestamp: number): string {
    if (!isValidDate(timestamp)) {
      return 'Unknown date';
    }
    return formatRelativeTime(timestamp);
  }

  async function markAsCompleted(propertyId: number) {
    try {
      const result = await DatabaseService.updatePropertyStatus(propertyId, 'DONE');
      if (!result.success) {
        error = `Failed to update status: ${result.error}`;
        return;
      }
      await loadDashboardData(); // Refresh data
    } catch (err) {
      console.error('Error updating property status:', err);
      error = `Failed to update status: ${err}`;
    }
  }

  function promptCompleteSet() {
    showCompleteSetConfirm = true;
  }

  async function doCompleteSet() {
    showCompleteSetConfirm = false;
    try {
      isCompletingSet = true;
      completeSetError = '';
      completeSetResult = await DatabaseService.completeSet();
      showCompleteSetResult = true;
      await loadDashboardData(); // Refresh data
    } catch (err) {
      console.error('Error completing set:', err);
      completeSetError = err instanceof Error ? err.message : String(err);
    } finally {
      isCompletingSet = false;
    }
  }
</script>

<div class="bg-background-0 min-h-full">
  <!-- Header -->
  <div class="bg-background-50 border-background-200 border-b">
    <div class="px-6 py-4">
      <div class="mx-auto max-w-7xl">
        <h1 class="text-foreground-900 text-xl font-semibold">Dashboard</h1>
        <p class="text-foreground-600 mt-0.5 text-sm">Overview of your property management</p>
      </div>
    </div>
  </div>

  <div class="mx-auto max-w-7xl space-y-5 p-6">
    <!-- Error Message -->
    {#if error}
      <div class="border-background-300 bg-background-100 border px-3 py-2">
        <p class="text-foreground-900 text-sm">{error}</p>
      </div>
    {/if}

    <!-- Loading State -->
    {#if isLoading}
      <div class="text-foreground-500 flex items-center gap-2 text-sm">
        <div class="border-foreground-300 h-4 w-4 animate-spin border-2 border-t-transparent"></div>
        <span>Loading data...</span>
      </div>
    {/if}

    <!-- Stats Grid -->
    <div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
      <!-- Total Properties -->
      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">Total</p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">{stats.totalProperties}</p>
      </div>

      <!-- In Progress -->
      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">In Progress</p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">{stats.inProgress}</p>
      </div>

      <!-- Completed -->
      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">Completed</p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">{stats.completed}</p>
      </div>

      <!-- Today's Work -->
      <div class="bg-background-50 border-background-200 border p-4">
        <p class="text-foreground-600 text-xs font-medium tracking-wide uppercase">Today</p>
        <p class="text-foreground-900 mt-1 text-2xl font-semibold">{stats.todayProcessed}</p>
      </div>
    </div>

    <!-- Complete Set Action -->
    {#if stats.doneWithCode > 0 || stats.doneWithoutCode > 0}
      <div class="bg-background-50 border-background-200 border p-4">
        <div class="flex items-center justify-between">
          <div>
            <h2 class="text-foreground-900 text-sm font-semibold">Complete Set</h2>
            <p class="text-foreground-600 mt-0.5 text-xs">
              {stats.doneWithCode} properties with code will be zipped and archived.
              {#if stats.doneWithoutCode > 0}
                {stats.doneWithoutCode} without code will be moved to Not Found.
              {/if}
            </p>
          </div>
          <button
            onclick={promptCompleteSet}
            disabled={isCompletingSet || stats.doneWithCode === 0}
            class="bg-accent-500 hover:bg-accent-600 flex items-center gap-2 px-4 py-2 text-sm font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            {#if isCompletingSet}
              <div
                class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
              ></div>
              <span>Processing...</span>
            {:else}
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"
                />
              </svg>
              <span>Complete Set</span>
            {/if}
          </button>
        </div>
        {#if completeSetError}
          <div class="mt-3 border border-red-300 bg-red-50 px-3 py-2">
            <p class="text-sm text-red-800">{completeSetError}</p>
          </div>
        {/if}
      </div>
    {/if}

    <!-- Recent Properties -->
    <div class="bg-background-50 border-background-200 border">
      <div class="border-background-200 flex items-center justify-between border-b px-4 py-3">
        <h2 class="text-foreground-900 text-sm font-semibold">Recent Properties</h2>
        <a
          href="/properties"
          class="text-accent-600 hover:text-accent-700 text-sm transition-colors"
        >
          View all
        </a>
      </div>

      <div class="">
        {#if recentProperties.length === 0 && !isLoading}
          <div class="py-6 text-center">
            <p class="text-foreground-500 mb-3 text-sm">No properties yet</p>
            <a
              href="/properties"
              class="bg-accent-500 hover:bg-accent-600 inline-flex px-4 py-2 text-sm font-medium text-white transition-colors"
            >
              Add Property
            </a>
          </div>
        {:else}
          <div class="flex flex-col">
            {#each recentProperties as property}
              <a
                href="/properties/{property.id}"
                class="odd:bg-background-100 hover:bg-background-200 flex items-center justify-between px-3 py-2 transition-colors"
              >
                <div class="flex items-center gap-3">
                  <div>
                    <h3 class="text-foreground-900 text-sm font-medium">{property.name}</h3>
                    <p class="text-foreground-500 text-xs">{property.city}</p>
                  </div>
                </div>
                <div class="flex items-center gap-3">
                  <span
                    class="inline-flex items-center gap-1 text-xs {property.status === 'DONE'
                      ? ' text-green-300'
                      : property.status === 'ARCHIVE'
                        ? ' text-gray-300'
                        : property.status === 'NOT_FOUND'
                          ? ' text-yellow-300'
                          : ' text-blue-300'}"
                  >
                    {#if property.status === 'DONE'}
                      <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M5 13l4 4L19 7"
                        />
                      </svg>
                      <span>Done</span>
                    {:else if property.status === 'ARCHIVE'}
                      <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"
                        />
                      </svg>
                      <span>Archived</span>
                    {:else if property.status === 'NOT_FOUND'}
                      <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
                        />
                      </svg>
                      <span>Not Found</span>
                    {:else}
                      <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
                        />
                      </svg>
                      <span>New</span>
                    {/if}
                  </span>
                  <span class="text-foreground-500 text-xs">{formatDate(property.updated_at)}</span>
                </div>
              </a>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<!-- Complete Set Confirmation Dialog -->
<ConfirmDialog
  bind:open={showCompleteSetConfirm}
  title="Complete Set"
  message="This will create a ZIP archive of {stats.doneWithCode} properties with codes and move them to Archive.{stats.doneWithoutCode >
  0
    ? ` ${stats.doneWithoutCode} properties without codes will be moved to Not Found.`
    : ''} Continue?"
  confirmText="Complete Set"
  destructive={false}
  onConfirm={doCompleteSet}
  onCancel={() => (showCompleteSetConfirm = false)}
/>

<!-- Complete Set Result Modal -->
{#if showCompleteSetResult && completeSetResult}
  <div class="bg-opacity-50 fixed inset-0 z-50 flex items-center justify-center bg-black">
    <div class="bg-background-50 border-background-200 mx-4 w-full max-w-md border">
      <div class="border-background-200 flex items-center justify-between border-b p-4">
        <h3 class="text-foreground-900 text-lg font-semibold">Set Created</h3>
        <button
          onclick={() => (showCompleteSetResult = false)}
          class="text-foreground-400 hover:text-foreground-600 p-1"
        >
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

      <div class="space-y-4 p-4">
        <div class="bg-background-100 rounded p-3">
          <p class="text-foreground-600 text-xs font-medium uppercase">Set Name</p>
          <p class="text-foreground-900 mt-1 text-sm font-medium">{completeSetResult.setName}</p>
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div class="bg-background-100 rounded p-3 text-center">
            <p class="text-foreground-900 text-xl font-semibold">
              {completeSetResult.propertiesArchived}
            </p>
            <p class="text-foreground-600 text-xs">Archived</p>
          </div>
          <div class="bg-background-100 rounded p-3 text-center">
            <p class="text-foreground-900 text-xl font-semibold">
              {completeSetResult.propertiesMovedToNotFound}
            </p>
            <p class="text-foreground-600 text-xs">Moved to Not Found</p>
          </div>
        </div>

        <div class="bg-background-100 rounded p-3">
          <p class="text-foreground-600 text-xs font-medium uppercase">ZIP Location</p>
          <p class="text-foreground-900 mt-1 font-mono text-xs break-all">
            {completeSetResult.zipPath}
          </p>
        </div>

        <div class="flex justify-end gap-2">
          <a
            href="/sets"
            class="bg-background-200 text-foreground-700 hover:bg-background-300 px-4 py-2 text-sm font-medium transition-colors"
          >
            View Sets
          </a>
          <button
            onclick={() => (showCompleteSetResult = false)}
            class="bg-accent-500 hover:bg-accent-600 px-4 py-2 text-sm font-medium text-white transition-colors"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
