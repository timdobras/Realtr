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
    <div class="px-6 py-8">
      <div class="mx-auto max-w-7xl">
        <h1 class="text-foreground-900 text-3xl font-bold">Dashboard</h1>
        <p class="text-foreground-600 mt-2">Overview of your property management activities</p>
      </div>
    </div>
  </div>

  <div class="mx-auto max-w-7xl space-y-8 p-6">
    <!-- Error Message -->
    {#if error}
      <div class="rounded-lg border border-red-200 bg-red-50 p-4">
        <div class="flex items-center space-x-3">
          <svg class="h-5 w-5 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
          <p class="font-medium text-red-800">{error}</p>
        </div>
      </div>
    {/if}

    <!-- Welcome Section -->
    <div class="bg-accent-500 relative overflow-hidden rounded-xl p-8 text-white">
      <div class="relative z-10">
        <h2 class="mb-2 text-2xl font-bold">Welcome back</h2>
        <p class="text-accent-100 mb-4">Manage your real estate properties efficiently</p>
        {#if isLoading}
          <div class="flex items-center space-x-2">
            <div
              class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
            ></div>
            <span class="text-accent-100">Loading dashboard data...</span>
          </div>
        {/if}
      </div>
      <!-- Background pattern -->
      <div class="absolute top-0 right-0 h-64 w-64 opacity-10">
        <svg class="h-full w-full" fill="currentColor" viewBox="0 0 24 24">
          <path
            d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"
          />
        </svg>
      </div>
    </div>

    <!-- Stats Grid -->
    <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-4">
      <!-- Total Properties -->
      <div
        class="bg-background-50 border-background-200 rounded-xl border p-6 transition-shadow hover:shadow-md"
      >
        <div class="flex items-center justify-between">
          <div>
            <p class="text-foreground-600 text-sm font-medium">Total Properties</p>
            <p class="text-foreground-900 mt-1 text-3xl font-bold">{stats.totalProperties}</p>
          </div>
          <div class="bg-accent-100 flex h-12 w-12 items-center justify-center rounded-lg">
            <svg
              class="text-accent-600 h-6 w-6"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"
              />
            </svg>
          </div>
        </div>
      </div>

      <!-- In Progress -->
      <div
        class="bg-background-50 border-background-200 rounded-xl border p-6 transition-shadow hover:shadow-md"
      >
        <div class="flex items-center justify-between">
          <div>
            <p class="text-foreground-600 text-sm font-medium">In Progress</p>
            <p class="mt-1 text-3xl font-bold text-orange-600">{stats.inProgress}</p>
          </div>
          <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-orange-100">
            <svg
              class="h-6 w-6 text-orange-600"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
          </div>
        </div>
      </div>

      <!-- Completed -->
      <div
        class="bg-background-50 border-background-200 rounded-xl border p-6 transition-shadow hover:shadow-md"
      >
        <div class="flex items-center justify-between">
          <div>
            <p class="text-foreground-600 text-sm font-medium">Completed</p>
            <p class="mt-1 text-3xl font-bold text-green-600">{stats.completed}</p>
          </div>
          <div class="flex h-12 w-12 items-center justify-center rounded-lg bg-green-100">
            <svg
              class="h-6 w-6 text-green-600"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
          </div>
        </div>
      </div>

      <!-- Today's Work -->
      <div
        class="bg-background-50 border-background-200 rounded-xl border p-6 transition-shadow hover:shadow-md"
      >
        <div class="flex items-center justify-between">
          <div>
            <p class="text-foreground-600 text-sm font-medium">Today's Work</p>
            <p class="text-accent-600 mt-1 text-3xl font-bold">{stats.todayProcessed}</p>
          </div>
          <div class="bg-accent-100 flex h-12 w-12 items-center justify-center rounded-lg">
            <svg
              class="text-accent-600 h-6 w-6"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M3 9a2 2 0 012-2h.93a2 2 0 001.664-.89l.812-1.22A2 2 0 0110.07 4h3.86a2 2 0 011.664.89l.812 1.22A2 2 0 0018.07 7H19a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V9z"
              />
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M15 13a3 3 0 11-6 0 3 3 0 016 0z"
              />
            </svg>
          </div>
        </div>
      </div>
    </div>

    <!-- Recent Properties -->
    <div class="bg-background-50 border-background-200 rounded-xl border shadow-sm">
      <div class="border-background-200 flex items-center justify-between border-b p-6">
        <h2 class="text-foreground-900 text-xl font-semibold">Recent Properties</h2>
        <a
          href="/properties"
          class="text-accent-600 hover:text-accent-700 flex items-center space-x-1 text-sm font-medium"
        >
          <span>View all</span>
          <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M9 5l7 7-7 7"
            />
          </svg>
        </a>
      </div>

      <div class="p-6">
        {#if isLoading}
          <div class="space-y-4">
            {#each Array(3) as _}
              <div class="animate-pulse">
                <div class="flex items-center space-x-4">
                  <div class="bg-background-300 h-12 w-12 rounded-lg"></div>
                  <div class="flex-1 space-y-2">
                    <div class="bg-background-300 h-4 w-3/4 rounded"></div>
                    <div class="bg-background-300 h-3 w-1/2 rounded"></div>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {:else if recentProperties.length === 0}
          <div class="py-12 text-center">
            <div
              class="bg-background-100 mx-auto mb-4 flex h-24 w-24 items-center justify-center rounded-full"
            >
              <svg
                class="text-foreground-400 h-12 w-12"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
                />
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M8 5a2 2 0 012-2h4a2 2 0 012 2v6H8V5z"
                />
              </svg>
            </div>
            <h3 class="text-foreground-900 text-lg font-medium">No properties yet</h3>
            <p class="text-foreground-500 mt-2 mb-6">Start by adding your first property</p>
            <a
              href="/properties"
              class="bg-accent-500 hover:bg-accent-600 inline-flex items-center rounded-lg px-4 py-2 font-medium text-white transition-colors"
            >
              <svg class="mr-2 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M12 4v16m8-8H4"
                />
              </svg>
              Add Property
            </a>
          </div>
        {:else}
          <div class="space-y-3">
            {#each recentProperties as property}
              <a
                href="/properties/{property.id}"
                class="bg-background-100 hover:bg-background-200 border-background-200 flex items-center justify-between rounded-lg border p-4 transition-all duration-200"
              >
                <div class="flex items-center space-x-4">
                  <div class="bg-accent-100 flex h-12 w-12 items-center justify-center rounded-lg">
                    <svg
                      class="text-accent-600 h-6 w-6"
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
                  <div>
                    <h3 class="text-foreground-900 font-medium">{property.name}</h3>
                    <p class="text-foreground-500 text-sm">{property.city}</p>
                  </div>
                </div>
                <div class="flex items-center space-x-4">
                  {#if !property.completed}
                    <button
                      onclick={() => markAsCompleted(property.id!)}
                      class="inline-flex items-center rounded-lg bg-orange-100 px-3 py-1.5 text-sm font-medium text-orange-700 transition-colors hover:bg-orange-200"
                    >
                      <svg
                        class="mr-1 h-3 w-3"
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
                      Complete
                    </button>
                  {:else}
                    <span
                      class="inline-flex items-center rounded-lg bg-green-100 px-3 py-1.5 text-sm font-medium text-green-700"
                    >
                      <svg
                        class="mr-1 h-3 w-3"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          stroke-linecap="round"
                          stroke-linejoin="round"
                          stroke-width="2"
                          d="M9 12l2 2 4-4"
                        />
                      </svg>
                      Completed
                    </span>
                  {/if}
                  <span class="text-foreground-500 text-sm">{formatDate(property.updated_at)}</span>
                </div>
              </a>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>
