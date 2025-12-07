<script lang="ts">
  import { DatabaseService } from '$lib/services/databaseService';

  interface Props {
    folderPath: string;
    status: string;
    subfolder: string;
    filename: string;
    alt?: string;
    class?: string;
    maxDimension?: number;
    onclick?: () => void;
  }

  let {
    folderPath,
    status,
    subfolder,
    filename,
    alt = '',
    class: className = '',
    maxDimension = 400,
    onclick
  }: Props = $props();

  let containerRef: HTMLDivElement | null = $state(null);
  let dataUrl: string | null = $state(null);
  let loading = $state(false);
  let error = $state(false);
  let hasBeenVisible = $state(false);

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
    if (loading || dataUrl) return;

    loading = true;
    error = false;

    try {
      const base64 = await DatabaseService.getGalleryThumbnail(
        folderPath,
        status,
        subfolder,
        filename,
        maxDimension
      );
      dataUrl = `data:image/jpeg;base64,${base64}`;
    } catch (e) {
      console.error('Failed to load image:', filename, e);
      error = true;
    } finally {
      loading = false;
    }
  }

  function getMimeType(ext: string): string {
    const mimeTypes: Record<string, string> = {
      jpg: 'image/jpeg',
      jpeg: 'image/jpeg',
      png: 'image/png',
      gif: 'image/gif',
      webp: 'image/webp',
      bmp: 'image/bmp',
      heic: 'image/heic'
    };
    return mimeTypes[ext.toLowerCase()] || 'image/jpeg';
  }
</script>

<div
  bind:this={containerRef}
  class="relative overflow-hidden bg-muted {className}"
  role={onclick ? 'button' : 'img'}
  tabindex={onclick ? 0 : -1}
  onclick={onclick}
  onkeydown={(e) => e.key === 'Enter' && onclick?.()}
>
  {#if loading}
    <!-- Loading skeleton -->
    <div class="absolute inset-0 flex items-center justify-center">
      <div class="h-6 w-6 animate-spin rounded-full border-2 border-muted-foreground/20 border-t-muted-foreground"></div>
    </div>
  {:else if error}
    <!-- Error state -->
    <div class="absolute inset-0 flex flex-col items-center justify-center text-muted-foreground">
      <svg class="h-8 w-8 mb-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
      </svg>
      <span class="text-xs">Failed</span>
    </div>
  {:else if dataUrl}
    <!-- Loaded image -->
    <img
      src={dataUrl}
      {alt}
      class="h-full w-full object-cover"
      draggable="false"
    />
  {:else}
    <!-- Placeholder before observer triggers -->
    <div class="absolute inset-0 bg-muted"></div>
  {/if}
</div>
