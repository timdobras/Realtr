<script lang="ts">
  import { page } from '$app/stores';
  import { DatabaseService } from '$lib/services/databaseService';
  import type { Property } from '$lib/types/database';
  import { onMount } from 'svelte';
  export const prerender = false;

  let { children } = $props();

  let property = $state<Property | null>(null);
  let error = $state<String>('');
  let loading = $state<Boolean>(true);

  const propertyId = $derived(Number($page.params.id));

  // Define the workflow steps
  const steps = [
    {
      number: 1,
      title: 'Copy to INTERNET',
      description: 'Copy originals to INTERNET folder',
      path: `/properties/${propertyId}/step1`,
      icon: `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"/>
      </svg>`
    },
    {
      number: 2,
      title: 'Order & Rename',
      description: 'Order and rename images',
      path: `/properties/${propertyId}/step2`,
      icon: `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4"/>
      </svg>`
    },
    {
      number: 3,
      title: 'Copy to AGGELIA',
      description: 'Copy edited images to AGGELIA',
      path: `/properties/${propertyId}/step3`,
      icon: `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"/>
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 5a2 2 0 012-2h4a2 2 0 012 2v6H8V5z"/>
      </svg>`
    },
    {
      number: 4,
      title: 'Add Watermark',
      description: 'Apply watermark to final images',
      path: `/properties/${propertyId}/step4`,
      icon: `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"/>
      </svg>`
    }
  ];

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
        class="border-accent-500 mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-t-transparent"
      ></div>
      <p class="text-foreground-600 font-medium">Loading property...</p>
    </div>
  </div>
{:else if property}
  <div class="bg-background-0 text-foreground-950 flex h-full w-full flex-col">
    <!-- Top Header -->
    <div class="bg-background-50 border-background-200 flex-shrink-0 border-b shadow-sm">
      <div class="px-6 py-6">
        <div class="flex items-center justify-between">
          <div class="flex items-center space-x-4">
            <!-- Back Button -->
            <a
              href="/properties"
              class="border-background-300 bg-background-100 text-foreground-600 hover:bg-background-200 hover:text-foreground-900 flex h-10 w-10 items-center justify-center rounded-lg border transition-colors"
              title="Back to Properties"
            >
              <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M15 19l-7-7 7-7"
                />
              </svg>
            </a>

            <!-- Property Info -->
            <div class="flex items-center space-x-4">
              <div class="bg-accent-100 flex h-12 w-12 items-center justify-center rounded-lg">
                <svg
                  class="text-accent-600 h-6 w-6"
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
              </div>
              <a href="/properties/{property.id}">
                <h1 class="text-foreground-900 text-2xl font-bold">{property.name}</h1>
                <div class="text-foreground-600 flex items-center space-x-2">
                  <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
          </div>

          <!-- Property Status -->
          <div class="flex items-center space-x-4">
            <span
              class="inline-flex items-center rounded-lg border px-3 py-1.5 text-sm font-medium {property.completed
                ? 'border-green-200 bg-green-50 text-green-700'
                : 'border-orange-200 bg-orange-50 text-orange-700'}"
            >
              {#if property.completed}
                <svg class="mr-1.5 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M9 12l2 2 4-4"
                  />
                </svg>
                Completed
              {:else}
                <svg class="mr-1.5 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M12 8v4l3 3"
                  />
                </svg>
                In Progress
              {/if}
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- Step Navigation -->
    <div class="bg-background-50 border-background-200 flex-shrink-0 border-b">
      <div class="px-6 py-4">
        <div class="mb-4 flex items-center justify-between">
          <h2 class="text-foreground-900 font-semibold">Workflow Progress</h2>
          <span class="text-foreground-500 text-sm">
            Step {currentStep || 0} of {steps.length}
          </span>
        </div>

        <!-- Steps -->
        <div class="flex items-center justify-between">
          {#each steps as step, index}
            <div class="flex items-center {index < steps.length - 1 ? 'flex-1' : ''}">
              <!-- Step Item -->
              <a
                href={step.path}
                class="group flex items-center space-x-3 rounded-lg border px-4 py-3 transition-all duration-200 {currentStep ===
                step.number
                  ? 'bg-accent-500 border-accent-500 text-white shadow-md'
                  : currentStep > step.number
                    ? 'border-green-200 bg-green-50 text-green-700 hover:bg-green-100'
                    : 'bg-background-100 text-foreground-700 border-background-300 hover:bg-background-200'}"
              >
                <!-- Step Icon/Number -->
                <div
                  class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full {currentStep ===
                  step.number
                    ? 'bg-white/20'
                    : currentStep > step.number
                      ? 'bg-green-100'
                      : 'bg-background-200'}"
                >
                  {#if currentStep > step.number}
                    <svg
                      class="h-4 w-4 text-green-600"
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
                  {:else}
                    <span
                      class="text-sm font-bold {currentStep === step.number
                        ? 'text-white'
                        : 'text-foreground-600'}"
                    >
                      {step.number}
                    </span>
                  {/if}
                </div>

                <!-- Step Content -->
                <div class="min-w-0">
                  <div class="mb-1 flex items-center space-x-2">
                    {@html step.icon}
                    <h3 class="truncate text-sm font-semibold">
                      {step.title}
                    </h3>
                  </div>
                  <p class="truncate text-xs opacity-75">
                    {step.description}
                  </p>
                </div>
              </a>

              <!-- Connector Line -->
              {#if index < steps.length - 1}
                <div class="mx-4 flex-1">
                  <div
                    class="h-0.5 {currentStep > step.number
                      ? 'bg-green-300'
                      : 'bg-background-300'} transition-colors duration-300"
                  ></div>
                </div>
              {/if}
            </div>
          {/each}
        </div>

        <!-- Progress Bar -->
        <div class="mt-4">
          <div class="bg-background-200 h-2 w-full overflow-hidden rounded-full">
            <div
              class="bg-accent-500 h-full rounded-full transition-all duration-500 ease-out"
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
      <div class="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-red-100">
        <svg class="h-10 w-10 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
      </div>
      <h3 class="text-foreground-900 mb-2 text-xl font-semibold">Property Not Found</h3>
      <p class="text-foreground-600 mb-6">
        The property you're looking for doesn't exist or has been removed.
      </p>
      <div class="flex items-center justify-center space-x-4">
        <a
          href="/properties"
          class="bg-accent-500 hover:bg-accent-600 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium text-white transition-colors"
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
          class="bg-background-200 text-foreground-700 hover:bg-background-300 flex items-center space-x-2 rounded-lg px-6 py-3 font-medium transition-colors"
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
