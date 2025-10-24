<script lang="ts">
  import { onMount } from 'svelte';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
  import { formatRelativeTime, isToday, isValidDate } from '$lib/utils/dateUtils';

  // Reactive state
  let stats = $state({
    totalProperties: 0,
    inProgress: 0,
    completed: 0,
    todayProcessed: 0
  });

  let recentProperties = $state<Property[]>([]);
  let isLoading = $state(true);
  let error = $state<string>('');

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
      const completed = properties.filter((p) => p.completed);
      const inProgress = properties.filter((p) => !p.completed);

      // Calculate today's processed (completed today)
      const todayProcessed = completed.filter(
        (p) => isValidDate(p.updated_at) && isToday(p.updated_at)
      ).length;

      stats = {
        totalProperties: properties.length,
        inProgress: inProgress.length,
        completed: completed.length,
        todayProcessed
      };

      // Get recent properties (last 5)
      recentProperties = properties.slice(0, 5);
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
      await DatabaseService.updatePropertyStatus(propertyId, true);
      await loadDashboardData(); // Refresh data
    } catch (err) {
      console.error('Error updating property status:', err);
    }
  }
</script>

<div class="bg-background-0 min-h-full">
  <!-- Header -->
  <div class="bg-background-50 border-background-200 border-b">
    <div class="px-8 py-6">
      <div class="mx-auto max-w-7xl">
        <h1 class="text-foreground-900 text-2xl font-semibold">Dashboard</h1>
        <p class="text-foreground-600 mt-1 text-sm">Overview of your property management</p>
      </div>
    </div>
  </div>

  <div class="mx-auto max-w-7xl space-y-6 p-8">
    <!-- Error Message -->
    {#if error}
      <div class="rounded-lg border border-red-300 bg-red-50 px-4 py-3">
        <p class="text-sm text-red-800">{error}</p>
      </div>
    {/if}

    <!-- Loading State -->
    {#if isLoading}
      <div class="flex items-center gap-2 text-sm text-foreground-500">
        <div
          class="h-4 w-4 animate-spin rounded-full border-2 border-foreground-300 border-t-transparent"
        ></div>
        <span>Loading data...</span>
      </div>
    {/if}

    <!-- Stats Grid -->
    <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-4">
      <!-- Total Properties -->
      <div class="bg-background-50 border-background-200 rounded-lg border p-5">
        <p class="text-foreground-600 text-xs font-medium uppercase tracking-wide">Total</p>
        <p class="text-foreground-900 mt-2 text-3xl font-semibold">{stats.totalProperties}</p>
      </div>

      <!-- In Progress -->
      <div class="bg-background-50 border-background-200 rounded-lg border p-5">
        <p class="text-foreground-600 text-xs font-medium uppercase tracking-wide">In Progress</p>
        <p class="mt-2 text-3xl font-semibold text-orange-600">{stats.inProgress}</p>
      </div>

      <!-- Completed -->
      <div class="bg-background-50 border-background-200 rounded-lg border p-5">
        <p class="text-foreground-600 text-xs font-medium uppercase tracking-wide">Completed</p>
        <p class="mt-2 text-3xl font-semibold text-green-600">{stats.completed}</p>
      </div>

      <!-- Today's Work -->
      <div class="bg-background-50 border-background-200 rounded-lg border p-5">
        <p class="text-foreground-600 text-xs font-medium uppercase tracking-wide">Today</p>
        <p class="text-accent-600 mt-2 text-3xl font-semibold">{stats.todayProcessed}</p>
      </div>
    </div>

    <!-- Recent Properties -->
    <div class="bg-background-50 border-background-200 rounded-lg border">
      <div class="border-background-200 flex items-center justify-between border-b px-5 py-4">
        <h2 class="text-foreground-900 text-base font-semibold">Recent Properties</h2>
        <a
          href="/properties"
          class="text-accent-600 hover:text-accent-700 text-sm font-medium transition-colors"
        >
          View all
        </a>
      </div>

      <div class="p-5">
        {#if recentProperties.length === 0 && !isLoading}
          <div class="py-8 text-center">
            <p class="text-foreground-500 mb-4 text-sm">No properties yet</p>
            <a
              href="/properties"
              class="bg-accent-500 hover:bg-accent-600 inline-flex rounded-md px-4 py-2 text-sm font-medium text-white transition-colors"
            >
              Add Property
            </a>
          </div>
        {:else}
          <div class="space-y-2">
            {#each recentProperties as property}
              <a
                href="/properties/{property.id}"
                class="bg-background-100 hover:bg-background-200 border-background-200 flex items-center justify-between rounded-md border px-4 py-3 transition-colors"
              >
                <div class="flex items-center gap-3">
                  <div>
                    <h3 class="text-foreground-900 text-sm font-medium">{property.name}</h3>
                    <p class="text-foreground-500 text-xs">{property.city}</p>
                  </div>
                </div>
                <div class="flex items-center gap-3">
                  {#if property.completed}
                    <span class="rounded bg-green-100 px-2 py-1 text-xs font-medium text-green-700">
                      Completed
                    </span>
                  {:else}
                    <span class="rounded bg-orange-100 px-2 py-1 text-xs font-medium text-orange-700">
                      In Progress
                    </span>
                  {/if}
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
