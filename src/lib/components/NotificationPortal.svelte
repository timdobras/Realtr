<script lang="ts">
	import { notificationStack } from '$lib/stores/notification';
	import { fly } from 'svelte/transition';
</script>

{#if notificationStack.items.length > 0}
	<aside class="fixed bottom-4 right-4 z-[100] flex flex-col gap-2 max-w-sm">
		{#each notificationStack.items as notification (notification.config.id)}
			<div
				use:notificationStack.actions.render={notification}
				in:fly={{ x: 100, duration: 200 }}
				out:fly={{ x: 100, duration: 150 }}
			></div>
		{/each}
	</aside>
{/if}
