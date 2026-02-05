<script lang="ts">
  interface Props {
    beforeUrl: string;
    afterUrl: string;
    selected?: boolean;
    onToggleSelect?: () => void;
    confidence?: number;
    rotationApplied?: number;
    needsCorrection?: boolean;
    // Adjustment values (for batch enhance)
    brightness?: number;
    exposure?: number;
    contrast?: number;
  }

  let {
    beforeUrl,
    afterUrl,
    selected = false,
    onToggleSelect,
    confidence = 0,
    rotationApplied = 0,
    needsCorrection = true,
    brightness = 0,
    exposure = 0,
    contrast = 0
  }: Props = $props();

  // Check if any adjustments will be applied
  let hasAdjustments = $derived(
    Math.abs(brightness) > 0 || Math.abs(exposure) > 0 || Math.abs(contrast) > 0
  );

  // Format adjustment value with sign
  function formatAdj(value: number, label: string): string {
    if (Math.abs(value) < 1) return '';
    const sign = value > 0 ? '+' : '';
    return `${label}${sign}${value}`;
  }

  let sliderPosition = $state(50);
  let containerRef: HTMLDivElement | null = $state(null);
  let isDragging = $state(false);

  function handleMouseDown(e: MouseEvent) {
    e.preventDefault();
    isDragging = true;
    updateSliderPosition(e);
  }

  function handleMouseMove(e: MouseEvent) {
    if (!isDragging || !containerRef) return;
    updateSliderPosition(e);
  }

  function handleMouseUp() {
    isDragging = false;
  }

  function updateSliderPosition(e: MouseEvent) {
    if (!containerRef) return;
    const rect = containerRef.getBoundingClientRect();
    const x = e.clientX - rect.left;
    sliderPosition = Math.max(0, Math.min(100, (x / rect.width) * 100));
  }

  function handleTouchStart(e: TouchEvent) {
    e.preventDefault();
    isDragging = true;
    updateTouchPosition(e);
  }

  function handleTouchMove(e: TouchEvent) {
    if (!isDragging || !containerRef) return;
    updateTouchPosition(e);
  }

  function handleTouchEnd() {
    isDragging = false;
  }

  function updateTouchPosition(e: TouchEvent) {
    if (!containerRef || !e.touches[0]) return;
    const rect = containerRef.getBoundingClientRect();
    const x = e.touches[0].clientX - rect.left;
    sliderPosition = Math.max(0, Math.min(100, (x / rect.width) * 100));
  }

  // Get confidence level label and color
  function getConfidenceInfo(conf: number): { label: string; colorClass: string } {
    if (conf >= 0.7) return { label: 'High', colorClass: 'bg-green-500' };
    if (conf >= 0.4) return { label: 'Medium', colorClass: 'bg-yellow-500' };
    return { label: 'Low', colorClass: 'bg-red-500' };
  }

  let confidenceInfo = $derived(getConfidenceInfo(confidence));
</script>

<svelte:window onmouseup={handleMouseUp} onmousemove={handleMouseMove} />

