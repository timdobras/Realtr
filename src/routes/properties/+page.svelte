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
</script>

<div class="space-y-6 p-6">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-foreground-900 text-2xl font-bold">Properties</h1>
      <p class="text-foreground-600 mt-1">Manage your real estate photo projects</p>
    </div>
    <button onclick={() => (showAddModal = true)} class="btn-primary flex items-center space-x-2">
      <span>➕</span>
      <span>Add Property</span>
    </button>
  </div>

  <!-- Filters -->
  <div class="bg-background-100 border-background-300 rounded-lg border p-6 shadow-sm">
    <div class="grid grid-cols-1 gap-4 md:grid-cols-4">
      <!-- Search -->
      <div>
        <label class="text-foreground-700 mb-2 block text-sm font-medium">Search</label>
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Search properties..."
          class="border-background-400 w-full rounded-lg border px-3 py-2 focus:border-blue-500 focus:ring-2 focus:ring-blue-500"
        />
      </div>

      <!-- Status Filter -->
      <div>
        <label class="text-foreground-700 mb-2 block text-sm font-medium">Status</label>
        <select
          bind:value={statusFilter}
          class="bg-background-100 border-background-400 w-full rounded-lg border px-3 py-2 focus:border-blue-500 focus:ring-2 focus:ring-blue-500"
        >
          <option value="all">All Properties</option>
          <option value="in_progress">In Progress</option>
          <option value="completed">Completed</option>
        </select>
      </div>

      <!-- City Filter -->
      <div>
        <label class="text-foreground-700 mb-2 block text-sm font-medium">City</label>
        <select
          bind:value={cityFilter}
          class="bg-background-100 border-background-400 w-full rounded-lg border px-3 py-2 focus:border-blue-500 focus:ring-2 focus:ring-blue-500"
        >
          <option value="">All Cities</option>
          {#each cities as city}
            <option value={city}>{city}</option>
          {/each}
        </select>
      </div>

      <!-- Results Count -->
      <div class="flex items-end">
        <div class="text-foreground-600 text-sm">
          Showing {filteredProperties.length} of {properties.length} properties
        </div>
      </div>
    </div>
  </div>

  <!-- Error Message -->
  {#if error}
    <div class="rounded-lg border border-red-200 bg-red-50 p-4">
      <div class="flex items-center space-x-2">
        <span class="text-red-600">❌</span>
        <p class="font-medium text-red-800">{error}</p>
      </div>
    </div>
  {/if}

  <!-- Properties Grid -->
  {#if isLoading}
    <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-3">
      {#each Array(6) as _}
        <div
          class="bg-background-100 border-background-300 animate-pulse rounded-lg border p-6 shadow-sm"
        >
          <div class="space-y-4">
            <div class="h-4 w-3/4 rounded bg-gray-300"></div>
            <div class="h-3 w-1/2 rounded bg-gray-300"></div>
            <div class="h-8 w-full rounded bg-gray-300"></div>
          </div>
        </div>
      {/each}
    </div>
  {:else if filteredProperties.length === 0}
    <div class="py-12 text-center">
      <span class="text-6xl">🔍</span>
      <h3 class="text-foreground-900 mt-4 text-lg font-medium">
        {properties.length === 0 ? 'No properties yet' : 'No properties match your filters'}
      </h3>
      <p class="text-foreground-500 mt-2">
        {properties.length === 0
          ? 'Start by adding your first property'
          : 'Try adjusting your search criteria'}
      </p>
      {#if properties.length === 0}
        <button onclick={() => (showAddModal = true)} class="btn-primary mt-4">
          Add Property
        </button>
      {/if}
    </div>
  {:else}
    <div class="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-3">
      {#each filteredProperties as property}
        <PropertyCard {property} onUpdate={onPropertyUpdated} onDelete={onPropertyDeleted} />
      {/each}
    </div>
  {/if}
</div>

<!-- Add Property Modal -->
{#if showAddModal}
  <AddPropertyModal onClose={() => (showAddModal = false)} {onPropertyAdded} />
{/if}
