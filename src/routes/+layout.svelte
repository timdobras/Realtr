<script lang="ts">
  import '../app.css';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
  import { checkForUpdates } from '$lib/utils/updater';

  let { children } = $props();
  let isDarkMode = $state(false);
  let currentPath = $derived($page.url.pathname);

  // Navigation with SVG icons
  const navItems = [
    {
      name: 'Dashboard',
      href: '/',
      icon: `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2H5a2 2 0 00-2-2z"/>
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 5a2 2 0 012-2h4a2 2 0 012 2v6H8V5z"/>
      </svg>`
    },
    {
      name: 'Properties',
      href: '/properties',
      icon: `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"/>
      </svg>`
    },
    {
      name: 'Settings',
      href: '/settings',
      icon: `<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"/>
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/>
      </svg>`
    }
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
  <aside class="bg-background-50 border-background-200 flex w-64 flex-col border-r shadow-sm">
    <!-- Logo Area -->
    <div class="border-background-200 flex items-center space-x-3 border-b p-6">
      <div class="bg-accent-500 flex h-10 w-10 items-center justify-center rounded-lg">
        <svg class="h-6 w-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M3 9a2 2 0 012-2h.93a2 2 0 001.664-.89l.812-1.22A2 2 0 0110.07 4h3.86a2 2 0 011.664.89l.812 1.22A2 2 0 0018.07 7H19a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V9z"
          />
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M15 13a3 3 0 11-6 0 3 3 0 016 0z"
          />
        </svg>
      </div>
      <div>
        <h1 class="text-foreground-900 text-xl font-bold">Realtr</h1>
        <p class="text-foreground-500 text-xs font-medium">Property Management</p>
      </div>
    </div>

    <!-- Navigation -->
    <nav class="flex-1 space-y-1 p-4">
      {#each navItems as item}
        <a
          href={item.href}
          class="flex items-center rounded-lg px-3 py-2.5 text-sm font-medium transition-all duration-200
            {isActive(item.href)
            ? 'bg-accent-50 text-accent-700 border-accent-500 border-r-2'
            : 'text-foreground-600 hover:bg-background-100 hover:text-foreground-900'}"
        >
          <span class="flex-shrink-0">
            {@html item.icon}
          </span>
          <span class="ml-3">{item.name}</span>
        </a>
      {/each}
    </nav>

    <!-- Theme Toggle -->
    <div class="border-background-200 border-t p-4">
      <button
        onclick={toggleTheme}
        class="bg-background-100 hover:bg-background-200 text-foreground-700 flex w-full items-center justify-center space-x-2 rounded-lg p-2.5 text-sm font-medium transition-colors duration-200"
        aria-label="Toggle dark mode"
      >
        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          {#if isDarkMode}
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
            />
          {:else}
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
            />
          {/if}
        </svg>
        <span>{isDarkMode ? 'Light Mode' : 'Dark Mode'}</span>
      </button>
    </div>
  </aside>

  <!-- Main Content -->
  <main class="bg-background-0 flex flex-1 flex-col">
    <section class="flex-1 overflow-auto">
      {@render children()}
    </section>
  </main>
</div>
