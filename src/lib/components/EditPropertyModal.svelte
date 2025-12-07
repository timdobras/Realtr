<script lang="ts">
  import { DatabaseService } from '$lib/services/databaseService';
  import { Dialog, DialogContent, DialogHeader, CityCombobox } from '$lib/components/ui';
  import type { City, Property } from '$lib/types/database';
  import { onMount } from 'svelte';

  interface Props {
    open?: boolean;
    property: Property;
    onClose: () => void;
    onSave: () => void;
  }

  let { open = $bindable(true), property, onClose, onSave }: Props = $props();

  let name = $state('');
  let city = $state('');
  let notes = $state('');
  let cities = $state<City[]>([]);
  let isSubmitting = $state(false);
  let error = $state('');

  onMount(async () => {
    // Pre-populate form with current property values
    name = property.name;
    city = property.city;
    notes = property.notes || '';

    try {
      cities = await DatabaseService.getCities();
    } catch (err) {
      console.error('Failed to load cities:', err);
    }
  });

  async function handleSubmit(event: Event) {
    event.preventDefault();

    if (!name.trim() || !city.trim()) {
      error = 'Please fill in all required fields';
      return;
    }

    try {
      isSubmitting = true;
      error = '';

      const propertyName = name.trim().toUpperCase();
      const propertyCity = city.trim().toUpperCase();

      const result = await DatabaseService.updateProperty(
        property.id!,
        propertyName,
        propertyCity,
        notes.trim() || undefined
      );

      if (result.success) {
        onSave();
      } else {
        error = result.error || 'Failed to update property';
      }
    } catch (err) {
      console.error('Error updating property:', err);
      error = 'Failed to update property';
    } finally {
      isSubmitting = false;
    }
  }

  function handleOpenChange(newOpen: boolean) {
    if (!newOpen) {
      onClose();
    }
  }
</script>

<Dialog bind:open onOpenChange={handleOpenChange}>
  <DialogContent class="max-h-[90vh] overflow-y-auto rounded-xl">
    <DialogHeader title="Edit Property" {onClose} />

    <form onsubmit={handleSubmit} class="space-y-5 p-5">
      <!-- Property Name -->
      <div>
        <label class="text-foreground-700 mb-1.5 block text-xs font-medium">
          Property Name <span class="text-red-600">*</span>
        </label>
        <input
          type="text"
          bind:value={name}
          oninput={(e) => {
            name = e.currentTarget.value.toUpperCase();
            e.currentTarget.value = name;
          }}
          placeholder="e.g., APARTMENT 85SQM"
          class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full rounded-md border px-3 py-2 text-sm transition-colors focus:ring-1 focus:outline-none"
          required
        />
      </div>

      <!-- City with Combobox -->
      <div>
        <label class="text-foreground-700 mb-1.5 block text-xs font-medium">
          City <span class="text-red-600">*</span>
        </label>
        <CityCombobox bind:value={city} {cities} placeholder="e.g., ATHENS" required />
      </div>

      <!-- Notes -->
      <div>
        <label class="text-foreground-700 mb-1.5 block text-xs font-medium">
          Notes <span class="text-foreground-500">(Optional)</span>
        </label>
        <textarea
          bind:value={notes}
          placeholder="Additional details..."
          rows="3"
          class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full resize-none rounded-md border px-3 py-2 text-sm transition-colors focus:ring-1 focus:outline-none"
        ></textarea>
      </div>

      {#if error}
        <div class="rounded-lg border border-red-300 bg-red-50 px-3 py-2">
          <p class="text-sm text-red-800">{error}</p>
        </div>
      {/if}

      <div class="border-background-200 flex items-center justify-end gap-2 border-t pt-4">
        <button
          type="button"
          onclick={onClose}
          disabled={isSubmitting}
          class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 rounded-md border px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={isSubmitting}
          class="bg-accent-500 hover:bg-accent-600 rounded-md px-4 py-2 text-sm font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
        >
          {#if isSubmitting}
            Saving...
          {:else}
            Save Changes
          {/if}
        </button>
      </div>
    </form>
  </DialogContent>
</Dialog>
