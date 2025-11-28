<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { DatabaseService } from '$lib/services/databaseService';
	import type { Property, PropertyStatus } from '$lib/types/database';
	import { formatDate, isValidDate } from '$lib/utils/dateUtils';
	import SetCodeModal from './SetCodeModal.svelte';
	import ConfirmDialog from './ConfirmDialog.svelte';

	interface Props {
		property: Property;
		thumbnails?: string[];
		onUpdate: () => void;
		onDelete: () => void;
	}

	let { property, thumbnails = [], onUpdate, onDelete }: Props = $props();

	let isDeleting = $state(false);
	let showDeleteConfirm = $state(false);
	let showActions = $state(false);
	let openFolderError = $state('');
	let showCodeModal = $state(false);
	let copySuccess = $state(false);

	function getStatusConfig(status: PropertyStatus) {
		const configs = {
			NEW: { label: 'New', classes: 'text-blue-300' },
			DONE: { label: 'Done', classes: 'text-green-300' },
			NOT_FOUND: { label: 'Not Found', classes: 'text-yellow-300' },
			ARCHIVE: { label: 'Archived', classes: 'text-gray-300' }
		};
		return configs[status] || configs.NEW;
	}

	async function openPropertyFolder() {
		try {
			openFolderError = '';
			const result: any = await invoke('open_property_folder', {
				folderPath: property.folder_path,
				status: property.status
			});

			if (!result.success) {
				openFolderError = result.error || 'Failed to open folder';
				setTimeout(() => (openFolderError = ''), 3000);
			}
		} catch (error) {
			console.error('Failed to open property folder:', error);
			openFolderError = 'Failed to open folder';
			setTimeout(() => (openFolderError = ''), 3000);
		}
	}

	async function confirmDelete() {
		try {
			isDeleting = true;
			await DatabaseService.deleteProperty(property.id!);
			showDeleteConfirm = false;
			onDelete();
		} catch (error) {
			console.error('Failed to delete property:', error);
		} finally {
			isDeleting = false;
		}
	}

	function formatDisplayDate(timestamp: number): string {
		if (!isValidDate(timestamp)) {
			return 'Unknown date';
		}
		return formatDate(timestamp);
	}

	async function copyPath() {
		try {
			const fullPath = await invoke<string>('get_full_property_path', {
				folderPath: property.folder_path,
				status: property.status
			});
			await navigator.clipboard.writeText(fullPath);
			copySuccess = true;
			setTimeout(() => (copySuccess = false), 2000);
		} catch (error) {
			console.error('Failed to copy path:', error);
			openFolderError = 'Failed to copy path';
			setTimeout(() => (openFolderError = ''), 3000);
		}
	}
</script>

<div
	class="bg-background-50 border-background-200 hover:border-background-300 group relative rounded border transition-colors"
	role="group"
	aria-label="Property card for {property.name}"
	onmouseenter={() => (showActions = true)}
	onmouseleave={() => (showActions = false)}
