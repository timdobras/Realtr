<script lang="ts">
  import { DatabaseService } from '$lib/services/databaseService';
  import { Dialog, DialogContent, DialogHeader } from '$lib/components/ui';

  interface Props {
    open?: boolean;
    propertyId: number;
    propertyName: string;
    currentCode?: string;
    onClose: () => void;
    onCodeSet: () => void;
  }

  let {
    open = $bindable(true),
    propertyId,
    propertyName,
    currentCode = '',
    onClose,
    onCodeSet
  }: Props = $props();

  let code = $state(currentCode);
  let isSubmitting = $state(false);
  let error = $state('');

  async function handleSubmit(event: Event) {
    event.preventDefault();

    if (!code.trim()) {
      error = 'Please enter a code';
      return;
    }

    try {
      isSubmitting = true;
      error = '';

      const result = await DatabaseService.setPropertyCode(propertyId, code.trim());

      if (result.success) {
        onCodeSet();
      } else {
        error = result.error || 'Failed to set code';
      }
    } catch (err) {
      console.error('Error setting code:', err);
      error = 'Failed to set code';
    } finally {
      isSubmitting = false;
    }
  }

  function handleOpenChange(newOpen: boolean) {
    if (!newOpen) {
      onClose();
    }
  }
</script>

<Dialog bind:open onOpenChange={handleOpenChange}>
  <DialogContent class="max-w-sm">
    <DialogHeader title={currentCode ? 'Edit Code' : 'Add Code'} {onClose} />

    <form onsubmit={handleSubmit} class="p-4">
      <p class="text-foreground-600 mb-3 text-xs">
        Set the website listing code for <strong class="text-foreground-900">{propertyName}</strong
        >. This will rename the folder to include the code.
      </p>

      <div class="mb-4">
        <label class="text-foreground-700 mb-1 block text-xs font-medium"> Listing Code </label>
        <input
          type="text"
          bind:value={code}
          placeholder="e.g., 45164"
          class="border-background-300 bg-background-100 text-foreground-900 placeholder-foreground-500 focus:ring-accent-500 focus:border-accent-500 w-full rounded border px-3 py-2 text-sm transition-colors focus:ring-1 focus:outline-none"
          autofocus
        />
      </div>

      {#if error}
        <div class="mb-4 rounded border border-red-300 bg-red-50 px-3 py-2">
          <p class="text-xs text-red-800">{error}</p>
        </div>
      {/if}

      <div class="flex items-center justify-end gap-2">
        <button
          type="button"
          onclick={onClose}
          disabled={isSubmitting}
          class="bg-background-100 hover:bg-background-200 text-foreground-700 rounded px-3 py-1.5 text-xs transition-colors disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={isSubmitting}
          class="bg-foreground-900 hover:bg-foreground-800 text-background-0 rounded px-3 py-1.5 text-xs transition-colors disabled:opacity-50"
        >
          {#if isSubmitting}
            Saving...
          {:else}
            Save Code
          {/if}
        </button>
      </div>
    </form>
  </DialogContent>
</Dialog>
