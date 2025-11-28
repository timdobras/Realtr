<script lang="ts">
  import '../app.css';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
  import { checkForUpdates } from '$lib/utils/updater';
  import { DatabaseService } from '$lib/services/databaseService';
  import OpenCVSetupModal from '$lib/components/OpenCVSetupModal.svelte';
  import NotificationPortal from '$lib/components/NotificationPortal.svelte';

  let { children } = $props();
  let isDarkMode = $state(false);
  let currentPath = $derived($page.url.pathname);
  let showOpenCVSetup = $state(false);
  let checkingOpenCV = $state(true);

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

  onMount(async () => {
    if (browser) {
      const saved = localStorage.getItem('darkMode');
      isDarkMode = saved
        ? saved === 'true'
        : window.matchMedia('(prefers-color-scheme: dark)').matches;
      document.documentElement.classList.toggle('dark', isDarkMode);
    }
    setTimeout(() => checkForUpdates(false), 5000);

    // Check OpenCV status on startup
    try {
      const wasSkipped = await DatabaseService.wasOpenCVSetupSkipped();
      if (!wasSkipped) {
        const status = await DatabaseService.checkOpenCVStatus();
        if (!status.installed) {
          showOpenCVSetup = true;
        }
      }
    } catch (err) {
      console.error('Failed to check OpenCV status:', err);
    } finally {
      checkingOpenCV = false;
    }
  });

  function handleOpenCVComplete() {
    showOpenCVSetup = false;
  }

  function handleOpenCVSkip() {
    showOpenCVSetup = false;
  }
</script>

<div class="bg-background-0 text-foreground-950 flex h-screen">
  <!-- Sidebar -->
  <aside class="bg-background-50 border-background-200 flex w-60 flex-col border-r">
    <!-- Logo Area -->
    <div class="border-background-200 flex h-20 flex-col items-center justify-center border-b">
      <h1 class="text-foreground-900 text-base font-semibold">Realtr</h1>
      <p class="text-foreground-500 mt-0.5 text-xs">Photo Manager</p>
    </div>

    <!-- Navigation -->
    <nav class="flex-1 space-y-1 px-2 py-3">
      {#each navItems as item}
        <a
          href={item.href}
          class="relative flex items-center gap-3 px-3 py-2 text-sm transition-colors
            {isActive(item.href)
            ? 'text-foreground-900 bg-background-100 border-foreground-900 border-l-2 font-medium'
            : 'text-foreground-600 hover:bg-background-100 hover:text-foreground-900 border-l-2 border-transparent'}"
        >
          {@html item.icon}
          <span>{item.name}</span>
        </a>
      {/each}
    </nav>

    <!-- Theme Toggle -->
    <div class="border-background-200 border-t px-2 py-3">
      <button
        onclick={toggleTheme}
        class="hover:bg-background-200 text-foreground-700 flex w-full items-center gap-3 px-3 py-2 text-sm transition-colors"
        aria-label="Toggle dark mode"
      >
        <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
        <span>{isDarkMode ? 'Light' : 'Dark'}</span>
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

<!-- OpenCV Setup Modal -->
{#if showOpenCVSetup}
  <OpenCVSetupModal onComplete={handleOpenCVComplete} onSkip={handleOpenCVSkip} />
{/if}

<!-- Notifications -->
<NotificationPortal />
