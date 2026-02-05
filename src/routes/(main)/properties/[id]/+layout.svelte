<script lang="ts">
  import { page } from '$app/stores';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property, PropertyStatus } from '$lib/types/database';
  import { onMount } from 'svelte';
  export const prerender = false;

  let { children } = $props();

  let property = $state<Property | null>(null);
  let error = $state<String>('');
  let loading = $state<Boolean>(true);
  let isUpdatingStatus = $state(false);
  let statusError = $state<string | null>(null);
  let selectKey = $state(0); // Used to force dropdown reset on error

  const propertyId = $derived(Number($page.params.id));

  async function handleStatusChange(newStatus: PropertyStatus) {
    if (!property || isUpdatingStatus) return;

    // Clear any previous error
    statusError = null;

    try {
      isUpdatingStatus = true;
      const result = await DatabaseService.updatePropertyStatus(property.id!, newStatus);

      if (result.success) {
        // Reload property to get updated data
        property = await DatabaseService.getPropertyById(propertyId);
      } else {
        // Show error to user and reset dropdown
        statusError = result.error || 'Failed to update status';
        selectKey++; // Force dropdown to re-render with original value
        // Auto-hide error after 5 seconds
        setTimeout(() => {
          statusError = null;
        }, 5000);
      }
    } catch (err) {
      statusError = `Failed to update status: ${err}`;
      selectKey++; // Force dropdown to re-render with original value
      setTimeout(() => {
        statusError = null;
      }, 5000);
    } finally {
      isUpdatingStatus = false;
    }
  }

  // Define the workflow steps
  const steps = $derived([
    {
      number: 1,
      title: 'Copy to INTERNET',
      path: `/properties/${propertyId}/step1`
    },
    {
      number: 2,
      title: 'Order & Rename',
      path: `/properties/${propertyId}/step2`
    },
    {
      number: 3,
      title: 'Copy to AGGELIA',
      path: `/properties/${propertyId}/step3`
    },
    {
      number: 4,
      title: 'Add Watermark',
      path: `/properties/${propertyId}/step4`
    }
  ]);

  // Get current step from URL
  const currentStep = $derived.by(() => {
    const pathname = $page.url.pathname;
    const stepMatch = pathname.match(/\/step(\d+)$/);
    return stepMatch ? parseInt(stepMatch[1]) : 0;
  });

  onMount(async () => {
    if (!propertyId) {
      error = 'Invalid property ID';
      loading = false;
      return;
    }

    try {
      loading = true;
      property = await DatabaseService.getPropertyById(propertyId);
      if (!property) {
        error = 'Property not found';
        loading = false;
        return;
      }
    } catch (e) {
      error = `Failed to load property: ${e}`;
    } finally {
      loading = false;
    }
  });
</script>

