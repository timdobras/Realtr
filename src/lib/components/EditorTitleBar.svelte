<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';

  interface Props {
    filename: string;
    dimensions?: { width: number; height: number } | null;
    canUndo: boolean;
    canRedo: boolean;
    canSave: boolean;
    isSaving: boolean;
    onUndo: () => void;
    onRedo: () => void;
    onReset: () => void;
    onSave: () => void;
    onClose: () => void;
  }

  let {
    filename,
    dimensions = null,
    canUndo,
    canRedo,
    canSave,
    isSaving,
    onUndo,
    onRedo,
    onReset,
    onSave,
    onClose
  }: Props = $props();

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
    onClose();
  }

  async function startDrag(e: MouseEvent) {
    if ((e.target as HTMLElement).closest('button')) return;
    await appWindow.startDragging();
  }

  checkMaximized();
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="bg-background-0 border-background-200 flex h-10 items-center justify-between border-b select-none"
  onmousedown={startDrag}
  ondblclick={toggleMaximize}
>
  <!-- Left: Title and file info -->
  <div class="flex items-center gap-3 px-3">
    <span class="text-foreground-900 text-sm font-medium">Editor</span>
    <span class="text-foreground-400">|</span>
    <span class="text-foreground-600 text-xs">{filename}</span>
    {#if dimensions}
      <span class="text-foreground-400 text-xs">
        {dimensions.width} x {dimensions.height}
      </span>
    {/if}
  </div>

  <!-- Center: Undo/Redo/Reset -->
  <div class="flex items-center gap-1">
    <button
      onclick={onUndo}
      disabled={!canUndo}
      class="text-foreground-600 hover:bg-background-200 rounded p-1.5 disabled:opacity-30"
      title="Undo (Ctrl+Z)"
      aria-label="Undo"
    >
      <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"
        />
      </svg>
    </button>
    <button
      onclick={onRedo}
      disabled={!canRedo}
      class="text-foreground-600 hover:bg-background-200 rounded p-1.5 disabled:opacity-30"
      title="Redo (Ctrl+Shift+Z)"
      aria-label="Redo"
    >
      <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M21 10h-10a8 8 0 00-8 8v2M21 10l-6 6m6-6l-6-6"
        />
      </svg>
    </button>
    <button
      onclick={onReset}
      class="text-foreground-600 hover:bg-background-200 rounded px-2 py-1 text-xs"
      title="Reset all changes"
    >
      Reset
    </button>
  </div>

  <!-- Right: Save/Cancel + Window controls -->
  <div class="flex h-full items-center">
    <!-- Save/Cancel buttons -->
    <div class="mr-2 flex items-center gap-2 pr-2">
      <button
        onclick={onClose}
        disabled={isSaving}
        class="text-foreground-600 hover:bg-background-200 rounded px-3 py-1 text-xs"
      >
        Cancel
      </button>
      <button
        onclick={onSave}
        disabled={isSaving || !canSave}
        class="bg-accent-500 hover:bg-accent-600 rounded px-3 py-1 text-xs font-medium text-white disabled:opacity-50"
      >
        {#if isSaving}
          Saving...
        {:else}
          Save
        {/if}
      </button>
    </div>

    <!-- Window Controls -->
    <div class="border-background-200 flex h-full border-l">
      <button
        onclick={minimize}
        class="hover:bg-background-200 text-foreground-600 flex h-full w-10 items-center justify-center transition-colors"
        aria-label="Minimize"
      >
        <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
          <path d="M5 12h14" />
        </svg>
      </button>

      <button
        onclick={toggleMaximize}
        class="hover:bg-background-200 text-foreground-600 flex h-full w-10 items-center justify-center transition-colors"
        aria-label={isMaximized ? 'Restore' : 'Maximize'}
      >
        {#if isMaximized}
          <svg
            class="h-3 w-3"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            stroke-width="2"
          >
            <path d="M8 4h12v12M4 8h12v12H4z" />
          </svg>
        {:else}
          <svg
            class="h-3 w-3"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            stroke-width="2"
          >
            <rect x="4" y="4" width="16" height="16" />
          </svg>
        {/if}
      </button>

      <button
        onclick={close}
        class="text-foreground-600 flex h-full w-10 items-center justify-center transition-colors hover:bg-red-500 hover:text-white"
        aria-label="Close"
      >
        <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
          <path d="M6 6l12 12M6 18L18 6" />
        </svg>
      </button>
    </div>
  </div>
</div>
