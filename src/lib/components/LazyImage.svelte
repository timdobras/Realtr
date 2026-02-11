<script lang="ts">
  import { DatabaseService } from '$lib/services/databaseService';
  import { convertFileSrc } from '@tauri-apps/api/core';

  interface Props {
    folderPath: string;
    status: string;
    subfolder: string;
    filename: string;
    alt?: string;
    class?: string;
    maxDimension?: number;
    onclick?: () => void;
    refreshKey?: number; // Increment to force reload
  }

  let {
    folderPath,
    status,
    subfolder,
    filename,
    alt = '',
    class: className = '',
    maxDimension = 400,
    onclick,
    refreshKey = 0
  }: Props = $props();

  let containerRef: HTMLDivElement | null = $state(null);
  let imgSrc: string | null = $state(null);
  let loading = $state(false);
  let error = $state(false);
  let hasBeenVisible = $state(false);
  let lastRefreshKey = $state(refreshKey);

  // Watch for refreshKey changes to force reload
  $effect(() => {
    if (refreshKey !== lastRefreshKey && hasBeenVisible) {
      lastRefreshKey = refreshKey;
      // Clear current image and reload
      imgSrc = null;
      error = false;
      loadImage();
    }
  });

  // Intersection Observer to detect when image enters viewport
  $effect(() => {
    if (!containerRef || hasBeenVisible) return;

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting && !hasBeenVisible) {
            hasBeenVisible = true;
            loadImage();
            observer.disconnect();
          }
        }
      },
      {
        rootMargin: '100px', // Start loading 100px before entering viewport
        threshold: 0
      }
    );

    observer.observe(containerRef);

    return () => {
      observer.disconnect();
    };
  });

  async function loadImage() {
    if (loading || imgSrc) return;

    loading = true;
    error = false;

    try {
      // Get the filesystem path to the (possibly just-generated) thumbnail
      const thumbnailPath = await DatabaseService.getGalleryThumbnailPath(
        folderPath,
        status,
        subfolder,
        filename,
        maxDimension
      );
      // Convert to asset protocol URL — served directly from disk, zero base64 overhead
      imgSrc = convertFileSrc(thumbnailPath) + `?t=${refreshKey}`;
    } catch (e) {
      console.error('Failed to load image:', filename, e);
      error = true;
    } finally {
      loading = false;
    }
  }

  function retryLoad(e: MouseEvent) {
    e.stopPropagation(); // Don't trigger parent onclick
    error = false;
    imgSrc = null;
    loadImage();
  }
</script>

<div
  bind:this={containerRef}
  class="bg-muted relative overflow-hidden {className}"
  role={onclick ? 'button' : 'img'}
  tabindex={onclick ? 0 : -1}
  {onclick}
  onkeydown={(e) => e.key === 'Enter' && onclick?.()}
>
  {#if loading}
    <!-- Loading skeleton -->
    <div class="absolute inset-0 flex items-center justify-center">
      <div
        class="border-muted-foreground/20 border-t-muted-foreground h-6 w-6 animate-spin rounded-full border-2"
      ></div>
    </div>
  {:else if error}
    <!-- Error state with retry button -->
    <div class="text-muted-foreground absolute inset-0 flex flex-col items-center justify-center">
      <svg class="mb-1 h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="1.5"
          d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
        />
      </svg>
      <span class="mb-1 text-xs">Failed to load</span>
      <button
        onclick={retryLoad}
        class="bg-background-200 hover:bg-background-300 rounded px-2 py-0.5 text-xs transition-colors"
      >
        Retry
      </button>
    </div>
  {:else if imgSrc}
    <!-- Loaded image -->
    <img src={imgSrc} {alt} class="h-full w-full object-cover" draggable="false" />
  {:else}
    <!-- Placeholder before observer triggers -->
    <div class="bg-muted absolute inset-0"></div>
  {/if}
</div>
