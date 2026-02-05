<script lang="ts">
  import '../app.css';
  import NotificationPortal from '$lib/components/NotificationPortal.svelte';

  let { children } = $props();
</script>

{@render children()}

<!-- Notifications (available in all windows) -->
<NotificationPortal />
