<script lang="ts">
	type NotificationType = 'success' | 'error' | 'info' | 'warning';

	interface Props {
		message: string;
		type?: NotificationType;
		item: {
			pause: () => void;
			resume: () => void;
			resolve: (value: unknown) => void;
		};
	}

	let { message, type = 'info' as NotificationType, item }: Props = $props();

	function dismiss() {
		item.resolve({ dismissed: true });
	}

	const icons: Record<NotificationType, string> = {
		success: `<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />`,
		error: `<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />`,
		warning: `<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />`,
		info: `<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />`
	};

	const colors: Record<NotificationType, string> = {
		success: 'bg-green-50 border-green-200 text-green-800 dark:bg-green-900/30 dark:border-green-800 dark:text-green-200',
		error: 'bg-red-50 border-red-200 text-red-800 dark:bg-red-900/30 dark:border-red-800 dark:text-red-200',
		warning: 'bg-amber-50 border-amber-200 text-amber-800 dark:bg-amber-900/30 dark:border-amber-800 dark:text-amber-200',
		info: 'bg-blue-50 border-blue-200 text-blue-800 dark:bg-blue-900/30 dark:border-blue-800 dark:text-blue-200'
	};

	const iconColors: Record<NotificationType, string> = {
		success: 'text-green-500 dark:text-green-400',
		error: 'text-red-500 dark:text-red-400',
		warning: 'text-amber-500 dark:text-amber-400',
		info: 'text-blue-500 dark:text-blue-400'
	};
</script>

<div
	class="flex items-center gap-3 border px-4 py-3 shadow-lg {colors[type]}"
	role="alert"
	onmouseenter={() => item.pause()}
	onmouseleave={() => item.resume()}
>
	<svg
		class="h-5 w-5 flex-shrink-0 {iconColors[type]}"
		fill="none"
		stroke="currentColor"
		viewBox="0 0 24 24"
	>
		{@html icons[type]}
	</svg>
	<p class="text-sm font-medium flex-1">{message}</p>
	<button
		onclick={dismiss}
		class="ml-2 opacity-60 hover:opacity-100 transition-opacity"
		aria-label="Dismiss notification"
	>
		<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
			<path
				stroke-linecap="round"
				stroke-linejoin="round"
				stroke-width="2"
				d="M6 18L18 6M6 6l12 12"
			/>
		</svg>
	</button>
</div>
