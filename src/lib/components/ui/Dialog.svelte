<script lang="ts">
  import { Dialog as BitsDialog } from 'bits-ui';

  interface Props {
    open: boolean;
    onOpenChange?: (open: boolean) => void;
    children: import('svelte').Snippet;
  }

  let { open = $bindable(), onOpenChange, children }: Props = $props();
</script>

<BitsDialog.Root bind:open {onOpenChange}>
  <BitsDialog.Portal>
    <BitsDialog.Overlay class="fixed inset-0 z-50 bg-black/50" />
    {@render children()}
  </BitsDialog.Portal>
</BitsDialog.Root>
