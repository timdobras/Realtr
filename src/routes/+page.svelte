<script lang="ts">
	import { onMount } from 'svelte';
	import { DatabaseService } from '$lib/services/databaseService';
	import type { Property } from '$lib/types/database';
	import { formatRelativeTime, isToday, isValidDate } from '$lib/utils/dateUtils';
	
	// Reactive state
	let stats = $state({
		totalProperties: 0,
		inProgress: 0,
		completed: 0,
		todayProcessed: 0
	});
	
	let recentProperties = $state<Property[]>([]);
	let isLoading = $state(true);
	let error = $state<string>('');
	
	onMount(async () => {
		await loadDashboardData();
	});
	
	async function loadDashboardData() {
		try {
			isLoading = true;
			error = '';
			
			// Load all properties
			const properties = await DatabaseService.getProperties();
			
			// Calculate stats
			const completed = properties.filter(p => p.completed);
			const inProgress = properties.filter(p => !p.completed);
			
			// Calculate today's processed (completed today)
			const todayProcessed = completed.filter(p => 
				isValidDate(p.updated_at) && isToday(p.updated_at)
			).length;
			
			stats = {
				totalProperties: properties.length,
				inProgress: inProgress.length,
				completed: completed.length,
				todayProcessed
			};
			
			// Get recent properties (last 5)
			recentProperties = properties.slice(0, 5);
			
		} catch (err) {
			console.error('Error loading dashboard data:', err);
			error = 'Failed to load dashboard data';
		} finally {
			isLoading = false;
		}
	}
	
	function formatDate(timestamp: number): string {
		if (!isValidDate(timestamp)) {
			return 'Unknown date';
		}
		return formatRelativeTime(timestamp);
	}
	
	async function markAsCompleted(propertyId: number) {
		try {
			await DatabaseService.updatePropertyStatus(propertyId, true);
			await loadDashboardData(); // Refresh data
		} catch (err) {
			console.error('Error updating property status:', err);
		}
	}
</script>

<div class="space-y-6 p-6">
	<!-- Error Message -->
	{#if error}
		<div class="bg-red-50 border border-red-200 rounded-lg p-4">
			<div class="flex items-center space-x-2">
				<span class="text-red-600">❌</span>
				<p class="text-red-800 font-medium">{error}</p>
			</div>
		</div>
	{/if}
	
	<!-- Welcome Section -->
	<div class="bg-gradient-to-r from-blue-600 to-blue-700 rounded-lg p-8 text-white">
		<h1 class="text-3xl font-bold mb-2">Welcome back!</h1>
		<p class="text-blue-100">Ready to process some amazing real estate photos?</p>
		{#if isLoading}
			<div class="mt-4 flex items-center space-x-2">
				<div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
				<span class="text-blue-100">Loading dashboard data...</span>
			</div>
		{/if}
	</div>
	
	<!-- Stats Grid -->
	<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
		<div class="bg-background-100 rounded-lg p-6 shadow-sm border border-background-300">
			<div class="flex items-center justify-between">
				<div>
					<p class="text-sm text-foreground-600">Total Properties</p>
					<p class="text-3xl font-bold text-foreground-900">{stats.totalProperties}</p>
				</div>
				<div class="w-12 h-12 bg-blue-100 rounded-lg flex items-center justify-center">
					<span class="text-2xl">🏢</span>
				</div>
			</div>
		</div>
		
		<div class="bg-background-100 rounded-lg p-6 shadow-sm border border-background-300">
			<div class="flex items-center justify-between">
				<div>
					<p class="text-sm text-foreground-600">In Progress</p>
					<p class="text-3xl font-bold text-orange-600">{stats.inProgress}</p>
				</div>
				<div class="w-12 h-12 bg-orange-100 rounded-lg flex items-center justify-center">
					<span class="text-2xl">⏳</span>
				</div>
			</div>
		</div>
		
		<div class="bg-background-100 rounded-lg p-6 shadow-sm border border-background-300">
			<div class="flex items-center justify-between">
				<div>
					<p class="text-sm text-foreground-600">Completed</p>
					<p class="text-3xl font-bold text-green-600">{stats.completed}</p>
				</div>
				<div class="w-12 h-12 bg-green-100 rounded-lg flex items-center justify-center">
					<span class="text-2xl">✅</span>
				</div>
			</div>
		</div>
		
		<div class="bg-background-100 rounded-lg p-6 shadow-sm border border-background-300">
			<div class="flex items-center justify-between">
				<div>
					<p class="text-sm text-foreground-600">Today's Work</p>
					<p class="text-3xl font-bold text-purple-600">{stats.todayProcessed}</p>
				</div>
				<div class="w-12 h-12 bg-purple-100 rounded-lg flex items-center justify-center">
					<span class="text-2xl">📸</span>
				</div>
			</div>
		</div>
	</div>
	
	<!-- Recent Properties -->
	<div class="bg-background-100 rounded-lg shadow-sm border border-background-300">
		<div class="p-6 border-b border-background-300 flex items-center justify-between">
			<h2 class="text-xl font-semibold text-foreground-900">Recent Properties</h2>
			<a href="/properties" class="text-blue-600 hover:text-blue-700 text-sm font-medium">
				View all →
			</a>
		</div>
		<div class="p-6">
			{#if isLoading}
				<div class="space-y-4">
					{#each Array(3) as _}
						<div class="animate-pulse">
							<div class="flex items-center space-x-4">
								<div class="w-10 h-10 bg-gray-300 rounded-lg"></div>
								<div class="flex-1 space-y-2">
									<div class="h-4 bg-gray-300 rounded w-3/4"></div>
									<div class="h-3 bg-gray-300 rounded w-1/2"></div>
								</div>
							</div>
						</div>
					{/each}
				</div>
			{:else if recentProperties.length === 0}
				<div class="text-center py-12">
					<span class="text-6xl">📁</span>
					<h3 class="text-lg font-medium text-foreground-900 mt-4">No properties yet</h3>
					<p class="text-foreground-500 mt-2">Start by adding your first property</p>
					<a href="/properties" class="mt-4 inline-block btn-primary">
						Add Property
					</a>
				</div>
			{:else}
				<div class="space-y-4">
					{#each recentProperties as property}
						<div class="flex items-center justify-between p-4 bg-background-200 rounded-lg hover:bg-background-300 transition-colors">
							<div class="flex items-center space-x-4">
								<div class="w-10 h-10 bg-blue-100 rounded-lg flex items-center justify-center">
									<span class="text-blue-600">🏠</span>
								</div>
								<div>
									<h3 class="font-medium text-foreground-900">{property.name}</h3>
									<p class="text-sm text-foreground-500">{property.city}</p>
								</div>
							</div>
							<div class="flex items-center space-x-4">
								{#if !property.completed}
									<button
										onclick={() => markAsCompleted(property.id!)}
										class="text-xs bg-orange-100 text-orange-700 px-2 py-1 rounded-full hover:bg-orange-200 transition-colors"
									>
										Mark Complete
									</button>
								{:else}
									<span class="px-3 py-1 text-xs font-medium rounded-full bg-green-100 text-green-700">
										Completed
									</span>
								{/if}
								<span class="text-sm text-foreground-500">{formatDate(property.updated_at)}</span>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</div>
