<script lang="ts">
  import { DatabaseService } from '$lib/services/databaseService';
  import type { City } from '$lib/types/database';
  import { onMount } from 'svelte';

  interface Props {
    onClose: () => void;
    onPropertyAdded: () => void;
  }

  let { onClose, onPropertyAdded }: Props = $props();

  let name = $state('');
  let city = $state('');
  let notes = $state('');
  let cities = $state<City[]>([]);
  let showCityDropdown = $state(false);
  let isSubmitting = $state(false);
  let error = $state('');

  // Load cities on mount
  onMount(async () => {
    try {
      cities = await DatabaseService.getCities();
    } catch (err) {
      console.error('Failed to load cities:', err);
    }
  });

  // Use $derived instead of $effect for computed values
  let filteredCities = $derived.by(() => {
    if (city.trim()) {
      return cities.filter((c) => c.name.toLowerCase().includes(city.toLowerCase())).slice(0, 5);
    } else {
      return cities.slice(0, 5);
    }
  });

  // Handle city input focus
  function handleCityFocus() {
    showCityDropdown = filteredCities.length > 0;
  }

  // Handle city input changes
  function handleCityInput() {
    showCityDropdown = city.trim() ? filteredCities.length > 0 : false;
  }

  function selectCity(cityName: string) {
    city = cityName;
    showCityDropdown = false;
  }

  async function handleSubmit() {
    if (!name.trim() || !city.trim()) {
      error = 'Please fill in all required fields';
      return;
    }

    try {
      isSubmitting = true;
      error = '';

      const result = await DatabaseService.createProperty(
        name.trim().toUpperCase(),
        city.trim().toUpperCase(),
        notes.trim() || undefined
      );

      if (result.success) {
        onPropertyAdded();
      } else {
        error = result.error || 'Failed to create property';
      }
    } catch (err) {
      console.error('Error creating property:', err);
      error = 'Failed to create property';
    } finally {
      isSubmitting = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="bg-opacity-50 fixed inset-0 z-50 flex items-center justify-center bg-black p-4">
  <div
    class="bg-background-50 border-background-200 max-h-[90vh] w-full max-w-md overflow-y-auto rounded-xl border shadow-xl"
  >
    <!-- Modal Header -->
    <div class="border-background-200 flex items-center justify-between border-b p-6">
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
              d="M12 4v16m8-8H4"
            />
          </svg>
        </div>
        <div>
          <h2 class="text-foreground-900 text-xl font-semibold">Add New Property</h2>
          <p class="text-foreground-600 text-sm">Create a new property entry</p>
        </div>
      </div>
      <button
        onclick={onClose}
        class="text-foreground-400 hover:text-foreground-600 hover:bg-background-100 rounded-lg p-1 transition-colors"
        title="Close"
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

    <!-- Modal Content -->
    <form onsubmit={handleSubmit} class="space-y-6 p-6">
      <!-- Property Name -->
      <div>
        <label class="text-foreground-700 mb-3 block text-sm font-medium">
          <div class="flex items-center space-x-2">
            <svg
              class="text-foreground-600 h-4 w-4"
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
            <span>Property Name</span>
            <span class="text-red-500">*</span>
          </div>
        </label>
        <input
          type="text"
          bind:value={name}
          placeholder="e.g., Apartment 85sqm, 2 bedrooms"
          class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full rounded-lg border px-4 py-3 transition-colors focus:ring-2 focus:outline-none"
          required
        />
      </div>

      <!-- City with Autocomplete -->
      <div class="relative">
        <label class="text-foreground-700 mb-3 block text-sm font-medium">
          <div class="flex items-center space-x-2">
            <svg
              class="text-foreground-600 h-4 w-4"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z"
              />
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M15 11a3 3 0 11-6 0 3 3 0 016 0z"
              />
            </svg>
            <span>City</span>
            <span class="text-red-500">*</span>
          </div>
        </label>
        <div class="relative">
          <input
            type="text"
            bind:value={city}
            oninput={handleCityInput}
            onfocus={handleCityFocus}
            onblur={() => setTimeout(() => (showCityDropdown = false), 200)}
            placeholder="e.g., Athens, Thessaloniki"
            class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full rounded-lg border px-4 py-3 pr-10 transition-colors focus:ring-2 focus:outline-none"
            required
          />
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

        <!-- City Dropdown -->
        {#if showCityDropdown}
          <div
            class="border-background-300 bg-background-50 absolute z-10 mt-2 max-h-48 w-full overflow-y-auto rounded-lg border shadow-lg"
          >
            {#each filteredCities as cityOption}
              <button
                type="button"
                onclick={() => selectCity(cityOption.name)}
                class="hover:bg-background-100 flex w-full items-center justify-between px-4 py-3 text-left transition-colors first:rounded-t-lg last:rounded-b-lg"
              >
                <div class="flex items-center space-x-2">
                  <svg
                    class="text-foreground-500 h-4 w-4"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M17.657 16.657L13.414 20.9a1.998 1.998 0 01-2.827 0l-4.244-4.243a8 8 0 1111.314 0z"
                    />
                  </svg>
                  <span class="text-foreground-900 font-medium">{cityOption.name}</span>
                </div>
                <span class="text-foreground-500 text-xs">
                  {cityOption.usageCount}
                  {cityOption.usageCount === 1 ? 'time' : 'times'}
                </span>
              </button>
            {/each}

            {#if filteredCities.length === 0 && city.trim()}
              <div class="px-4 py-3 text-center">
                <div class="text-foreground-500 flex items-center justify-center space-x-2">
                  <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M12 4v16m8-8H4"
                    />
                  </svg>
                  <span class="text-sm">New city will be created</span>
                </div>
              </div>
            {/if}
          </div>
        {/if}
      </div>

      <!-- Notes -->
      <div>
        <label class="text-foreground-700 mb-3 block text-sm font-medium">
          <div class="flex items-center space-x-2">
            <svg
              class="text-foreground-600 h-4 w-4"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
              />
            </svg>
            <span>Notes</span>
            <span class="text-foreground-500 text-sm">(Optional)</span>
          </div>
        </label>
        <textarea
          bind:value={notes}
          placeholder="Additional details about the property..."
          rows="3"
          class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full resize-none rounded-lg border px-4 py-3 transition-colors focus:ring-2 focus:outline-none"
        ></textarea>
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
            <p class="text-sm font-medium text-red-800">{error}</p>
          </div>
        </div>
      {/if}

      <!-- Actions -->
      <div class="border-background-200 flex items-center justify-end space-x-3 border-t pt-4">
        <button
          type="button"
          onclick={onClose}
          disabled={isSubmitting}
          class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 rounded-lg border px-6 py-3 text-sm font-medium transition-colors disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={isSubmitting}
          class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 text-sm font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
        >
          {#if isSubmitting}
            <div
              class="h-4 w-4 animate-spin rounded-full border-2 border-white border-t-transparent"
            ></div>
            <span>Creating...</span>
          {:else}
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 4v16m8-8H4"
              />
            </svg>
            <span>Create Property</span>
          {/if}
        </button>
      </div>
    </form>
  </div>
</div>
