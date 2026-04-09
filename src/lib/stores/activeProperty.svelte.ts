/**
 * Reactive store for the currently-viewed property.
 *
 * Replaces the prop-drilling pattern where the property layout AND
 * each of the four step pages independently called
 * `DatabaseService.getPropertyById(id)`. Each navigation between steps
 * was triggering an unnecessary refetch and the status dropdown in the
 * layout could go out of sync with the data the step page was using.
 *
 * Usage:
 *   ```ts
 *   // In a +layout.svelte or +page.svelte:
 *   import { activeProperty } from '$lib/stores/activeProperty.svelte';
 *
 *   onMount(() => activeProperty.load(propertyId));
 *
 *   // Then anywhere in the component subtree:
 *   {#if activeProperty.property}
 *     <h1>{activeProperty.property.name}</h1>
 *   {/if}
 *   ```
 *
 * The store is implemented as a class with `$state` runes — the modern
 * Svelte 5 idiom for shared mutable state in `.svelte.ts` modules. It
 * is exported as a singleton instance because there is only ever one
 * active property at a time in this app.
 */

import { DatabaseService } from '$lib/services/databaseService';
import type { Property } from '$lib/types/database';

class ActivePropertyStore {
  property = $state<Property | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  /** ID of the in-flight load, used to ignore stale responses if the
   *  user navigates between properties before the first fetch resolves. */
  private currentRequestId = 0;

  /**
   * Load a property by ID. If the same ID is already loaded and not
   * stale, this is a no-op (the cache is the store itself).
   *
   * Pass `force: true` after a mutation (status update, rename, etc) to
   * bypass the cache check and refetch.
   */
  async load(propertyId: number, options: { force?: boolean } = {}): Promise<void> {
    if (!Number.isFinite(propertyId) || propertyId < 1) {
      this.error = 'Invalid property ID';
      this.loading = false;
      this.property = null;
      return;
    }

    // Cache hit: same property already loaded.
    if (!options.force && this.property?.id === propertyId) {
      return;
    }

    const requestId = ++this.currentRequestId;
    this.loading = true;
    this.error = null;

    try {
      const result = await DatabaseService.getPropertyById(propertyId);
      // Drop the response if a newer load() has been issued in the meantime.
      if (requestId !== this.currentRequestId) {
        return;
      }
      if (!result) {
        this.error = 'Property not found';
        this.property = null;
        return;
      }
      this.property = result;
    } catch (e) {
      if (requestId !== this.currentRequestId) {
        return;
      }
      this.error = `Failed to load property: ${e}`;
      this.property = null;
    } finally {
      if (requestId === this.currentRequestId) {
        this.loading = false;
      }
    }
  }

  /** Re-fetch the current property from the database, e.g. after a
   *  status change or rename. No-op if no property is currently loaded. */
  async refresh(): Promise<void> {
    if (this.property?.id != null) {
      await this.load(this.property.id, { force: true });
    }
  }

  /** Clear the store. Call from onDestroy of the property layout if you
   *  want to drop the cached property when leaving the section. */
  clear(): void {
    this.currentRequestId++;
    this.property = null;
    this.loading = false;
    this.error = null;
  }
}

/** Singleton — there's only ever one active property at a time. */
export const activeProperty = new ActivePropertyStore();
