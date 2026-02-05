<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { browser } from '$app/environment';
  import { Select } from 'bits-ui';
  import { DatabaseService } from '$lib/services/databaseService';
  import PropertyCard from '$lib/components/PropertyCard.svelte';
  import AddPropertyModal from '$lib/components/AddPropertyModal.svelte';
  import type { Property, PropertyStatus } from '$lib/types/database';

  // Status display labels
  const STATUS_LABELS: Record<PropertyStatus, string> = {
    NEW: 'New',
    DONE: 'Done',
    NOT_FOUND: 'Not Found',
    ARCHIVE: 'Archived'
  };

  // Sort options
  const SORT_OPTIONS = [
    { value: 'newest', label: 'Newest' },
    { value: 'oldest', label: 'Oldest' },
    { value: 'name-asc', label: 'A-Z' },
    { value: 'name-desc', label: 'Z-A' }
  ] as const;

  // Filter persistence types and constants
  type SortOption = 'newest' | 'oldest' | 'name-asc' | 'name-desc';
  type CodeFilter = 'all' | 'with-code' | 'without-code';

  interface FilterSettings {
    selectedStatuses: PropertyStatus[];
    cityFilter: string;
    sortOrder: SortOption;
    codeFilter: CodeFilter;
  }

  const FILTER_STORAGE_KEY = 'realtr-properties-filters';
  const ALL_STATUSES: PropertyStatus[] = ['NEW', 'DONE', 'NOT_FOUND', 'ARCHIVE'];
  const DEFAULT_FILTERS: FilterSettings = {
    selectedStatuses: ['NEW', 'DONE', 'NOT_FOUND'],
    cityFilter: '',
    sortOrder: 'newest',
    codeFilter: 'all'
  };

  const CODE_FILTER_OPTIONS = [
    { value: 'all', label: 'All' },
    { value: 'with-code', label: 'With Code' },
    { value: 'without-code', label: 'No Code' }
  ] as const;

  let properties = $state<Property[]>([]);
  let filteredProperties = $state<Property[]>([]);
  let propertyThumbnails = $state<Map<number, string[]>>(new Map());
  let propertyImageCounts = $state<Map<number, number>>(new Map());
  let isLoading = $state(true);
  let error = $state<string>('');
  let showAddModal = $state(false);

  // Filters
  let searchQuery = $state('');
  let selectedStatuses = $state<Set<PropertyStatus>>(new Set(DEFAULT_FILTERS.selectedStatuses));
  let cityFilter = $state('');
  let sortOrder = $state<SortOption>('newest');
  let codeFilter = $state<CodeFilter>('all');

  // Get unique cities for filter
  let cities = $derived(Array.from(new Set(properties.map((p) => p.city))).sort());

  onMount(async () => {
    // Load saved filter settings from localStorage
    if (browser) {
      const saved = localStorage.getItem(FILTER_STORAGE_KEY);
      if (saved) {
        try {
          const settings = JSON.parse(saved) as FilterSettings;
          selectedStatuses = new Set(settings.selectedStatuses);
          cityFilter = settings.cityFilter;
          sortOrder = settings.sortOrder;
          codeFilter = settings.codeFilter ?? 'all';
        } catch {
          // Invalid JSON, use defaults
        }
      }
    }
    await loadProperties();
  });

  function saveFilters() {
    if (browser) {
      const settings: FilterSettings = {
        selectedStatuses: Array.from(selectedStatuses),
        cityFilter,
        sortOrder,
        codeFilter
      };
      localStorage.setItem(FILTER_STORAGE_KEY, JSON.stringify(settings));
    }
  }

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
          const totalCount = response.length;
          const limit = Math.min(6, totalCount);
          const filenames = response.slice(0, limit);

          // Store the total image count
          propertyImageCounts.set(property.id, totalCount);
          propertyImageCounts = new Map(propertyImageCounts);

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
    let result = properties.filter((property) => {
      // Text search (name, city, or code)
      const matchesSearch =
        searchQuery === '' ||
        property.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        property.city.toLowerCase().includes(searchQuery.toLowerCase()) ||
        (property.code && property.code.toLowerCase().includes(searchQuery.toLowerCase()));

      // Status filter - use checkbox selection
      const matchesStatus = selectedStatuses.has(property.status);

      // City filter
      const matchesCity = cityFilter === '' || property.city === cityFilter;

      // Code filter
      const matchesCode =
        codeFilter === 'all' ||
        (codeFilter === 'with-code' && property.code && property.code.trim() !== '') ||
        (codeFilter === 'without-code' && (!property.code || property.code.trim() === ''));

      return matchesSearch && matchesStatus && matchesCity && matchesCode;
    });

    // Apply sorting
    result.sort((a, b) => {
      switch (sortOrder) {
        case 'newest':
          return b.created_at - a.created_at;
        case 'oldest':
          return a.created_at - b.created_at;
        case 'name-asc':
          return a.name.localeCompare(b.name);
        case 'name-desc':
          return b.name.localeCompare(a.name);
      }
    });

    filteredProperties = result;
  }

  // Watch for filter changes
  $effect(() => {
    // Dependencies
    searchQuery;
    selectedStatuses;
    cityFilter;
    sortOrder;
    codeFilter;
    applyFilters();
  });

  // Save filters when they change (but not search query)
  $effect(() => {
    // Dependencies for persistence
    selectedStatuses;
    cityFilter;
    sortOrder;
    codeFilter;
    saveFilters();
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
    selectedStatuses = new Set(ALL_STATUSES);
    cityFilter = '';
    sortOrder = 'newest';
    codeFilter = 'all';
  }

  function resetToDefaults() {
    searchQuery = '';
    selectedStatuses = new Set(DEFAULT_FILTERS.selectedStatuses);
    cityFilter = DEFAULT_FILTERS.cityFilter;
    sortOrder = DEFAULT_FILTERS.sortOrder;
    codeFilter = DEFAULT_FILTERS.codeFilter;
    if (browser) {
      localStorage.removeItem(FILTER_STORAGE_KEY);
    }
  }

  function toggleStatus(status: PropertyStatus) {
    const newSet = new Set(selectedStatuses);
    if (newSet.has(status)) {
      newSet.delete(status);
    } else {
      newSet.add(status);
    }
    selectedStatuses = newSet;
  }

  function selectAllStatuses() {
    selectedStatuses = new Set(ALL_STATUSES);
  }

  function clearAllStatuses() {
    selectedStatuses = new Set();
  }

  // Check if filters have been modified from defaults
  let hasCustomFilters = $derived(
    selectedStatuses.size !== DEFAULT_FILTERS.selectedStatuses.length ||
      !DEFAULT_FILTERS.selectedStatuses.every((s) => selectedStatuses.has(s)) ||
      cityFilter !== DEFAULT_FILTERS.cityFilter ||
      sortOrder !== DEFAULT_FILTERS.sortOrder ||
      codeFilter !== DEFAULT_FILTERS.codeFilter
  );

  // Derived labels for display
  let sortLabel = $derived(SORT_OPTIONS.find((o) => o.value === sortOrder)?.label ?? 'Sort');
  let cityLabel = $derived(cityFilter || 'All Cities');
  let codeFilterLabel = $derived(
    CODE_FILTER_OPTIONS.find((o) => o.value === codeFilter)?.label ?? 'Code'
  );
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

  <div class="mx-auto space-y-4 p-6">
    <!-- Compact Filter Bar -->
    <div class="bg-background-50 border-background-200 border p-3">
      <div class="flex flex-wrap items-center gap-3">
        <!-- Search -->
        <div class="relative min-w-[200px] flex-1">
          <svg
            class="text-foreground-400 pointer-events-none absolute top-1/2 left-2.5 h-4 w-4 -translate-y-1/2"
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
          <input
            type="text"
            bind:value={searchQuery}
            placeholder="Search..."
            class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:border-accent-500 w-full border py-1.5 pr-3 pl-8 text-sm transition-colors focus:outline-none"
          />
        </div>

        <!-- Divider -->
        <div class="bg-background-300 hidden h-6 w-px md:block"></div>

        <!-- Status Checkboxes -->
        <div class="flex items-center gap-3">
          {#each ALL_STATUSES as status}
            {@const isChecked = selectedStatuses.has(status)}
            <button
              type="button"
              onclick={() => toggleStatus(status)}
              class="group flex cursor-pointer items-center gap-1.5"
            >
              <div
                class="flex h-4 w-4 items-center justify-center border transition-colors
                  {isChecked
                  ? 'border-accent-500 bg-accent-500'
                  : 'border-background-400 bg-transparent'}"
              >
                {#if isChecked}
                  <svg
                    class="h-3 w-3 text-white"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="3"
                      d="M5 13l4 4L19 7"
                    />
                  </svg>
                {/if}
              </div>
              <span class="text-foreground-700 text-sm">{STATUS_LABELS[status]}</span>
            </button>
          {/each}
          <div class="flex gap-1 text-xs">
            <button
              onclick={selectAllStatuses}
              class="text-foreground-500 hover:text-foreground-900 transition-colors"
            >
              All
            </button>
            <span class="text-foreground-300">/</span>
            <button
              onclick={clearAllStatuses}
              class="text-foreground-500 hover:text-foreground-900 transition-colors"
            >
              None
            </button>
          </div>
        </div>

        <!-- Divider -->
        <div class="bg-background-300 hidden h-6 w-px md:block"></div>

        <!-- City Select -->
        <Select.Root type="single" value={cityFilter} onValueChange={(v) => (cityFilter = v ?? '')}>
          <Select.Trigger
            class="border-background-300 bg-background-100 hover:bg-background-200 text-foreground-700 flex min-w-[120px] items-center justify-between gap-2 border px-2.5 py-1.5 text-sm transition-colors"
          >
            <span class="truncate">{cityLabel}</span>
            <svg
              class="text-foreground-500 h-4 w-4 shrink-0"
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
          </Select.Trigger>
          <Select.Portal>
            <Select.Content
              sideOffset={4}
              class="border-background-300 bg-background-50 z-50 max-h-60 min-w-[120px] overflow-y-auto border shadow-lg"
            >
              <Select.Viewport class="p-1">
                <Select.Item
                  value=""
                  label="All Cities"
                  class="data-[highlighted]:bg-background-200 hover:bg-background-100 flex cursor-pointer items-center justify-between px-2 py-1.5 text-sm outline-none"
                >
                  {#snippet children({ selected })}
                    <span class="text-foreground-700">All Cities</span>
                    {#if selected}
                      <svg
                        class="text-accent-500 h-4 w-4"
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
                    {/if}
                  {/snippet}
                </Select.Item>
                {#each cities as city}
                  <Select.Item
                    value={city}
                    label={city}
                    class="data-[highlighted]:bg-background-200 hover:bg-background-100 flex cursor-pointer items-center justify-between px-2 py-1.5 text-sm outline-none"
                  >
                    {#snippet children({ selected })}
                      <span class="text-foreground-900">{city}</span>
                      {#if selected}
                        <svg
                          class="text-accent-500 h-4 w-4"
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
                      {/if}
                    {/snippet}
                  </Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>

        <!-- Code Filter Select -->
        <Select.Root
          type="single"
          value={codeFilter}
          onValueChange={(v) => (codeFilter = (v as CodeFilter) ?? 'all')}
        >
          <Select.Trigger
            class="border-background-300 bg-background-100 hover:bg-background-200 text-foreground-700 flex min-w-[90px] items-center justify-between gap-2 border px-2.5 py-1.5 text-sm transition-colors"
          >
            <span>{codeFilterLabel}</span>
            <svg
              class="text-foreground-500 h-4 w-4 shrink-0"
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
          </Select.Trigger>
          <Select.Portal>
            <Select.Content
              sideOffset={4}
              class="border-background-300 bg-background-50 z-50 min-w-[90px] border shadow-lg"
            >
              <Select.Viewport class="p-1">
                {#each CODE_FILTER_OPTIONS as option}
                  <Select.Item
                    value={option.value}
                    label={option.label}
                    class="data-[highlighted]:bg-background-200 hover:bg-background-100 flex cursor-pointer items-center justify-between px-2 py-1.5 text-sm outline-none"
                  >
                    {#snippet children({ selected })}
                      <span class="text-foreground-900">{option.label}</span>
                      {#if selected}
                        <svg
                          class="text-accent-500 h-4 w-4"
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
                      {/if}
                    {/snippet}
                  </Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>

        <!-- Sort Select -->
        <Select.Root
          type="single"
          value={sortOrder}
          onValueChange={(v) => (sortOrder = (v as SortOption) ?? 'newest')}
        >
          <Select.Trigger
            class="border-background-300 bg-background-100 hover:bg-background-200 text-foreground-700 flex min-w-[90px] items-center justify-between gap-2 border px-2.5 py-1.5 text-sm transition-colors"
          >
            <span>{sortLabel}</span>
            <svg
              class="text-foreground-500 h-4 w-4 shrink-0"
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
          </Select.Trigger>
          <Select.Portal>
            <Select.Content
              sideOffset={4}
              class="border-background-300 bg-background-50 z-50 min-w-[90px] border shadow-lg"
            >
              <Select.Viewport class="p-1">
                {#each SORT_OPTIONS as option}
                  <Select.Item
                    value={option.value}
                    label={option.label}
                    class="data-[highlighted]:bg-background-200 hover:bg-background-100 flex cursor-pointer items-center justify-between px-2 py-1.5 text-sm outline-none"
                  >
                    {#snippet children({ selected })}
                      <span class="text-foreground-900">{option.label}</span>
                      {#if selected}
                        <svg
                          class="text-accent-500 h-4 w-4"
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
                      {/if}
                    {/snippet}
                  </Select.Item>
                {/each}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>

        <!-- Divider -->
        <div class="bg-background-300 hidden h-6 w-px md:block"></div>

        <!-- Results & Reset -->
        <div class="flex items-center gap-3">
          <span class="text-foreground-500 text-sm">
            <span class="text-foreground-900 font-medium">{filteredProperties.length}</span>
            <span class="text-foreground-400">/</span>
            <span>{properties.length}</span>
          </span>
          {#if hasCustomFilters}
            <button
              onclick={resetToDefaults}
              class="text-foreground-500 hover:text-foreground-900 text-sm transition-colors"
            >
              Reset
            </button>
          {/if}
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
            totalImageCount={propertyImageCounts.get(property.id!) || 0}
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
