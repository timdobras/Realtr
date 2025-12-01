<script lang="ts">
  interface Props {
    class?: string;
    children: import('svelte').Snippet;
  }

  let { class: className = '', children }: Props = $props();
</script>

<div
  class="border-background-200 flex items-center justify-end gap-2 border-t px-5 py-3 {className}"
>
  {@render children()}
</div>
