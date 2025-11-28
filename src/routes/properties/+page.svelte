<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { DatabaseService } from '$lib/services/databaseService';
  import PropertyCard from '$lib/components/PropertyCard.svelte';
  import AddPropertyModal from '$lib/components/AddPropertyModal.svelte';
  import type { Property, PropertyStatus } from '$lib/types/database';

  let properties = $state<Property[]>([]);
  let filteredProperties = $state<Property[]>([]);
  let propertyThumbnails = $state<Map<number, string[]>>(new Map());
  let isLoading = $state(true);
  let error = $state<string>('');
  let showAddModal = $state(false);

  // Filters
  let searchQuery = $state('');
  let statusFilter = $state<'ALL' | PropertyStatus>('ALL');
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
      await loadThumbnails();
    } catch (err) {
      console.error('Error loading properties:', err);
      error = 'Failed to load properties';
    } finally {
      isLoading = false;
    }
  }

  async function loadThumbnails() {
    // Load thumbnails progressively in parallel for better performance
    const thumbnailPromises = properties.map(async (property) => {
      if (!property.id) return;

      try {
        // Get list of thumbnail filenames
        const response = await invoke('list_thumbnails', {
          folderPath: property.folder_path,
          status: property.status
        });

        if (Array.isArray(response) && response.length > 0) {
          const limit = Math.min(6, response.length);
          const filenames = response.slice(0, limit);

          // Load all thumbnails for this property in parallel
          const thumbnailPromises = filenames.map(async (filename) => {
            try {
              const base64Data = await invoke('get_thumbnail_as_base64', {
                folderPath: property.folder_path,
                status: property.status,
                filename: filename
              });
              return `data:image/jpeg;base64,${base64Data}`;
            } catch (e) {
              console.error(`Failed to load thumbnail for ${property.name}:`, e);
              return null;
            }
          });

          const thumbnails = (await Promise.all(thumbnailPromises)).filter(
            (t): t is string => t !== null
          );

          if (thumbnails.length > 0) {
            // Update the map reactively
            propertyThumbnails.set(property.id, thumbnails);
            propertyThumbnails = new Map(propertyThumbnails);
          }
        }
      } catch (e) {
        // Silently fail for individual properties
        console.error(`Failed to load thumbnails for ${property.name}:`, e);
      }
    });

    // Wait for all property thumbnails to load in parallel
    await Promise.all(thumbnailPromises);
  }

  function applyFilters() {
    filteredProperties = properties.filter((property) => {
      // Text search
      const matchesSearch =
        searchQuery === '' ||
        property.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        property.city.toLowerCase().includes(searchQuery.toLowerCase());

      // Status filter - hide ARCHIVE by default unless searching or explicitly selected
      const matchesStatus =
        statusFilter === 'ALL'
          ? searchQuery !== '' || property.status !== 'ARCHIVE' // Hide archive unless searching
          : property.status === statusFilter;

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
    statusFilter = 'ALL';
    cityFilter = '';
  }
</script>

<div class="bg-background-0 min-h-full">
  <!-- Header -->
  <div class="bg-background-50 border-background-200 border-b">
    <div class="px-6 py-4">
      <div class="mx-auto">
        <div class="flex items-center justify-between">
          <div>
            <h1 class="text-foreground-900 text-xl font-semibold">Properties</h1>
            <p class="text-foreground-600 mt-0.5 text-sm">Manage your photo projects</p>
          </div>
          <button
            onclick={() => (showAddModal = true)}
            class="bg-accent-500 hover:bg-accent-600 flex items-center gap-2 px-4 py-2 text-sm font-medium text-white transition-colors"
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

  <div class="mx-auto space-y-5 p-6">
    <!-- Filters -->
    <div class="bg-background-50 border-background-200 border p-4">
      <div class="mb-3 flex items-center justify-between">
        <h2 class="text-foreground-900 text-sm font-semibold">Filters</h2>

        {#if searchQuery || statusFilter !== 'ALL' || cityFilter}
          <button
            onclick={clearAllFilters}
            class="text-foreground-600 hover:text-foreground-900 text-xs font-medium transition-colors"
          >
            Clear
          </button>
        {/if}
      </div>

      <div class="grid grid-cols-1 gap-3 md:grid-cols-4">
        <!-- Search -->
        <div>
          <label class="text-foreground-700 mb-1 block text-xs font-medium">Search</label>
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Search..."
            class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full border px-3 py-2 text-sm transition-colors focus:ring-1 focus:outline-none"
          />
        </div>

        <!-- Status Filter -->
        <div>
          <label class="text-foreground-700 mb-1 block text-xs font-medium">Status</label>
          <select
            bind:value={statusFilter}
            class="bg-background-100 border-background-300 text-foreground-900 focus:ring-accent-500 focus:border-accent-500 w-full border px-3 py-2 text-sm transition-colors focus:ring-1 focus:outline-none"
          >
            <option value="ALL">All (exclude archived)</option>
            <option value="NEW">New</option>
            <option value="DONE">Done</option>
            <option value="NOT_FOUND">Not Found</option>
            <option value="ARCHIVE">Archived</option>
          </select>
        </div>

        <!-- City Filter -->
        <div>
          <label class="text-foreground-700 mb-1 block text-xs font-medium">City</label>
          <select
            bind:value={cityFilter}
            class="bg-background-100 border-background-300 text-foreground-900 focus:ring-accent-500 focus:border-accent-500 w-full border px-3 py-2 text-sm transition-colors focus:ring-1 focus:outline-none"
          >
            <option value="">All Cities</option>
            {#each cities as city}
              <option value={city}>{city}</option>
            {/each}
          </select>
        </div>

        <!-- Results Count -->
        <div class="flex items-end">
          <div class="bg-background-100 border-background-200 w-full border px-3 py-2">
            <div class="text-foreground-600 text-xs">
              <span class="text-foreground-900 font-semibold">{filteredProperties.length}</span>
              <span class="text-foreground-500"> of </span>
              <span class="text-foreground-700 font-semibold">{properties.length}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Error Message -->
    {#if error}
      <div class="border-background-300 bg-background-100 border px-3 py-2">
        <p class="text-foreground-900 text-sm">{error}</p>
      </div>
    {/if}

    <!-- Properties Grid -->
    {#if isLoading}
      <div class="text-foreground-500 flex items-center gap-2 text-sm">
        <div class="border-foreground-300 h-4 w-4 animate-spin border-2 border-t-transparent"></div>
        <span>Loading properties...</span>
      </div>
    {:else if filteredProperties.length === 0}
      <div class="bg-background-50 border-background-200 border py-10 text-center">
        <p class="text-foreground-500 mb-3 text-sm">
          {properties.length === 0 ? 'No properties yet' : 'No properties match your filters'}
        </p>

        {#if properties.length === 0}
          <button
            onclick={() => (showAddModal = true)}
            class="bg-accent-500 hover:bg-accent-600 inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white transition-colors"
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
            class="bg-background-200 text-foreground-700 hover:bg-background-300 inline-flex px-4 py-2 text-sm font-medium transition-colors"
          >
            Clear Filters
          </button>
        {/if}
      </div>
    {:else}
      <!-- Properties Grid -->
      <div class="grid grid-cols-1 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
        {#each filteredProperties as property}
          <PropertyCard
            {property}
            thumbnails={propertyThumbnails.get(property.id!) || []}
            onUpdate={onPropertyUpdated}
            onDelete={onPropertyDeleted}
          />
        {/each}
      </div>
    {/if}
  </div>
</div>

<!-- Add Property Modal -->
{#if showAddModal}
  <AddPropertyModal onClose={() => (showAddModal = false)} {onPropertyAdded} />
{/if}
