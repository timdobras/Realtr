<script lang="ts">
	import { DatabaseService } from '$lib/services/databaseService';
	import { Dialog, DialogContent, DialogFooter } from '$lib/components/ui';
	import { Dialog as BitsDialog } from 'bits-ui';
	import type { AcceptedCorrection, CorrectionResult } from '$lib/types/database';
	import { onMount } from 'svelte';
	import BeforeAfterSlider from './BeforeAfterSlider.svelte';
	import { showSuccess, showError } from '$lib/stores/notification';

	interface Props {
		open?: boolean;
		folderPath: string;
		status: string;
		propertyId: number;
		onClose: () => void;
		onComplete: () => void;
	}

	let { open = $bindable(true), folderPath, status, propertyId, onClose, onComplete }: Props = $props();

	// Processing state
	let isProcessing = $state(false);
	let processingMessage = $state('');
	let results = $state<CorrectionResult[]>([]);
	let selectedIndices = $state<Set<number>>(new Set());
	let originalPreviews = $state<Map<number, string>>(new Map());
	let error = $state('');
	let isApplying = $state(false);

	onMount(async () => {
		await startProcessing();
	});

	async function startProcessing() {
		try {
			isProcessing = true;
			processingMessage = 'Detecting vertical lines and calculating corrections...';
			results = await DatabaseService.processImagesForPerspective(folderPath, status, propertyId);

			const needsCorrectionIndices = new Set<number>();
			results.forEach((result, index) => {
				if (result.needs_correction) {
					needsCorrectionIndices.add(index);
				}
			});
			selectedIndices = needsCorrectionIndices;

			processingMessage = 'Loading previews...';
			for (let i = 0; i < results.length; i++) {
				try {
					const originalBase64 = await DatabaseService.getOriginalImageForComparison(results[i].original_path);
					originalPreviews.set(i, originalBase64);
					originalPreviews = new Map(originalPreviews);
				} catch (err) {
					console.error('Failed to load original preview:', err);
				}
			}

			isProcessing = false;
		} catch (err) {
			console.error('Failed to process images:', err);
			error = err instanceof Error ? err.message : String(err);
			isProcessing = false;
		}
	}

	function toggleSelection(index: number) {
		const newSelection = new Set(selectedIndices);
		if (newSelection.has(index)) {
			newSelection.delete(index);
		} else {
			newSelection.add(index);
		}
		selectedIndices = newSelection;
	}

	function selectAll() {
		const allIndices = new Set<number>();
		results.forEach((_, index) => allIndices.add(index));
		selectedIndices = allIndices;
	}

	function deselectAll() {
		selectedIndices = new Set();
	}

	function selectNeedingCorrection() {
		const needsCorrectionIndices = new Set<number>();
		results.forEach((result, index) => {
			if (result.needs_correction) {
				needsCorrectionIndices.add(index);
			}
		});
		selectedIndices = needsCorrectionIndices;
	}

	async function handleAccept() {
		if (selectedIndices.size === 0) {
			error = 'Please select at least one image to apply corrections';
			return;
		}

		try {
			isApplying = true;
			error = '';

			const corrections: AcceptedCorrection[] = Array.from(selectedIndices).map((index) => ({
				original_path: results[index].original_path,
				corrected_temp_path: results[index].corrected_temp_path
			}));

			const result = await DatabaseService.acceptPerspectiveCorrections(corrections);

			if (result.success) {
				showSuccess(`Applied perspective corrections to ${corrections.length} images`);
				onComplete();
			} else {
				showError(result.error || 'Failed to apply corrections');
			}
		} catch (err) {
			console.error('Failed to apply corrections:', err);
			showError(err instanceof Error ? err.message : 'Failed to apply corrections');
		} finally {
			isApplying = false;
		}
	}

	async function handleCancel() {
		try {
			await DatabaseService.cleanupPerspectiveTemp();
		} catch (err) {
			console.error('Failed to cleanup temp files:', err);
		}
		onClose();
	}

	function handleOpenChange(newOpen: boolean) {
		if (!newOpen && !isProcessing && !isApplying) {
			handleCancel();
		}
	}

	let needsCorrectionCount = $derived(results.filter((r) => r.needs_correction).length);

	let selectedCount = $derived(selectedIndices.size);
</script>