<div class="relative">
  <!-- Selection checkbox -->
  {#if onToggleSelect}
    <button
      onclick={onToggleSelect}
      class="absolute top-2 left-2 z-20 flex h-6 w-6 items-center justify-center rounded-md border-2 transition-all
        {selected
        ? 'border-accent-500 bg-accent-500 text-white'
        : 'hover:border-accent-400 border-white bg-white/80 text-transparent'}"
    >
      <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
      </svg>
    </button>
  {/if}

  <!-- Confidence badge -->
  <div
    class="absolute top-2 right-2 z-20 flex items-center gap-1.5 rounded-full bg-black/60 px-2 py-1"
  >
    <span class="h-2 w-2 rounded-full {confidenceInfo.colorClass}"></span>
    <span class="text-xs font-medium text-white">{confidenceInfo.label}</span>
  </div>

  <!-- Rotation info -->
  {#if needsCorrection && Math.abs(rotationApplied) > 0.1}
    <div class="absolute right-2 bottom-8 z-20 rounded-full bg-black/60 px-2 py-1">
      <span class="text-xs text-white"
        >{rotationApplied > 0 ? '+' : ''}{rotationApplied.toFixed(1)}&deg;</span
      >
    </div>
  {/if}

  <!-- Adjustments info -->
  {#if hasAdjustments}
    <div class="absolute right-2 bottom-2 z-20 flex gap-1">
      {#if Math.abs(brightness) >= 1}
        <div class="rounded bg-amber-500/80 px-1.5 py-0.5">
          <span class="text-[10px] font-medium text-white">B{brightness > 0 ? '+' : ''}{brightness}</span>
        </div>
      {/if}
      {#if Math.abs(exposure) >= 1}
        <div class="rounded bg-blue-500/80 px-1.5 py-0.5">
          <span class="text-[10px] font-medium text-white">E{exposure > 0 ? '+' : ''}{exposure}</span>
        </div>
      {/if}
      {#if Math.abs(contrast) >= 1}
        <div class="rounded bg-purple-500/80 px-1.5 py-0.5">
          <span class="text-[10px] font-medium text-white">C{contrast > 0 ? '+' : ''}{contrast}</span>
        </div>
      {/if}
    </div>
  {/if}

  <!-- No correction needed badge -->
  {#if !needsCorrection}
    <div class="absolute bottom-2 left-2 z-20 rounded-full bg-green-500/80 px-2 py-1">
      <span class="text-xs font-medium text-white">No changes needed</span>
    </div>
  {/if}

  <!-- Before/After comparison container -->
  <div
    bind:this={containerRef}
    class="relative aspect-[4/3] w-full cursor-ew-resize overflow-hidden rounded-lg bg-black select-none"
    role="slider"
    aria-valuenow={sliderPosition}
    aria-valuemin={0}
    aria-valuemax={100}
    tabindex="0"
    onmousedown={handleMouseDown}
    ontouchstart={handleTouchStart}
    ontouchmove={handleTouchMove}
    ontouchend={handleTouchEnd}
  >
    <!-- After image (full width, behind) -->
    <img
      src="data:image/jpeg;base64,{afterUrl}"
      alt="After correction"
      class="absolute inset-0 h-full w-full object-contain"
      draggable="false"
    />

    <!-- Before image (clipped) -->
    <div
      class="absolute inset-0 overflow-hidden"
      style="clip-path: inset(0 {100 - sliderPosition}% 0 0)"
    >
      <img
        src="data:image/jpeg;base64,{beforeUrl}"
        alt="Before correction"
        class="h-full w-full object-contain"
        draggable="false"
      />
    </div>

    <!-- Labels -->
    <div
      class="pointer-events-none absolute bottom-2 left-2 rounded bg-black/60 px-1.5 py-0.5 text-xs font-medium text-white"
      style="opacity: {sliderPosition > 15 ? 1 : 0}"
    >
      Before
    </div>
    <div
      class="pointer-events-none absolute right-2 bottom-2 rounded bg-black/60 px-1.5 py-0.5 text-xs font-medium text-white"
      style="opacity: {sliderPosition < 85 ? 1 : 0}"
    >
      After
    </div>

    <!-- Slider handle -->
    <div class="absolute top-0 bottom-0 w-0.5 bg-white shadow-lg" style="left: {sliderPosition}%">
      <!-- Handle circle -->
      <div
        class="absolute top-1/2 left-1/2 flex h-8 w-8 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full bg-white shadow-lg"
      >
        <svg class="h-4 w-4 text-gray-600" viewBox="0 0 24 24" fill="currentColor">
          <path d="M8 5v14l-7-7 7-7zm8 0v14l7-7-7-7z" />
        </svg>
      </div>
    </div>
  </div>
</div>
