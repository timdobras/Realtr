<script lang="ts">
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';

  let { children } = $props();

  // Apply dark mode setting from main window
  onMount(() => {
    if (browser) {
      const saved = localStorage.getItem('darkMode');
      const isDarkMode = saved
        ? saved === 'true'
        : window.matchMedia('(prefers-color-scheme: dark)').matches;
      document.documentElement.classList.toggle('dark', isDarkMode);
    }
  });
</script>

<!-- Editor window: minimal layout with no sidebar or custom titlebar -->
<div class="bg-background-0 text-foreground-950 h-screen">
  {@render children()}
</div>
