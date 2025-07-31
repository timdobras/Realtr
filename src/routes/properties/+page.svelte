<script lang="ts">
  import { onMount } from 'svelte';
  import { DatabaseService } from '$lib/services/databaseService';
  import PropertyCard from '$lib/components/PropertyCard.svelte';
  import AddPropertyModal from '$lib/components/AddPropertyModal.svelte';
  import type { Property } from '$lib/types/database';

  let properties = $state<Property[]>([]);
  let filteredProperties = $state<Property[]>([]);
  let isLoading = $state(true);
  let error = $state<string>('');
  let showAddModal = $state(false);

  // Filters
  let searchQuery = $state('');
  let statusFilter = $state<'all' | 'completed' | 'in_progress'>('all');
  let cityFilter = $state('');

  // Get unique cities for filter
  let cities = $derived(Array.from(new Set(properties.map((p) => p.city))).sort());

  onMount(async () => {
    await loadProperties();
  });

  async function loadProperties() {
    try {
      isLoading = true;
      error = '';
      properties = await DatabaseService.getProperties();
      applyFilters();
    } catch (err) {
      console.error('Error loading properties:', err);
      error = 'Failed to load properties';
    } finally {
      isLoading = false;
    }
  }

  function applyFilters() {
    filteredProperties = properties.filter((property) => {
      // Text search
      const matchesSearch =
        searchQuery === '' ||
        property.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        property.city.toLowerCase().includes(searchQuery.toLowerCase());

      // Status filter
      const matchesStatus =
        statusFilter === 'all' ||
        (statusFilter === 'completed' && property.completed) ||
        (statusFilter === 'in_progress' && !property.completed);

      // City filter
      const matchesCity = cityFilter === '' || property.city === cityFilter;

      return matchesSearch && matchesStatus && matchesCity;
    });
  }

  // Watch for filter changes
  $effect(() => {
    searchQuery;
    statusFilter;
    cityFilter; // Dependencies
    applyFilters();
  });

  async function onPropertyAdded() {
    showAddModal = false;
    await loadProperties();
  }

  async function onPropertyUpdated() {
    await loadProperties();
  }

  async function onPropertyDeleted() {
    await loadProperties();
  }

  function clearAllFilters() {
    searchQuery = '';
    statusFilter = 'all';
    cityFilter = '';
  }
</script>

