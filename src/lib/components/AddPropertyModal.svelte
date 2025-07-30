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
      return cities.filter(c => 
        c.name.toLowerCase().includes(city.toLowerCase())
      ).slice(0, 5);
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

<div class="fixed inset-0 bg-black/70 backdrop-blur-lg flex items-center justify-center z-50">
  <div class="bg-background-100 rounded-lg p-6 max-w-md w-full mx-4 max-h-[90vh] overflow-y-auto shadow-lg">
    <h2 class="text-xl font-semibold text-foreground-900 mb-6">Add New Property</h2>
    
    <form onsubmit={handleSubmit} class="space-y-4">
      <!-- Property Name -->
      <div>
        <label class="block text-sm font-medium text-foreground-700 mb-2">
          Property Name <span class="text-red-500">*</span>
        </label>
        <input
          type="text"
          bind:value={name}
          placeholder="e.g., Apartment 85sqm, 2 bedrooms"
          class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          required
        />
      </div>
      
      <!-- City with Autocomplete -->
      <div class="relative">
        <label class="block text-sm font-medium text-foreground-700 mb-2">
          City <span class="text-red-500">*</span>
        </label>
        <input
          type="text"
          bind:value={city}
          oninput={handleCityInput}
          onfocus={handleCityFocus}
          onblur={() => setTimeout(() => showCityDropdown = false, 200)}
          placeholder="e.g., Athens, Thessaloniki"
          class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          required
        />
        
        <!-- City Dropdown -->
        {#if showCityDropdown}
          <div class="absolute z-10 w-full mt-1 bg-background-100 border border-gray-300 rounded-lg shadow-lg max-h-48 overflow-y-auto">
            {#each filteredCities as cityOption}
              <button
                type="button"
                onclick={() => selectCity(cityOption.name)}
                class="w-full px-3 py-2 text-left hover:bg-gray-50 flex items-center justify-between"
              >
                <span>{cityOption.name}</span>
                <span class="text-xs text-foreground-500">Used {cityOption.usageCount} times</span>
              </button>
            {/each}
            
            {#if filteredCities.length === 0 && city.trim()}
              <div class="px-3 py-2 text-foreground-500 text-sm">
                No cities found. You can type a new city name.
              </div>
            {/if}
          </div>
        {/if}
      </div>
      
      <!-- Notes -->
      <div>
        <label class="block text-sm font-medium text-foreground-700 mb-2">
          Notes (Optional)
        </label>
        <textarea
          bind:value={notes}
          placeholder="Additional details about the property..."
          rows="3"
          class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 resize-none"
        ></textarea>
      </div>
      
      <!-- Error Message -->
      {#if error}
        <div class="bg-red-50 border border-red-200 rounded-lg p-3">
          <p class="text-red-800 text-sm">{error}</p>
        </div>
      {/if}
      
      <!-- Actions -->
      <div class="flex items-center justify-end space-x-4 pt-4">
        <button
          type="button"
          onclick={onClose}
          class="btn-secondary"
          disabled={isSubmitting}
        >
          Cancel
        </button>
        <button
          type="submit"
          class="btn-primary disabled:opacity-50 disabled:cursor-not-allowed"
          disabled={isSubmitting}
        >
          {isSubmitting ? 'Creating...' : 'Create Property'}
        </button>
      </div>
    </form>
  </div>
</div>
