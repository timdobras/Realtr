<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';

  /**
   * Responsive 2/3/4/5-column thumbnail grid used by every workflow step.
   * The caller renders each tile via the `children` snippet so this stays
   * agnostic about whether tiles are <ImageTile>, plain <LazyImage>, or
   * something custom.
   *
   * If `items` is empty and `emptyState` is provided, the grid renders the
   * empty state instead — kills the `{#if list.length === 0}{:else}...{/if}`
   * boilerplate that every step page currently duplicates.
   */
  interface Props {
    items: T[];
    children: Snippet<[T, number]>;
    emptyState?: Snippet;
    class?: string;
  }

  let { items, children, emptyState, class: className = '' }: Props = $props();
</script>

{#if items.length === 0 && emptyState}
  {@render emptyState()}
{:else}
  <div class="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 {className}">
    {#each items as item, index}
      {@render children(item, index)}
    {/each}
  </div>
{/if}
