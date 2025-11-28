<script lang="ts">
	import { Dialog as BitsDialog } from 'bits-ui';

	interface Props {
		title: string;
		showCloseButton?: boolean;
		onClose?: () => void;
	}

	let { title, showCloseButton = true, onClose }: Props = $props();
</script>

<div class="border-background-200 flex items-center justify-between border-b px-5 py-4">
	<BitsDialog.Title class="text-foreground-900 text-base font-semibold">
		{title}
	</BitsDialog.Title>
	{#if showCloseButton}
		<BitsDialog.Close
			onclick={onClose}
			class="text-foreground-500 hover:text-foreground-700 transition-colors"
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
	{/if}
</div>
