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
    <div class="px-8 py-6">
      <div class="mx-auto max-w-7xl">
        <div class="flex items-center justify-between">
          <div>
            <h1 class="text-foreground-900 text-2xl font-semibold">Properties</h1>
            <p class="text-foreground-600 mt-1 text-sm">Manage your photo projects</p>
          </div>
          <button
            onclick={() => (showAddModal = true)}
            class="bg-accent-500 hover:bg-accent-600 flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium text-white transition-colors"
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

  <div class="mx-auto max-w-7xl space-y-6 p-8">
    <!-- Filters -->
    <div class="bg-background-50 border-background-200 rounded-lg border p-5">
      <div class="mb-4 flex items-center justify-between">
        <h2 class="text-foreground-900 text-sm font-semibold">Filters</h2>

        {#if searchQuery || statusFilter !== 'all' || cityFilter}
          <button
            onclick={clearAllFilters}
            class="text-foreground-600 hover:text-foreground-900 text-xs font-medium transition-colors"
          >
            Clear
          </button>
        {/if}
      </div>

      <div class="grid grid-cols-1 gap-4 md:grid-cols-4">
        <!-- Search -->
        <div>
          <label class="text-foreground-700 mb-1.5 block text-xs font-medium">Search</label>
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Search..."
            class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full rounded-md border px-3 py-2 text-sm transition-colors focus:ring-1 focus:outline-none"
          />
        </div>

        <!-- Status Filter -->
        <div>
          <label class="text-foreground-700 mb-1.5 block text-xs font-medium">Status</label>
          <select
            bind:value={statusFilter}
            class="bg-background-100 border-background-300 text-foreground-900 focus:ring-accent-500 focus:border-accent-500 w-full rounded-md border px-3 py-2 text-sm transition-colors focus:ring-1 focus:outline-none"
          >
            <option value="all">All</option>
            <option value="in_progress">In Progress</option>
            <option value="completed">Completed</option>
          </select>
        </div>

        <!-- City Filter -->
        <div>
          <label class="text-foreground-700 mb-1.5 block text-xs font-medium">City</label>
          <select
            bind:value={cityFilter}
            class="bg-background-100 border-background-300 text-foreground-900 focus:ring-accent-500 focus:border-accent-500 w-full rounded-md border px-3 py-2 text-sm transition-colors focus:ring-1 focus:outline-none"
          >
            <option value="">All Cities</option>
            {#each cities as city}
              <option value={city}>{city}</option>
            {/each}
          </select>
        </div>

        <!-- Results Count -->
        <div class="flex items-end">
          <div class="bg-background-100 border-background-200 w-full rounded-md border px-3 py-2">
            <div class="text-foreground-600 text-xs">
              <span class="text-accent-600 font-semibold">{filteredProperties.length}</span>
              <span class="text-foreground-500"> of </span>
              <span class="text-foreground-700 font-semibold">{properties.length}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Error Message -->
    {#if error}
      <div class="rounded-lg border border-red-300 bg-red-50 px-4 py-3">
        <p class="text-sm text-red-800">{error}</p>
      </div>
    {/if}

    <!-- Properties Grid -->
    {#if isLoading}
      <div class="flex items-center gap-2 text-sm text-foreground-500">
        <div
          class="h-4 w-4 animate-spin rounded-full border-2 border-foreground-300 border-t-transparent"
        ></div>
        <span>Loading properties...</span>
      </div>
    {:else if filteredProperties.length === 0}
      <div class="bg-background-50 border-background-200 rounded-lg border py-12 text-center">
        <p class="text-foreground-500 mb-4 text-sm">
          {properties.length === 0 ? 'No properties yet' : 'No properties match your filters'}
        </p>

        {#if properties.length === 0}
          <button
            onclick={() => (showAddModal = true)}
            class="bg-accent-500 hover:bg-accent-600 inline-flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium text-white transition-colors"
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
        {:else}
          <button
            onclick={clearAllFilters}
            class="bg-background-200 text-foreground-700 hover:bg-background-300 inline-flex rounded-md px-4 py-2 text-sm font-medium transition-colors"
          >
            Clear Filters
          </button>
        {/if}
      </div>
    {:else}
      <!-- Properties Grid -->
      <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
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
