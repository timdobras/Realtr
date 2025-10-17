# Watermark Feature Integration Guide

## Summary of Changes

I've successfully implemented a comprehensive watermark system with advanced configuration options. Here's what has been completed:

### ✅ Backend (Rust) - COMPLETE

1. **Enhanced Configuration** (`src-tauri/src/config.rs`):
   - Added `WatermarkConfig` struct with:
     - Size modes: proportional, fit, stretch, tile
     - Proportional sizing with percentage (5-100%)
     - Position anchoring (9-point grid)
     - X/Y offsets for fine-tuning
     - Opacity control (0-100%)
     - Alpha channel toggle
   - Added `copy_watermark_to_app_data()` command
   - Added `get_watermark_from_app_data()` command

2. **Watermark Application Logic** (`src-tauri/src/database.rs`):
   - `apply_watermark_with_config()` - Main watermarking logic
   - `apply_single_watermark()` - Single watermark placement
   - `apply_tiled_watermark()` - Repeating pattern
   - `blend_watermark()` - Alpha blending with transparency support
   - `generate_watermark_preview()` - Live preview generation

3. **Updated Commands**:
   - All commands registered in `src-tauri/src/main.rs`
   - `copy_and_watermark_images` now uses new config system

### ✅ Frontend (TypeScript) - COMPLETE

1. **Type Definitions** (`src/lib/types/database.ts`):
   - `WatermarkConfig` interface
   - Updated `AppConfig` interface

2. **Settings Page Logic** (`src/routes/settings/+page.svelte`):
   - Updated state management
   - Added preview generation
   - Added config persistence

### 🔄 Frontend UI - NEEDS INTEGRATION

The enhanced watermark UI is ready in `watermark-ui-section.svelte`. You need to replace the existing watermark section (lines 909-1013) in `src/routes/settings/+page.svelte`.

## Integration Steps

### Step 1: Replace Watermark Section

Open `src/routes/settings/+page.svelte` and find the watermark configuration section (starts around line 909 with `<!-- Watermark Configuration -->`).

Replace everything from:
```svelte
<!-- Watermark Configuration -->
<div class="bg-background-50 border-background-200 rounded-xl border p-6 shadow-sm">
  ...
</div>
```

Up to and including the closing `</div>` (around line 1013) with the content from `watermark-ui-section.svelte`.

###  Step 2: Test the Application

1. **Build Rust backend**:
   ```bash
   cd src-tauri
   cargo check
   ```

2. **Run in development mode**:
   ```bash
   npm run tauri dev
   ```

3. **Test watermark functionality**:
   - Go to Settings
   - Select a watermark image (preferably PNG with transparency)
   - Configure size, position, and opacity
   - Watch the live preview update
   - Save configuration
   - Go to a property's Step 4
   - Apply watermark to images

### Step 3: Verify All Features

- [ ] Watermark image selection and copying to app data
- [ ] Size modes (proportional, fit, stretch, tile)
- [ ] Proportional sizing percentage slider
- [ ] "Relative to" dropdown (longest/shortest side, width, height)
- [ ] 9-point anchor grid
- [ ] X/Y offset inputs
- [ ] Opacity slider (0-100%)
- [ ] "Use alpha channel" checkbox
- [ ] Live preview generation
- [ ] Configuration persistence
- [ ] Watermark application on actual images

## Key Features

### Size Configuration
- **Proportional**: Scales watermark to a percentage of the reference dimension
- **Fit**: Scales to fit within image bounds (maintaining aspect ratio)
- **Stretch**: Stretches watermark to cover entire image
- **Tile**: Repeats watermark in a grid pattern

### Position Control
- **9-point anchor grid**: TL, TC, TR, CL, C, CR, BL, BC, BR
- **Pixel offsets**: Fine-tune position with X/Y offsets

### Advanced Options
- **Opacity**: Full control from 0% (invisible) to 100% (opaque)
- **Alpha channel**: Respect PNG transparency or apply uniform opacity

### Live Preview
- Real-time preview updates as you adjust settings
- Uses actual watermark image with current configuration
- Debounced updates (500ms) for smooth UX

## File Structure

```
realtor-photo-manager/
├── src-tauri/
│   ├── src/
│   │   ├── config.rs          ← Updated with WatermarkConfig
│   │   ├── database.rs        ← Enhanced watermarking logic
│   │   └── main.rs            ← Registered new commands
│   └── Cargo.toml
├── src/
│   ├── lib/
│   │   └── types/
│   │       └── database.ts    ← Added WatermarkConfig interface
│   └── routes/
│       └── settings/
│           └── +page.svelte   ← Updated script, needs UI replacement
├── watermark-ui-section.svelte    ← NEW UI to integrate
└── WATERMARK_INTEGRATION_GUIDE.md ← This file
```

## Troubleshooting

### Preview not generating
- Check browser console for errors
- Verify watermark image was copied to app data
- Ensure `generate_watermark_preview` command is registered

### Watermark not applying to images
- Verify config is saved before applying watermark
- Check that watermark image exists in app data
- Look for Rust console errors during watermarking

### UI not responding
- Clear browser cache
- Restart `npm run tauri dev`
- Check for TypeScript errors in terminal

## Next Steps

After integration:
1. Test with various watermark images (PNG, JPG)
2. Test all size modes
3. Test all anchor positions
4. Test with both portrait and landscape images
5. Verify watermarked output quality

## Cleanup (Optional)

The old `apply_watermark_to_image` function in `database.rs` (line 2248) is no longer used and can be removed if you want cleaner code. It's been replaced by `apply_watermark_to_image_with_config`.

To remove it, delete lines 2248-2316 in `src-tauri/src/database.rs`.
