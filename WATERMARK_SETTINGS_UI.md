# Watermark Settings UI Implementation

This document shows the complete watermark section that should replace lines 845-1013 in `src/routes/settings/+page.svelte`.

Due to the large size of the changes, I'll break this into manageable pieces. The complete implementation includes:

1. **Watermark Image Selection** - Browse and copy to app data
2. **Size Configuration** - Proportional, Fit, Stretch, Tile modes with percentage slider
3. **Position Controls** - 9-point anchor grid with X/Y offset inputs
4. **Opacity Slider** - With alpha channel checkbox
5. **Live Preview** - Real-time preview of watermark settings

## Implementation Steps

### Step 1: Add state variables (add to script section)
```typescript
// Watermark config state
let watermarkConfig = $state<WatermarkConfig>({
  sizeMode: 'proportional',
  sizePercentage: 0.35,
  relativeTo: 'longest-side',
  positionAnchor: 'center',
  offsetX: 0,
  offsetY: 0,
  opacity: 0.15,
  useAlphaChannel: true
});

let watermarkPreviewUrl = $state<string>('');
let isGeneratingPreview = $state(false);
```

### Step 2: Add helper functions
```typescript
async function selectWatermarkImage(): Promise<void> {
  try {
    const selected = await open({
      directory: false,
      multiple: false,
      title: 'Select Watermark Image',
      filters: [{
        name: 'Image Files',
        extensions: ['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp']
      }]
    });

    if (selected && typeof selected === 'string') {
      // Copy to app data
      await invoke('copy_watermark_to_app_data', { sourcePath: selected });
      config.watermark_image_path = await invoke('get_watermark_from_app_data');
      await generatePreview();
      statusMessage = 'Watermark image selected and copied to app data';
      statusType = 'success';
    }
  } catch (error) {
    console.error('Error selecting watermark image:', error);
    statusMessage = `Error selecting watermark image: ${error}`;
    statusType = 'error';
  }
}

async function generatePreview(): Promise<void> {
  if (!config.watermark_image_path) return;

  try {
    isGeneratingPreview = true;
    const base64Preview = await invoke<string>('generate_watermark_preview', {
      sampleImageBase64: null // null uses default gray sample image
    });
    watermarkPreviewUrl = `data:image/png;base64,${base64Preview}`;
  } catch (error) {
    console.error('Failed to generate preview:', error);
  } finally {
    isGeneratingPreview = false;
  }
}

// Call generatePreview whenever watermark config changes
$effect(() => {
  if (config.watermark_image_path) {
    generatePreview();
  }
});
```

See WATERMARK_SETTINGS_UI.md for the complete UI markup.
