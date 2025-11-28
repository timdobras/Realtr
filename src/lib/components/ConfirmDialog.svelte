<script lang="ts">
	import { Dialog, DialogContent, DialogFooter } from '$lib/components/ui';
	import { Dialog as BitsDialog } from 'bits-ui';

	interface Props {
		open?: boolean;
		title?: string;
		message: string;
		confirmText?: string;
		cancelText?: string;
		destructive?: boolean;
		onConfirm: () => void;
		onCancel: () => void;
	}

	let {
		open = $bindable(true),
		title = 'Confirm',
		message,
		confirmText = 'Confirm',
		cancelText = 'Cancel',
		destructive = false,
		onConfirm,
		onCancel
	}: Props = $props();

	function handleOpenChange(newOpen: boolean) {
		if (!newOpen) {
			onCancel();
		}
	}
</script>

<Dialog bind:open onOpenChange={handleOpenChange}>
	<DialogContent>
		<div class="p-5">
			<BitsDialog.Title class="text-foreground-900 text-base font-semibold">
				{title}
			</BitsDialog.Title>
			<p class="text-foreground-600 mt-2 whitespace-pre-line text-sm">{message}</p>
		</div>

		<DialogFooter>
			<button
				onclick={onCancel}
				class="border-background-300 bg-background-100 text-foreground-700 hover:bg-background-200 px-4 py-2 text-sm font-medium transition-colors"
			>
				{cancelText}
			</button>
			<button
				onclick={onConfirm}
				class="px-4 py-2 text-sm font-medium text-white transition-colors {destructive
					? 'bg-red-600 hover:bg-red-700'
					: 'bg-accent-500 hover:bg-accent-600'}"
			>
				{confirmText}
			</button>
		</DialogFooter>
	</DialogContent>
</Dialog>