<div class="bg-background-0 min-h-full">
  <!-- Header -->
  <div class="bg-background-50 border-background-200 border-b">
    <div class="px-6 py-8">
      <div class="mx-auto max-w-7xl">
        <div class="flex items-center justify-between">
          <div>
            <h1 class="text-foreground-900 text-3xl font-bold">Properties</h1>
            <p class="text-foreground-600 mt-2">Manage your real estate photo projects</p>
          </div>
          <button
            onclick={() => (showAddModal = true)}
            class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 4v16m8-8H4"
              />
            </svg>
            <span>Add Property</span>
          </button>
        </div>
      </div>
    </div>
  </div>

  <div class="mx-auto max-w-7xl space-y-8 p-6">
    <!-- Filters -->
    <div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
      <div class="mb-6 flex items-center justify-between">
        <div class="flex items-center space-x-3">
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
                d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z"
              />
            </svg>
          </div>
          <div>
            <h2 class="text-foreground-900 text-lg font-semibold">Filter Properties</h2>
            <p class="text-foreground-600 text-sm">Narrow down your search results</p>
          </div>
        </div>

        {#if searchQuery || statusFilter !== 'all' || cityFilter}
          <button
            onclick={clearAllFilters}
            class="text-foreground-600 hover:text-foreground-900 flex items-center space-x-2 px-3 py-1.5 text-sm font-medium transition-colors"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
            <span>Clear filters</span>
          </button>
        {/if}
      </div>

      <div class="grid grid-cols-1 gap-4 md:grid-cols-4">
        <!-- Search -->
        <div>
          <label class="text-foreground-700 mb-2 block text-sm font-medium">Search</label>
          <div class="relative">
            <div class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3">
              <svg
                class="text-foreground-400 h-4 w-4"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                />
              </svg>
            </div>
            <input
              type="text"
              bind:value={searchQuery}
              placeholder="Search properties..."
              class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full rounded-lg border py-2 pr-3 pl-10 transition-colors focus:ring-2 focus:outline-none"
            />
          </div>
        </div>

        <!-- Status Filter -->
        <div>
          <label class="text-foreground-700 mb-2 block text-sm font-medium">Status</label>
          <div class="relative">
            <select
              bind:value={statusFilter}
              class="bg-background-100 border-background-300 text-foreground-900 focus:ring-accent-500 focus:border-accent-500 w-full appearance-none rounded-lg border px-3 py-2 pr-10 transition-colors focus:ring-2 focus:outline-none"
            >
              <option value="all">All Properties</option>
              <option value="in_progress">In Progress</option>
              <option value="completed">Completed</option>
            </select>
            <div class="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-3">
              <svg
                class="text-foreground-400 h-4 w-4"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M19 9l-7 7-7-7"
                />
              </svg>
            </div>
          </div>
        </div>

        <!-- City Filter -->
        <div>
          <label class="text-foreground-700 mb-2 block text-sm font-medium">City</label>
          <div class="relative">
            <select
              bind:value={cityFilter}
              class="bg-background-100 border-background-300 text-foreground-900 focus:ring-accent-500 focus:border-accent-500 w-full appearance-none rounded-lg border px-3 py-2 pr-10 transition-colors focus:ring-2 focus:outline-none"
            >
              <option value="">All Cities</option>
              {#each cities as city}
                <option value={city}>{city}</option>
              {/each}
            </select>
            <div class="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-3">
              <svg
                class="text-foreground-400 h-4 w-4"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M19 9l-7 7-7-7"
                />
              </svg>
            </div>
          </div>
        </div>

        <!-- Results Count -->
        <div class="flex items-end">
          <div class="bg-background-100 border-background-200 w-full rounded-lg border px-4 py-2">
            <div class="text-foreground-600 text-sm font-medium">
              <span class="text-accent-600 font-bold">{filteredProperties.length}</span>
              <span class="text-foreground-400 mx-1">of</span>
              <span class="text-foreground-700 font-bold">{properties.length}</span>
              <div class="text-foreground-500 mt-1 text-xs">properties</div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Error Message -->
    {#if error}
      <div class="rounded-lg border border-red-200 bg-red-50 p-4">
        <div class="flex items-center space-x-3">
          <svg
            class="h-5 w-5 flex-shrink-0 text-red-500"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
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

    <!-- Properties Grid -->
    {#if isLoading}
      <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-3">
        {#each Array(6) as _}
          <div
            class="bg-background-50 border-background-200 animate-pulse rounded-xl border p-6 shadow-sm"
          >
            <div class="space-y-4">
              <div class="flex items-center space-x-3">
                <div class="bg-background-300 h-10 w-10 rounded-lg"></div>
                <div class="flex-1 space-y-2">
                  <div class="bg-background-300 h-4 w-3/4 rounded"></div>
                  <div class="bg-background-300 h-3 w-1/2 rounded"></div>
                </div>
              </div>
              <div class="bg-background-300 h-20 w-full rounded"></div>
              <div class="flex space-x-2">
                <div class="bg-background-300 h-8 w-20 rounded"></div>
                <div class="bg-background-300 h-8 w-16 rounded"></div>
              </div>
            </div>
          </div>
        {/each}
      </div>
    {:else if filteredProperties.length === 0}
      <div class="py-16 text-center">
        <div
          class="bg-background-100 mx-auto mb-6 flex h-24 w-24 items-center justify-center rounded-full"
        >
          {#if properties.length === 0}
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
                d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"
              />
            </svg>
          {:else}
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
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
          {/if}
        </div>

        <h3 class="text-foreground-900 mb-2 text-xl font-semibold">
          {properties.length === 0 ? 'No properties yet' : 'No properties match your filters'}
        </h3>

        <p class="text-foreground-500 mx-auto mb-8 max-w-md">
          {properties.length === 0
            ? 'Start by adding your first property to begin managing your real estate photos'
            : 'Try adjusting your search criteria or clear all filters to see more results'}
        </p>

        <div class="flex items-center justify-center space-x-4">
          {#if properties.length === 0}
            <button
              onclick={() => (showAddModal = true)}
              class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M12 4v16m8-8H4"
                />
              </svg>
              <span>Add Your First Property</span>
            </button>
          {:else}
            <button
              onclick={clearAllFilters}
              class="bg-background-200 text-foreground-700 hover:bg-background-300 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium transition-colors"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
              <span>Clear All Filters</span>
            </button>
            <button
              onclick={() => (showAddModal = true)}
              class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M12 4v16m8-8H4"
                />
              </svg>
              <span>Add Property</span>
            </button>
          {/if}
        </div>
      </div>
    {:else}
      <!-- Properties Count Header -->
      <div class="flex items-center justify-between">
        <div class="flex items-center space-x-3">
          <div class="bg-accent-100 flex h-8 w-8 items-center justify-center rounded-lg">
            <svg
              class="text-accent-600 h-4 w-4"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
              />
            </svg>
          </div>
          <div>
            <h3 class="text-foreground-900 font-semibold">Property Portfolio</h3>
            <p class="text-foreground-500 text-sm">
              {filteredProperties.length}
              {filteredProperties.length === 1 ? 'property' : 'properties'}
              {searchQuery || statusFilter !== 'all' || cityFilter
                ? 'matching your criteria'
                : 'total'}
            </p>
          </div>
        </div>

        <!-- Sort Options (you can expand this later) -->
        <div class="flex items-center space-x-2">
          <span class="text-foreground-600 text-sm font-medium">Sort by:</span>
          <select
            class="bg-background-100 border-background-300 text-foreground-700 focus:ring-accent-500 focus:border-accent-500 rounded-lg border px-3 py-1.5 text-sm focus:ring-2 focus:outline-none"
          >
            <option>Most Recent</option>
            <option>Name (A-Z)</option>
            <option>City</option>
            <option>Status</option>
          </select>
        </div>
      </div>

      <!-- Properties Grid -->
      <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
        {#each filteredProperties as property}
          <PropertyCard {property} onUpdate={onPropertyUpdated} onDelete={onPropertyDeleted} />
        {/each}
      </div>
    {/if}
  </div>
</div>

<!-- Add Property Modal -->
{#if showAddModal}
  <AddPropertyModal onClose={() => (showAddModal = false)} {onPropertyAdded} />
{/if}