>
	<a href="/properties/{property.id}" class="block p-3">
		<!-- Thumbnail Grid -->
		<div class="bg-background-100 mb-3 aspect-[3/2] overflow-hidden rounded">
			{#if thumbnails.length === 0}
				<div class="flex h-full items-center justify-center">
					<div class="text-center">
						<svg
							class="text-foreground-400 mx-auto mb-1 h-8 w-8"
							fill="none"
							stroke="currentColor"
							viewBox="0 0 24 24"
						>
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
							/>
						</svg>
						<p class="text-foreground-500 text-xs">No images</p>
					</div>
				</div>
			{:else}
				<div class="grid h-full grid-cols-3 grid-rows-2 gap-0.5">
					{#each thumbnails.slice(0, 6) as thumbnail, i}
						<div class="bg-background-200 overflow-hidden">
							<img src={thumbnail} alt="Thumbnail {i + 1}" class="h-full w-full object-cover" />
						</div>
					{/each}
					{#if thumbnails.length < 6}
						{#each Array(6 - thumbnails.length) as _, i}
							<div class="bg-background-200"></div>
						{/each}
					{/if}
				</div>
			{/if}
		</div>

		<div class="flex flex-row items-center justify-between gap-4">
			<h3
				class="text-foreground-900 group-hover:text-accent-600 truncate text-sm font-medium transition-colors"
			>
				{property.name}
			</h3>
			<div class="flex items-center gap-2">
				{#if property.code}
					<span class="bg-accent-100 text-accent-700 rounded px-1.5 py-0.5 text-[10px] font-medium">
						{property.code}
					</span>
				{/if}
				<span class="inline-flex items-center text-xs {getStatusConfig(property.status).classes}">
					{getStatusConfig(property.status).label}
				</span>
			</div>
		</div>

		<div class="flex items-center justify-between">
			<p class="text-foreground-500 text-xs">{property.city}</p>

			<span class="text-foreground-500 text-[10px]">
				{formatDisplayDate(property.updated_at)}
			</span>
		</div>
	</a>

	<!-- Error Message -->
	{#if openFolderError}
		<div class="border-background-200 border-t px-3 py-2">
			<p class="text-foreground-900 text-xs">{openFolderError}</p>
		</div>
	{/if}

	<!-- Actions Overlay (shown on hover) -->
	{#if showActions}
		<div
			class="bg-background-50/95 border-background-200 absolute inset-x-0 bottom-0 border-t px-3 py-2 backdrop-blur-sm"
		>
			<div class="flex items-center justify-between gap-2">
				<h3 class="text-foreground-900 truncate text-sm font-medium transition-colors">
					{property.name}
				</h3>
				<div class="flex flex-row items-center gap-2">
					<button
						onclick={() => (showCodeModal = true)}
						class="text-foreground-600 hover:bg-background-100 hover:text-foreground-900 p-1.5 transition-colors"
						title={property.code ? 'Edit Code' : 'Add Code'}
						aria-label={property.code ? 'Edit listing code' : 'Add listing code'}
					>
						<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M7 20l4-16m2 16l4-16M6 9h14M4 15h14"
							/>
						</svg>
					</button>

					<button
						onclick={copyPath}
						class="{copySuccess
							? 'text-green-600'
							: 'text-foreground-600'} hover:bg-background-100 hover:text-foreground-900 p-1.5 transition-colors"
						title={copySuccess ? 'Copied!' : 'Copy Path'}
						aria-label="Copy property path to clipboard"
					>
						{#if copySuccess}
							<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									stroke-width="2"
									d="M5 13l4 4L19 7"
								/>
							</svg>
						{:else}
							<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									stroke-width="2"
									d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
								/>
							</svg>
						{/if}
					</button>

					<button
						onclick={openPropertyFolder}
						class="text-foreground-600 hover:bg-background-100 hover:text-foreground-900 p-1.5 transition-colors"
						title="Open Folder"
						aria-label="Open property folder"
					>
						<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"
							/>
						</svg>
					</button>

					<button
						onclick={() => (showDeleteConfirm = true)}
						class="text-foreground-600 hover:text-foreground-900 hover:bg-background-100 p-1.5 transition-colors"
						title="Delete"
						aria-label="Delete property"
					>
						<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
							/>
						</svg>
					</button>
				</div>
			</div>
		</div>
	{/if}
</div>

<!-- Delete Confirmation Modal -->
<ConfirmDialog
	bind:open={showDeleteConfirm}
	title="Delete Property"
	message="Delete &quot;{property.name}&quot;? This cannot be undone."
	confirmText={isDeleting ? 'Deleting...' : 'Delete'}
	onConfirm={confirmDelete}
	onCancel={() => (showDeleteConfirm = false)}
/>

<!-- Set Code Modal -->
<SetCodeModal
	bind:open={showCodeModal}
	propertyId={property.id!}
	propertyName={property.name}
	currentCode={property.code}
	onClose={() => (showCodeModal = false)}
	onCodeSet={() => {
		showCodeModal = false;
		onUpdate();
	}}
/>
