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

<div class="space-y-6 p-6">
  <!-- Error Message -->
  {#if error}
    <div class="rounded-lg border border-red-200 bg-red-50 p-4">
      <div class="flex items-center space-x-2">
        <span class="text-red-600">❌</span>
        <p class="font-medium text-red-800">{error}</p>
      </div>
    </div>
  {/if}

  <!-- Welcome Section -->
  <div class="rounded-lg bg-gradient-to-r from-blue-600 to-blue-700 p-8 text-white">
    <h1 class="mb-2 text-3xl font-bold">Welcome back!</h1>
    <p class="text-blue-100">Ready to process some amazing real estate photos?</p>
    {#if isLoading}
      <div class="mt-4 flex items-center space-x-2">
        <div
          class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
        ></div>
        <span class="text-blue-100">Loading dashboard data...</span>
      </div>
    {/if}
  </div>

  <!-- Stats Grid -->
  <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
    <div class="bg-background-100 border-background-300 rounded-lg border p-6 shadow-sm">
      <div class="flex items-center justify-between">
        <div>
          <p class="text-foreground-600 text-sm">Total Properties</p>
          <p class="text-foreground-900 text-3xl font-bold">{stats.totalProperties}</p>
        </div>
        <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-blue-100">
          <span class="text-2xl">🏢</span>
        </div>
      </div>
    </div>

    <div class="bg-background-100 border-background-300 rounded-lg border p-6 shadow-sm">
      <div class="flex items-center justify-between">
        <div>
          <p class="text-foreground-600 text-sm">In Progress</p>
          <p class="text-3xl font-bold text-orange-600">{stats.inProgress}</p>
        </div>
        <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-orange-100">
          <span class="text-2xl">⏳</span>
        </div>
      </div>
    </div>

    <div class="bg-background-100 border-background-300 rounded-lg border p-6 shadow-sm">
      <div class="flex items-center justify-between">
        <div>
          <p class="text-foreground-600 text-sm">Completed</p>
          <p class="text-3xl font-bold text-green-600">{stats.completed}</p>
        </div>
        <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-green-100">
          <span class="text-2xl">✅</span>
        </div>
      </div>
    </div>

    <div class="bg-background-100 border-background-300 rounded-lg border p-6 shadow-sm">
      <div class="flex items-center justify-between">
        <div>
          <p class="text-foreground-600 text-sm">Today's Work</p>
          <p class="text-3xl font-bold text-purple-600">{stats.todayProcessed}</p>
        </div>
        <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-purple-100">
          <span class="text-2xl">📸</span>
        </div>
      </div>
    </div>
  </div>

  <!-- Recent Properties -->
  <div class="bg-background-100 border-background-300 rounded-lg border shadow-sm">
    <div class="border-background-300 flex items-center justify-between border-b p-6">
      <h2 class="text-foreground-900 text-xl font-semibold">Recent Properties</h2>
      <a href="/properties" class="text-sm font-medium text-blue-600 hover:text-blue-700">
        View all →
      </a>
    </div>
    <div class="p-6">
      {#if isLoading}
        <div class="space-y-4">
          {#each Array(3) as _}
            <div class="animate-pulse">
              <div class="flex items-center space-x-4">
                <div class="h-10 w-10 rounded-lg bg-gray-300"></div>
                <div class="flex-1 space-y-2">
                  <div class="h-4 w-3/4 rounded bg-gray-300"></div>
                  <div class="h-3 w-1/2 rounded bg-gray-300"></div>
                </div>
              </div>
            </div>
          {/each}
        </div>
      {:else if recentProperties.length === 0}
        <div class="py-12 text-center">
          <span class="text-6xl">📁</span>
          <h3 class="text-foreground-900 mt-4 text-lg font-medium">No properties yet</h3>
          <p class="text-foreground-500 mt-2">Start by adding your first property</p>
          <a href="/properties" class="btn-primary mt-4 inline-block"> Add Property </a>
        </div>
      {:else}
        <div class="space-y-4">
          {#each recentProperties as property}
            <div
              class="bg-background-200 hover:bg-background-300 flex items-center justify-between rounded-lg p-4 transition-colors"
            >
              <div class="flex items-center space-x-4">
                <div class="flex h-10 w-10 items-center justify-center rounded-lg bg-blue-100">
                  <span class="text-blue-600">🏠</span>
                </div>
                <div>
                  <h3 class="text-foreground-900 font-medium">{property.name}</h3>
                  <p class="text-foreground-500 text-sm">{property.city}</p>
                </div>
              </div>
              <div class="flex items-center space-x-4">
                {#if !property.completed}
                  <button
                    onclick={() => markAsCompleted(property.id!)}
                    class="rounded-full bg-orange-100 px-2 py-1 text-xs text-orange-700 transition-colors hover:bg-orange-200"
                  >
                    Mark Complete
                  </button>
                {:else}
                  <span
                    class="rounded-full bg-green-100 px-3 py-1 text-xs font-medium text-green-700"
                  >
                    Completed
                  </span>
                {/if}
                <span class="text-foreground-500 text-sm">{formatDate(property.updated_at)}</span>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>
