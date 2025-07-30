<script lang="ts">
    import { page } from '$app/stores'; // Change from $app/state to $app/stores
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
        },
        {
            number: 2,
            title: 'Order & Rename',
            description: 'Order and rename images',
            path: `/properties/${propertyId}/step2`,
        },
        {
            number: 3,
            title: 'Copy to AGGELIA',
            description: 'Copy edited images to AGGELIA',
            path: `/properties/${propertyId}/step3`,
        },
        {
            number: 4,
            title: 'Add Watermark',
            description: 'Apply watermark to final images',
            path: `/properties/${propertyId}/step4`,
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
    <div class="flex items-center justify-center h-full">
        <div class="text-center">
            <div class="animate-spin w-8 h-8 border-4 border-blue-500 border-t-transparent rounded-full mx-auto mb-4"></div>
            <p class="text-gray-600">Loading...</p>
        </div>
    </div>
{:else if property}
    <div class="flex flex-col w-full h-full text-foreground-950 ">
        <!-- Top Navbar -->
        <div class="w-full bg-background-100 flex-shrink-0 border-b border-background-300">
            <div class="p-4 px-8 flex items-center justify-between">
                <a href={`/properties/${property.id}`} class="flex flex-col gap-1">
                    <h1 class="text-xl font-bold">{property.name}</h1>
                    <h2 class="text-foreground-700">{property.city}</h2>
                </a>
            </div>
        </div>

        <!-- Step Navigation -->
        <div class="w-full bg-background-100 flex-shrink-0 border-b border-background-300">
            <div class="">
                <div class="flex items-center w-full p-4">
                    {#each steps as step, index}
                        <div class="flex items-center">
                            <!-- Step Item -->
                            <a 
                                href={step.path}
                                class="flex items-center py-2 px-4 transition-all rounded-lg  duration-200 group {
                                    currentStep === step.number
                                        ? 'bg-background-400 text-foreground-950 '
                                        : currentStep > step.number
                                        ? 'bg-background-200 text-foreground-700'
                                        : 'bg-background-200 text-foreground-700 group-hover:underline'
                                }"
                            >
                                <!-- Step Number/Icon -->
                                <!-- <div class="flex-shrink-0 w-10 h-10 rounded-full flex items-center justify-center text-sm font-bold {
                                    currentStep === step.number
                                        ? 'bg-blue-500 text-white'
                                        : currentStep > step.number
                                        ? 'bg-green-500 text-white'
                                        : 'bg-gray-200 text-gray-600 group-hover:bg-gray-300'
                                }">
                                    {#if currentStep > step.number}
                                        ✓
                                    {:else if currentStep === step.number}
                                        {step.number}
                                    {:else}
                                        {step.number}
                                    {/if}
                                </div> -->

                                <!-- Step Content -->
                                <div class="flex-1 min-w-0">
                                    <div class="flex items-center space-x-2">
                                        <h3 class="font-medium text-sm">
                                            {step.title}
                                        </h3>
                                    </div>
                                </div>

                                <!-- Active Indicator -->
                                <!-- {#if currentStep === step.number}
                                    <div class="w-2 h-2 bg-blue-500 rounded-full animate-pulse"></div>
                                {/if} -->
                            </a>

                            {#if index < steps.length - 1}
                                <div class="flex-1 h-0.5 w-8 flex-shrink-0 mx-2 bg-background-400"></div>
                            {/if}
                        </div>
                    {/each}
                </div>

                <!-- Progress Bar -->
                <div class="">
                    <div class="w-full bg-background-100 rounded-full">
                        <div 
                            class="bg-background-400 h-1 transition-all duration-500 ease-out"
                            style="width: {currentStep === 0 ? 0 : (currentStep / steps.length) * 100}%"
                        ></div>
                    </div>
                </div>
            </div>
        </div>

        <!-- Main Content Area (scrollable) -->
        <main class="flex-1 overflow-hidden">
            <section class="h-full overflow-y-auto">
                {@render children()}
            </section>
        </main>
    </div>
{:else}
    <div class="flex items-center justify-center h-full">
        <div class="text-center">
            <span class="text-4xl mb-4 block">❌</span>
            <p class="text-gray-600">Property not found</p>
            <a href="/" class="text-blue-500 hover:text-blue-600 mt-2 inline-block">← Back to Dashboard</a>
        </div>
    </div>
{/if}