{#if loading}
  <div class="bg-background-0 flex h-full items-center justify-center">
    <div class="text-center">
      <div
        class="border-foreground-300 mx-auto mb-4 h-8 w-8 animate-spin border-4 border-t-transparent"
      ></div>
      <p class="text-foreground-600 font-medium">Loading property...</p>
    </div>
  </div>
{:else if property}
  <div class="bg-background-0 text-foreground-950 flex h-full w-full flex-col">
    <!-- Top Header -->
    <div class="bg-background-50 border-background-200 flex-shrink-0 border-b">
      <div class="px-5 py-4">
        <div class="flex items-center justify-between">
          <div class="flex items-center space-x-3">
            <!-- Back Button -->
            <a
              href="/properties"
              class="border-background-300 bg-background-100 text-foreground-600 hover:bg-background-200 hover:text-foreground-900 flex h-9 w-9 items-center justify-center border transition-colors"
              title="Back to Properties"
            >
              <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M15 19l-7-7 7-7"
                />
              </svg>
            </a>

            <!-- Property Info -->
            <a href="/properties/{property.id}">
              <h1 class="text-foreground-900 text-xl font-semibold">{property.name}</h1>
              <div class="text-foreground-600 flex items-center space-x-1.5 text-sm">
                <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
                <span>{property.city}</span>
              </div>
            </a>
          </div>

          <!-- Property Status Dropdown -->
          <div class="flex flex-col gap-1">
            <div class="flex items-center gap-3">
              <label class="text-foreground-700 text-sm font-medium">Status:</label>
              {#key selectKey}
                <select
                  value={property.status}
                  onchange={(e) => handleStatusChange(e.currentTarget.value as PropertyStatus)}
                  disabled={isUpdatingStatus}
                  class="border-background-300 bg-background-100 text-foreground-900 focus:ring-accent-500 focus:border-accent-500 border px-3 py-1.5 text-sm transition-colors focus:ring-1 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <option value="NEW">New</option>
                  <option value="DONE">Done</option>
                  <option value="NOT_FOUND">Not Found</option>
                  <option value="ARCHIVE">Archive</option>
                </select>
              {/key}
              {#if isUpdatingStatus}
                <div
                  class="border-foreground-300 h-4 w-4 animate-spin border-2 border-t-transparent"
                ></div>
              {/if}
            </div>
            {#if statusError}
              <p class="text-xs text-red-600">{statusError}</p>
            {/if}
          </div>
        </div>
      </div>
    </div>

    <!-- Step Navigation -->
    <div class="bg-background-50 border-background-200 flex-shrink-0 border-b">
      <div class="px-5 py-3">
        <div class="mb-3 flex items-center justify-between">
          <h2 class="text-foreground-900 text-sm font-semibold">Workflow Progress</h2>
          <span class="text-foreground-500 text-xs">
            Step {currentStep || 0} of {steps.length}
          </span>
        </div>

        <!-- Steps -->
        <div class="flex items-center gap-2">
          {#each steps as step}
            <a
              href={step.path}
              class="flex items-center space-x-2 border px-3 py-2 transition-colors {currentStep ===
              step.number
                ? 'bg-foreground-900 border-foreground-900 text-background-0'
                : currentStep > step.number
                  ? 'border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200'
                  : 'bg-background-100 text-foreground-700 border-background-300 hover:bg-background-200'}"
            >
              <!-- Step Number -->
              <div class="flex h-5 w-5 flex-shrink-0 items-center justify-center">
                {#if currentStep > step.number}
                  <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M5 13l4 4L19 7"
                    />
                  </svg>
                {:else}
                  <span class="text-xs font-semibold">
                    {step.number}
                  </span>
                {/if}
              </div>

              <!-- Step Title -->
              <h3 class="text-xs font-semibold">
                {step.title}
              </h3>
            </a>
          {/each}
        </div>

        <!-- Progress Bar -->
        <div class="mt-3">
          <div class="bg-background-200 h-1 w-full overflow-hidden">
            <div
              class="bg-foreground-900 h-full transition-all duration-500 ease-out"
              style="width: {currentStep === 0 ? 0 : (currentStep / steps.length) * 100}%"
            ></div>
          </div>
        </div>
      </div>
    </div>

    <!-- Main Content Area -->
    <main class="flex-1 overflow-hidden">
      <section class="h-full overflow-y-auto">
        {@render children()}
      </section>
    </main>
  </div>
{:else}
  <div class="bg-background-0 flex h-full items-center justify-center">
    <div class="mx-auto max-w-md text-center">
      <h3 class="text-foreground-900 mb-2 text-lg font-semibold">Property Not Found</h3>
      <p class="text-foreground-600 mb-5 text-sm">
        The property you're looking for doesn't exist or has been removed.
      </p>
      <div class="flex items-center justify-center space-x-3">
        <a
          href="/properties"
          class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 px-4 py-2 text-sm font-medium text-white transition-colors"
        >
          <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M15 19l-7-7 7-7"
            />
          </svg>
          <span>Back to Properties</span>
        </a>
        <a
          href="/"
          class="bg-background-200 text-foreground-700 hover:bg-background-300 flex items-center space-x-2 px-4 py-2 text-sm font-medium transition-colors"
        >
          <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"
            />
          </svg>
          <span>Dashboard</span>
        </a>
      </div>
    </div>
  </div>
{/if}
