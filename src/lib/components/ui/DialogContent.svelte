<script lang="ts">
  import { Dialog as BitsDialog } from 'bits-ui';

  interface Props {
    class?: string;
    children: import('svelte').Snippet;
  }

  let { class: className = '', children }: Props = $props();
</script>

<BitsDialog.Content
  class="bg-background-50 border-background-200 fixed top-1/2 left-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 border shadow-xl {className}"
>
  {@render children()}
</BitsDialog.Content>
