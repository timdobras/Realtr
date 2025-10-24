# Standardized Image Display Pattern

## Simple Image Grid - Use Everywhere

```svelte
<!-- Standard Image Grid -->
<div class="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
  {#each images as image}
    <button
      onclick={() => openImage(image.filename)}
      class="bg-background-100 border-background-200 aspect-square overflow-hidden rounded-md border transition-colors hover:border-background-300"
    >
      {#if image.loading}
        <div class="flex h-full items-center justify-center">
          <div class="h-4 w-4 animate-spin rounded-full border-2 border-foreground-300 border-t-transparent"></div>
        </div>
      {:else if image.dataUrl}
        <img
          src={image.dataUrl}
          alt={image.filename}
          loading="lazy"
          class="h-full w-full object-cover"
        />
      {:else}
        <div class="flex h-full items-center justify-center text-xs text-red-700">
          Failed
        </div>
      {/if}
    </button>
  {/each}
</div>
```

## No More:
- ❌ Hover scale effects (`hover:scale-[1.02]`)
- ❌ Gradient overlays (`bg-gradient-to-t from-black`)
- ❌ Filename overlays on hover
- ❌ Icon overlays (eye icon, edit icon, etc.)
- ❌ Shadow transitions (`hover:shadow-lg`)
- ❌ Opacity transitions on hover
- ❌ Complex group hover states

## Simple is Better:
- ✅ Clean border
- ✅ Simple hover border color change
- ✅ Aspect square containers
- ✅ Object-cover images
- ✅ Loading spinner
- ✅ Error state
