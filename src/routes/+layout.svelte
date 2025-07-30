<script lang="ts">
  import '../app.css'
  import { page } from '$app/stores'
  import { onMount } from 'svelte'
  import { browser } from '$app/environment'
	import { checkForUpdates } from '$lib/utils/updater';

  let { children } = $props();
  let isDarkMode = $state(false);
  let currentPath = $derived($page.url.pathname);

  // Navigation
  const navItems = [
    { name: 'Dashboard', href: '/', icon: '🏠' },
    { name: 'Properties', href: '/properties', icon: '🏢' },
    { name: 'Settings', href: '/settings', icon: '⚙️' }
  ];

  function isActive(href: string) {
    return currentPath === href || (href !== '/' && currentPath.startsWith(href));
  }

  function toggleTheme() {
    isDarkMode = !isDarkMode;
    if (browser) {
      localStorage.setItem('darkMode', isDarkMode.toString());
      document.documentElement.classList.toggle('dark', isDarkMode);
    }
  }

  onMount(() => {
    if (browser) {
      const saved = localStorage.getItem('darkMode');
      isDarkMode = saved ? saved === 'true' : window.matchMedia('(prefers-color-scheme: dark)').matches;
      document.documentElement.classList.toggle('dark', isDarkMode);
    }
    setTimeout(() => checkForUpdates(false), 5000);
  });
</script>

<div class="flex h-screen font-plusjakarta bg-background-0 text-foreground-950">
  <!-- Sidebar -->
  <aside class="w-64 bg-background-100 border-r border-background-300 flex flex-col">
    <div class="p-6 flex items-center space-x-3">
      <div class="w-10 h-10 bg-primary-500 rounded-lg flex items-center justify-center text-white">
        📸
      </div>
      <h1 class="text-xl font-bold">Realtr</h1>
    </div>

    <nav class="flex-1 px-4 space-y-1">
      {#each navItems as item}
        <a
          href={item.href}
          class="flex items-center px-4 py-2 rounded-lg transition 
            {isActive(item.href) 
              ? 'bg-primary-100 text-primary-700' 
              : 'hover:bg-background-100'}"
        >
          <span class="text-lg">{item.icon}</span>
          <span class="ml-3">{item.name}</span>
        </a>
      {/each}
    </nav>

    <button
      on:click={toggleTheme}
      class="m-4 p-2 rounded-lg bg-background-100 hover:bg-background-200 text-foreground-700"
      aria-label="Toggle dark mode"
    >
      {isDarkMode ? '☀️ Light' : '🌙 Dark'}
    </button>
  </aside>

  <!-- Main Content -->
  <main class="flex-1 flex flex-col">
    <section class="flex-1 overflow-auto">
      {@render children()}
    </section>
  </main>
</div>

<style></style>
