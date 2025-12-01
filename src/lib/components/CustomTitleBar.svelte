<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let isMaximized = $state(false);

  const appWindow = getCurrentWindow();

  async function checkMaximized() {
    isMaximized = await appWindow.isMaximized();
  }

  async function minimize() {
    await appWindow.minimize();
  }

  async function toggleMaximize() {
    await appWindow.toggleMaximize();
    isMaximized = await appWindow.isMaximized();
  }

  async function close() {
    await appWindow.close();
  }

  async function startDrag(e: MouseEvent) {
    // Only drag if clicking on the title bar itself, not the buttons
    if ((e.target as HTMLElement).closest('button')) return;
    await appWindow.startDragging();
  }

  // Check initial state
  checkMaximized();
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="bg-background-100 border-background-200 flex h-8 items-center justify-between border-b select-none"
  onmousedown={startDrag}
  ondblclick={toggleMaximize}
>
  <!-- App Title -->
  <div class="flex items-center gap-2 px-3">
    <span class="text-foreground-900 text-xs font-medium">Realtr</span>
  </div>

  <!-- Window Controls -->
  <div class="flex h-full">
    <!-- Minimize -->
    <button
      onclick={minimize}
      class="hover:bg-background-200 text-foreground-600 flex h-full w-11 items-center justify-center transition-colors"
      aria-label="Minimize"
    >
      <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
        <path d="M5 12h14" />
      </svg>
    </button>

    <!-- Maximize/Restore -->
    <button
      onclick={toggleMaximize}
      class="hover:bg-background-200 text-foreground-600 flex h-full w-11 items-center justify-center transition-colors"
      aria-label={isMaximized ? 'Restore' : 'Maximize'}
    >
      {#if isMaximized}
        <!-- Restore icon -->
        <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
          <path d="M8 4h12v12M4 8h12v12H4z" />
        </svg>
      {:else}
        <!-- Maximize icon -->
        <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
          <rect x="4" y="4" width="16" height="16" />
        </svg>
      {/if}
    </button>

    <!-- Close -->
    <button
      onclick={close}
      class="text-foreground-600 flex h-full w-11 items-center justify-center transition-colors hover:bg-red-500 hover:text-white"
      aria-label="Close"
    >
      <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
        <path d="M6 6l12 12M6 18L18 6" />
      </svg>
    </button>
  </div>
</div>
