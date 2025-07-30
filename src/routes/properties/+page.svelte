<script lang="ts">
	import { onMount } from 'svelte';
	import { DatabaseService } from '$lib/services/databaseService';
	import PropertyCard from '$lib/components/PropertyCard.svelte';
	import AddPropertyModal from '$lib/components/AddPropertyModal.svelte';
	import type { Property } from '$lib/types/database';
	
	let properties = $state<Property[]>([]);
	let filteredProperties = $state<Property[]>([]);
	let isLoading = $state(true);
	let error = $state<string>('');
	let showAddModal = $state(false);
	
	// Filters
	let searchQuery = $state('');
	let statusFilter = $state<'all' | 'completed' | 'in_progress'>('all');
	let cityFilter = $state('');
	
	// Get unique cities for filter
	let cities = $derived(Array.from(new Set(properties.map(p => p.city))).sort());
	
	onMount(async () => {
		await loadProperties();
	});
	
	async function loadProperties() {
		try {
			isLoading = true;
			error = '';
			properties = await DatabaseService.getProperties();
			applyFilters();
		} catch (err) {
			console.error('Error loading properties:', err);
			error = 'Failed to load properties';
		} finally {
			isLoading = false;
		}
	}
	
	function applyFilters() {
		filteredProperties = properties.filter(property => {
			// Text search
			const matchesSearch = searchQuery === '' || 
				property.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
				property.city.toLowerCase().includes(searchQuery.toLowerCase());
			
			// Status filter
			const matchesStatus = statusFilter === 'all' ||
				(statusFilter === 'completed' && property.completed) ||
				(statusFilter === 'in_progress' && !property.completed);
			
			// City filter
			const matchesCity = cityFilter === '' || property.city === cityFilter;
			
			return matchesSearch && matchesStatus && matchesCity;
		});
	}
	
	// Watch for filter changes
	$effect(() => {
		searchQuery; statusFilter; cityFilter; // Dependencies
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
</script>

<div class="space-y-6 p-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div>
			<h1 class="text-2xl font-bold text-foreground-900">Properties</h1>
			<p class="text-foreground-600 mt-1">Manage your real estate photo projects</p>
		</div>
		<button
			onclick={() => showAddModal = true}
			class="btn-primary flex items-center space-x-2"
		>
			<span>➕</span>
			<span>Add Property</span>
		</button>
	</div>
	
	<!-- Filters -->
	<div class="bg-background-100 rounded-lg p-6 shadow-sm border border-background-300">
		<div class="grid grid-cols-1 md:grid-cols-4 gap-4">
			<!-- Search -->
			<div>
				<label class="block text-sm font-medium text-foreground-700 mb-2">Search</label>
				<input
					type="text"
					bind:value={searchQuery}
					placeholder="Search properties..."
					class="w-full px-3 py-2 border border-background-400 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
				/>
			</div>
			
			<!-- Status Filter -->
			<div>
				<label class="block text-sm font-medium text-foreground-700 mb-2">Status</label>
				<select
					bind:value={statusFilter}
					class="w-full px-3 py-2 border bg-background-100 border-background-400 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
				>
					<option value="all">All Properties</option>
					<option value="in_progress">In Progress</option>
					<option value="completed">Completed</option>
				</select>
			</div>
			
			<!-- City Filter -->
			<div>
				<label class="block text-sm font-medium text-foreground-700 mb-2">City</label>
				<select
					bind:value={cityFilter}
					class="w-full px-3 py-2 border bg-background-100 border-background-400 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
				>
					<option value="">All Cities</option>
					{#each cities as city}
						<option value={city}>{city}</option>
					{/each}
				</select>
			</div>
			
			<!-- Results Count -->
			<div class="flex items-end">
				<div class="text-sm text-foreground-600">
					Showing {filteredProperties.length} of {properties.length} properties
				</div>
			</div>
		</div>
	</div>
	
	<!-- Error Message -->
	{#if error}
		<div class="bg-red-50 border border-red-200 rounded-lg p-4">
			<div class="flex items-center space-x-2">
				<span class="text-red-600">❌</span>
				<p class="text-red-800 font-medium">{error}</p>
			</div>
		</div>
	{/if}
	
	<!-- Properties Grid -->
	{#if isLoading}
		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
			{#each Array(6) as _}
				<div class="bg-background-100 rounded-lg p-6 shadow-sm border border-background-300 animate-pulse">
					<div class="space-y-4">
						<div class="h-4 bg-gray-300 rounded w-3/4"></div>
						<div class="h-3 bg-gray-300 rounded w-1/2"></div>
						<div class="h-8 bg-gray-300 rounded w-full"></div>
					</div>
				</div>
			{/each}
		</div>
	{:else if filteredProperties.length === 0}
		<div class="text-center py-12">
			<span class="text-6xl">🔍</span>
			<h3 class="text-lg font-medium text-foreground-900 mt-4">
				{properties.length === 0 ? 'No properties yet' : 'No properties match your filters'}
			</h3>
			<p class="text-foreground-500 mt-2">
				{properties.length === 0 
					? 'Start by adding your first property' 
					: 'Try adjusting your search criteria'}
			</p>
			{#if properties.length === 0}
				<button
					onclick={() => showAddModal = true}
					class="mt-4 btn-primary"
				>
					Add Property
				</button>
			{/if}
		</div>
	{:else}
		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
			{#each filteredProperties as property}
				<PropertyCard 
					{property} 
					onUpdate={onPropertyUpdated}
					onDelete={onPropertyDeleted}
				/>
			{/each}
		</div>
	{/if}
</div>

<!-- Add Property Modal -->
{#if showAddModal}
	<AddPropertyModal 
		onClose={() => showAddModal = false}
		onPropertyAdded={onPropertyAdded}
	/>
{/if}
