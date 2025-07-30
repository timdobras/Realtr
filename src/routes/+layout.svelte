<script lang="ts">
  import '../app.css';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
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
      isDarkMode = saved
        ? saved === 'true'
        : window.matchMedia('(prefers-color-scheme: dark)').matches;
      document.documentElement.classList.toggle('dark', isDarkMode);
    }
    setTimeout(() => checkForUpdates(false), 5000);
  });
</script>

<div class="font-plusjakarta bg-background-0 text-foreground-950 flex h-screen">
  <!-- Sidebar -->
  <aside class="bg-background-100 border-background-300 flex w-64 flex-col border-r">
    <div class="flex items-center space-x-3 p-6">
      <div class="bg-primary-500 flex h-10 w-10 items-center justify-center rounded-lg text-white">
        📸
      </div>
      <h1 class="text-xl font-bold">Realtr</h1>
    </div>

    <nav class="flex-1 space-y-1 px-4">
      {#each navItems as item}
        <a
          href={item.href}
          class="flex items-center rounded-lg px-4 py-2 transition
            {isActive(item.href) ? 'bg-primary-100 text-primary-700' : 'hover:bg-background-100'}"
        >
          <span class="text-lg">{item.icon}</span>
          <span class="ml-3">{item.name}</span>
        </a>
      {/each}
    </nav>

    <button
      on:click={toggleTheme}
      class="bg-background-100 hover:bg-background-200 text-foreground-700 m-4 rounded-lg p-2"
      aria-label="Toggle dark mode"
    >
      {isDarkMode ? '☀️ Light' : '🌙 Dark'}
    </button>
  </aside>

  <!-- Main Content -->
  <main class="flex flex-1 flex-col">
    <section class="flex-1 overflow-auto">
      {@render children()}
    </section>
  </main>
</div>

<style></style>