<Dialog bind:open onOpenChange={handleOpenChange}>
	<DialogContent class="flex max-h-[90vh] max-w-5xl flex-col overflow-hidden rounded-xl">
		<!-- Modal Header -->
		<div class="border-background-200 flex items-center justify-between border-b px-5 py-4">
			<div>
				<BitsDialog.Title class="text-foreground-900 text-lg font-semibold">
					Auto-Straighten Images
				</BitsDialog.Title>
				<p class="text-foreground-500 mt-0.5 text-sm">
					{#if isProcessing}
						{processingMessage}
					{:else if results.length > 0}
						{needsCorrectionCount} of {results.length} images need correction
					{:else}
						Analyzing images...
					{/if}
				</p>
			</div>
			<BitsDialog.Close
				onclick={handleCancel}
				disabled={isProcessing || isApplying}
				class="text-foreground-500 hover:text-foreground-700 transition-colors disabled:opacity-50"
			>
				<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
				</svg>
			</BitsDialog.Close>
		</div>

		<!-- Modal Content -->
		<div class="flex-1 overflow-y-auto p-5">
			{#if isProcessing}
				<div class="flex flex-col items-center justify-center py-16">
					<div class="border-accent-500 h-10 w-10 animate-spin rounded-full border-4 border-t-transparent"></div>
					<p class="text-foreground-600 mt-4 text-sm">{processingMessage}</p>
				</div>
			{:else if error && results.length === 0}
				<div class="flex flex-col items-center justify-center py-16">
					<div class="rounded-full bg-red-100 p-3">
						<svg class="h-8 w-8 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
							/>
						</svg>
					</div>
					<p class="mt-3 text-sm text-red-600">{error}</p>
				</div>
			{:else if results.length === 0}
				<div class="flex flex-col items-center justify-center py-16">
					<p class="text-foreground-500 text-sm">No images found in INTERNET folder</p>
				</div>
			{:else}
				<!-- Selection controls -->
				<div class="mb-4 flex flex-wrap items-center gap-2">
					<button
						onclick={selectAll}
						class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors"
					>
						Select All ({results.length})
					</button>
					<button
						onclick={selectNeedingCorrection}
						class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors"
					>
						Select Needing Correction ({needsCorrectionCount})
					</button>
					<button
						onclick={deselectAll}
						class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors"
					>
						Deselect All
					</button>
					<span class="text-foreground-500 ml-auto text-sm">
						{selectedCount} selected
					</span>
				</div>

				<!-- Image grid -->
				<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
					{#each results as result, index}
						<div class="relative">
							<p class="text-foreground-700 mb-1.5 truncate text-xs font-medium" title={result.original_filename}>
								{result.original_filename}
							</p>
							{#if originalPreviews.has(index) && result.corrected_preview_base64}
								<BeforeAfterSlider
									beforeUrl={originalPreviews.get(index) || ''}
									afterUrl={result.corrected_preview_base64}
									selected={selectedIndices.has(index)}
									onToggleSelect={() => toggleSelection(index)}
									confidence={result.confidence}
									rotationApplied={result.rotation_applied}
									needsCorrection={result.needs_correction}
								/>
							{:else}
								<div class="bg-background-200 flex aspect-[4/3] items-center justify-center rounded-lg">
									<div class="border-accent-500 h-6 w-6 animate-spin rounded-full border-2 border-t-transparent"></div>
								</div>
							{/if}
						</div>
					{/each}
				</div>
			{/if}

			{#if error && results.length > 0}
				<div class="mt-4 rounded-lg border border-red-300 bg-red-50 px-3 py-2">
					<p class="text-sm text-red-800">{error}</p>
				</div>
			{/if}
		</div>

		<!-- Modal Footer -->
		<DialogFooter>
			<button
				onclick={handleCancel}
				disabled={isProcessing || isApplying}
				class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 rounded-md border px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50"
			>
				Cancel
			</button>
			<button
				onclick={handleAccept}
				disabled={isProcessing || isApplying || selectedIndices.size === 0}
				class="bg-accent-500 hover:bg-accent-600 rounded-md px-4 py-2 text-sm font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
			>
				{#if isApplying}
					Applying...
				{:else}
					Accept Selected ({selectedCount})
				{/if}
			</button>
		</DialogFooter>
	</DialogContent>
</Dialog>
