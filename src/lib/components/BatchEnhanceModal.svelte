<script lang="ts">
  import { DatabaseService } from '$lib/services/databaseService';
  import { Dialog, DialogContent, DialogFooter } from '$lib/components/ui';
  import { Dialog as BitsDialog } from 'bits-ui';
  import type { EnhanceAnalysisResult, EnhanceRequest } from '$lib/types/database';
  import { onMount } from 'svelte';
  import BeforeAfterSlider from './BeforeAfterSlider.svelte';
  import { showSuccess, showError } from '$lib/stores/notification';

  interface Props {
    open?: boolean;
    folderPath: string;
    status: string;
    onClose: () => void;
    onComplete: () => void;
  }

  let {
    open = $bindable(true),
    folderPath,
    status,
    onClose,
    onComplete
  }: Props = $props();

  // Processing state
  let isAnalyzing = $state(false);
  let processingMessage = $state('');
  let results = $state<EnhanceAnalysisResult[]>([]);
  let selectedIndices = $state<Set<number>>(new Set());
  let error = $state('');
  let isApplying = $state(false);

  onMount(async () => {
    await analyzeImages();
  });

  async function analyzeImages() {
    try {
      isAnalyzing = true;
      processingMessage = 'Analyzing images for straightening and adjustments...';

      results = await DatabaseService.batchAnalyzeForEnhance(folderPath, status);

      // Pre-select images that need enhancement
      const needsEnhancementIndices = new Set<number>();
      results.forEach((result, index) => {
        if (result.needs_enhancement) {
          needsEnhancementIndices.add(index);
        }
      });
      selectedIndices = needsEnhancementIndices;

      isAnalyzing = false;
    } catch (err) {
      console.error('Failed to analyze images:', err);
      error = err instanceof Error ? err.message : String(err);
      isAnalyzing = false;
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

  function selectRecommended() {
    const needsEnhancementIndices = new Set<number>();
    results.forEach((result, index) => {
      if (result.needs_enhancement) {
        needsEnhancementIndices.add(index);
      }
    });
    selectedIndices = needsEnhancementIndices;
  }

  async function handleApply() {
    if (selectedIndices.size === 0) {
      error = 'Please select at least one image to enhance';
      return;
    }

    try {
      isApplying = true;
      error = '';

      const enhancements: EnhanceRequest[] = Array.from(selectedIndices).map((index) => ({
        filename: results[index].filename,
        original_path: results[index].original_path,
        rotation: results[index].straighten.rotation,
        brightness: results[index].adjustments.brightness,
        exposure: results[index].adjustments.exposure,
        contrast: results[index].adjustments.contrast,
        highlights: results[index].adjustments.highlights,
        shadows: results[index].adjustments.shadows
      }));

      const applyResults = await DatabaseService.batchApplyEnhancements(enhancements);

      const successCount = applyResults.filter((r) => r.success).length;
      const failCount = applyResults.filter((r) => !r.success).length;

      if (failCount === 0) {
        showSuccess(`Enhanced ${successCount} images successfully`);
        onComplete();
        onClose();
      } else if (successCount > 0) {
        showSuccess(`Enhanced ${successCount} images, ${failCount} failed`);
        onComplete();
        onClose();
      } else {
        const firstError = applyResults.find((r) => r.error)?.error;
        showError(firstError || 'Failed to apply enhancements');
      }
    } catch (err) {
      console.error('Failed to apply enhancements:', err);
      showError(err instanceof Error ? err.message : 'Failed to apply enhancements');
    } finally {
      isApplying = false;
    }
  }

  function handleCancel() {
    onClose();
  }

  function handleOpenChange(newOpen: boolean) {
    if (!newOpen && !isAnalyzing && !isApplying) {
      handleCancel();
    }
  }

  let recommendedCount = $derived(results.filter((r) => r.needs_enhancement).length);
  let selectedCount = $derived(selectedIndices.size);
</script>

<Dialog bind:open onOpenChange={handleOpenChange}>
  <DialogContent class="flex max-h-[95vh] w-[95vw] max-w-[1600px] flex-col overflow-hidden rounded-xl">
    <!-- Modal Header -->
    <div class="border-background-200 flex items-center justify-between border-b px-5 py-4">
      <div>
        <BitsDialog.Title class="text-foreground-900 text-lg font-semibold">
          Auto-Enhance Images
        </BitsDialog.Title>
        <p class="text-foreground-500 mt-0.5 text-sm">
          {#if isAnalyzing}
            {processingMessage}
          {:else if results.length > 0}
            {recommendedCount} of {results.length} images recommended for enhancement
          {:else}
            Analyzing images...
          {/if}
        </p>
      </div>
      <BitsDialog.Close
        onclick={handleCancel}
        disabled={isAnalyzing || isApplying}
        class="text-foreground-500 hover:text-foreground-700 transition-colors disabled:opacity-50"
      >
        <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M6 18L18 6M6 6l12 12"
          />
        </svg>
      </BitsDialog.Close>
    </div>

    <!-- Modal Content -->
    <div class="flex-1 overflow-y-auto p-5">
      {#if isAnalyzing}
        <div class="flex flex-col items-center justify-center py-16">
          <div
            class="border-accent-500 h-10 w-10 animate-spin rounded-full border-4 border-t-transparent"
          ></div>
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
            onclick={selectRecommended}
            class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 rounded-md border px-3 py-1.5 text-xs font-medium transition-colors"
          >
            Select Recommended ({recommendedCount})
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
        <div class="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
          {#each results as result, index}
            <div class="relative">
              <p
                class="text-foreground-700 mb-1.5 truncate text-xs font-medium"
                title={result.filename}
              >
                {result.filename}
              </p>
              <BeforeAfterSlider
                beforeUrl={result.original_preview_base64}
                afterUrl={result.preview_base64}
                selected={selectedIndices.has(index)}
                onToggleSelect={() => toggleSelection(index)}
                confidence={result.combined_confidence}
                rotationApplied={result.straighten.rotation}
                needsCorrection={result.needs_enhancement}
                brightness={result.adjustments.brightness}
                exposure={result.adjustments.exposure}
                contrast={result.adjustments.contrast}
                highlights={result.adjustments.highlights}
                shadows={result.adjustments.shadows}
              />
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
        disabled={isAnalyzing || isApplying}
        class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 rounded-md border px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50"
      >
        Cancel
      </button>
      <button
        onclick={handleApply}
        disabled={isAnalyzing || isApplying || selectedIndices.size === 0}
        class="bg-accent-500 hover:bg-accent-600 rounded-md px-4 py-2 text-sm font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
      >
        {#if isApplying}
          Applying...
        {:else}
          Apply to {selectedCount} Images
        {/if}
      </button>
    </DialogFooter>
  </DialogContent>
</Dialog>
