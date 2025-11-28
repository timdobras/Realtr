<script lang="ts">
	import { Combobox } from 'bits-ui';
	import type { City } from '$lib/types/database';

	interface Props {
		value: string;
		cities: City[];
		placeholder?: string;
		required?: boolean;
	}

	let {
		value = $bindable(),
		cities,
		placeholder = 'Select a city...',
		required = false
	}: Props = $props();

	let searchValue = $state('');

	// Filter cities based on search input
	let filteredCities = $derived(
		searchValue === ''
			? cities.slice(0, 5)
			: cities.filter((c) => c.name.toLowerCase().includes(searchValue.toLowerCase())).slice(0, 5)
	);

	// Check if current input is a new city (not in existing list)
	let isNewCity = $derived(
		searchValue.trim() !== '' &&
			!cities.some((c) => c.name.toLowerCase() === searchValue.toLowerCase())
	);

	function handleValueChange(selectedValue: string | undefined) {
		if (selectedValue) {
			value = selectedValue;
			searchValue = selectedValue;
		}
	}
</script>

<Combobox.Root
	type="single"
	bind:value
	onValueChange={handleValueChange}
	onOpenChange={(o) => {
		if (!o && searchValue.trim()) {
			// When closing, keep the typed value even if not selected
			value = searchValue;
		}
		if (!o) {
			// Reset search on close if value was selected
			if (value) searchValue = value;
		}
	}}
>
	<div class="relative">
		<Combobox.Input
			{placeholder}
			{required}
			oninput={(e) => {
				searchValue = e.currentTarget.value;
				value = e.currentTarget.value;
			}}
			class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full rounded-md border px-3 py-2 pr-8 text-sm transition-colors focus:ring-1 focus:outline-none"
		/>
		<Combobox.Trigger class="text-foreground-500 hover:text-foreground-700 absolute end-2 top-1/2 -translate-y-1/2">
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
			</svg>
		</Combobox.Trigger>
	</div>
	<Combobox.Portal>
		<Combobox.Content
			sideOffset={4}
			class="border-background-300 bg-background-50 z-[100] max-h-48 w-[var(--bits-combobox-anchor-width)] overflow-y-auto rounded-md border shadow-lg"
		>
			<Combobox.Viewport class="p-1">
				{#each filteredCities as city (city.id)}
					<Combobox.Item
						value={city.name}
						label={city.name}
						class="data-[highlighted]:bg-background-200 hover:bg-background-100 flex w-full cursor-pointer items-center justify-between rounded px-2 py-1.5 text-sm transition-colors outline-none"
					>
						{#snippet children({ selected })}
							<span class="text-foreground-900 font-medium">{city.name}</span>
							<div class="flex items-center gap-2">
								<span class="text-foreground-500 text-xs">{city.usageCount}</span>
								{#if selected}
									<svg class="text-accent-500 h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
										<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
									</svg>
								{/if}
							</div>
						{/snippet}
					</Combobox.Item>
				{:else}
					{#if !isNewCity}
						<span class="text-foreground-500 block px-2 py-1.5 text-center text-xs">No cities found</span>
					{/if}
				{/each}

				{#if isNewCity}
					<div
						class="text-foreground-500 {filteredCities.length > 0
							? 'border-background-200 mt-1 border-t pt-1'
							: ''} px-2 py-1.5 text-center text-xs"
					>
						New city will be created
					</div>
				{/if}
			</Combobox.Viewport>
		</Combobox.Content>
	</Combobox.Portal>
</Combobox.Root>
