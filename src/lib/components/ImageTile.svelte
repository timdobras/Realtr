<script lang="ts">
  import LazyImage from './LazyImage.svelte';

  /**
   * A square thumbnail tile wrapping LazyImage with optional selection,
   * dimming, badge text, and click handling. Used by every step page that
   * shows a grid of property images (step 1, 3, 4). Step 2 has its own
   * drag/drop tile and intentionally does not use this component.
   */
  interface Props {
    folderPath: string;
    status: string;
    subfolder: string;
    filename: string;
    /** Highlights the tile with the accent border + checkmark overlay. */
    selected?: boolean;
    /** Greys the tile (e.g. step 3 marks images already in AGGELIA). */
    dimmed?: boolean;
    /** Optional small badge appended to the hover filename (e.g. "(In AGGELIA)"). */
    badge?: string;
    disabled?: boolean;
    refreshKey?: number;
    onclick?: () => void;
  }

  let {
    folderPath,
    status,
    subfolder,
    filename,
    selected = false,
    dimmed = false,
    badge,
    disabled = false,
    refreshKey = 0,
    onclick
  }: Props = $props();
</script>

<button
  type="button"
  class="group relative aspect-square w-full overflow-hidden border transition-opacity hover:opacity-75 {selected
    ? 'border-accent-500'
    : 'border-background-200'} {dimmed ? 'opacity-50' : ''}"
  {disabled}
  {onclick}
>
  <LazyImage
    {folderPath}
    {status}
    {subfolder}
    {filename}
    alt={filename}
    class="h-full w-full"
    {refreshKey}
  />

  {#if selected}
    <div class="bg-accent-500 absolute top-2 left-2 h-5 w-5">
      <svg class="h-5 w-5 text-white" fill="currentColor" viewBox="0 0 20 20">
        <path
          fill-rule="evenodd"
          d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
          clip-rule="evenodd"
        />
      </svg>
    </div>
  {/if}

  <div
    class="bg-foreground-900/75 absolute inset-x-0 bottom-0 p-2 opacity-0 transition-opacity group-hover:opacity-100"
  >
    <p class="truncate text-xs text-white" title={filename}>
      {filename}
      {#if badge}
        <span class="text-foreground-300 text-xs"> {badge}</span>
      {/if}
    </p>
  </div>
</button>
