# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Realtr is a specialized **Tauri v2 desktop application** for real estate photography workflow management. It's built with:
- **Frontend**: SvelteKit 2 + Svelte 5 + Tailwind CSS 4
- **Backend**: Rust with Tauri 2
- **Database**: SQLite with sqlx
- **Build**: SPA mode using @sveltejs/adapter-static

The application manages a 4-step workflow for processing real estate property photos from import to watermarking.

## Development Commands

### Frontend (SvelteKit)
```bash
npm run dev          # Start Vite dev server (frontend only)
npm run build        # Build SvelteKit app
npm run preview      # Preview production build
npm run check        # Run svelte-check for type checking
npm run check:watch  # Type checking in watch mode
```

### Linting & Formatting
```bash
npm run lint         # Check formatting (Prettier) and linting (ESLint)
npm run lint:fix     # Auto-fix ESLint issues and format with Prettier
npm run format       # Format code with Prettier
npm run lint:css     # Lint CSS/Svelte styles with Stylelint
npm run lint:all     # Lint both frontend and Rust code
npm run format:all   # Format both frontend and Rust code
```

### Rust Backend
```bash
npm run rust:fmt     # Format Rust code with cargo fmt
npm run rust:clippy  # Run Clippy linter (fails on warnings)
cd src-tauri && cargo build    # Build Rust backend
cd src-tauri && cargo test     # Run Rust tests
```

### Tauri (Full App)
```bash
npm run tauri dev    # Run full Tauri app in development mode
npm run tauri build  # Build production application
```

## Architecture

### Frontend Structure (SvelteKit)

**Routing**: Uses SvelteKit file-based routing in `src/routes/`:
- `/` - Home page
- `/properties` - Property list view
- `/properties/[id]` - Property detail view (4-step workflow tabs)
  - `/properties/[id]/step1` - Import original images
  - `/properties/[id]/step2` - Order and rename images
  - `/properties/[id]/step3` - Select images for advanced editing (AGGELIA)
  - `/properties/[id]/step4` - Apply watermarks
- `/settings` - App configuration

**Services Layer**:
- `src/lib/services/databaseService.ts` - All Tauri command invocations for database operations, file management, and image processing
- Uses Tauri's `invoke()` API to call Rust commands

**Type Definitions**:
- `src/lib/types/database.ts` - TypeScript interfaces matching Rust structs (Property, City, CommandResult, etc.)

### Backend Structure (Rust/Tauri)

**Main Entry Point**: `src-tauri/src/main.rs`
- Initializes Tauri app with plugins (fs, dialog, shell, opener, updater)
- Initializes SQLite database on startup
- Registers all Tauri commands via `invoke_handler!`

**Core Modules**:
- `src-tauri/src/database.rs` - **Main database and file operations module** (2300+ lines)
  - Property CRUD operations with SQLite
  - Image file operations (list, copy, rename, base64 encoding)
  - Folder management (INTERNET, AGGELIA, WATERMARK)
  - Watermarking with opacity control using the `image` crate
  - Property scanning and auto-import from filesystem

- `src-tauri/src/config.rs` - Application configuration management
  - Stores config in app data directory as JSON
  - Manages root path, editor paths, watermark settings

- `src-tauri/src/lib.rs` - Library crate (minimal, mostly for Tauri setup)

**Database Schema** (SQLite):
- `properties` table: id, name, city, completed, folder_path, notes, created_at, updated_at
- `cities` table: id, name, usage_count, created_at (for autocomplete)
- Timestamps stored as INTEGER (milliseconds since epoch)

### Folder Structure Convention

The app expects this specific filesystem structure:
```
{root_path}/
├── FOTOGRAFIES - NEW/        # Incomplete properties
│   └── {City}/
│       └── {Property}/
│           ├── INTERNET/      # Processed images
│           │   └── AGGELIA/   # Advanced edited images
│           └── WATERMARK/     # Watermarked outputs
│               └── AGGELIA/
└── FOTOGRAFIES - DONE/        # Completed properties
    └── {City}/
        └── {Property}/
            ├── INTERNET/
            │   └── AGGELIA/
            └── WATERMARK/
                └── AGGELIA/
```

### Key Workflow Steps

1. **Step 1 (Import)**: Import original images from property folder root into database
2. **Step 2 (Order)**: Copy images to INTERNET folder, rename with drag-and-drop reordering
3. **Step 3 (AGGELIA)**: Select specific images to copy into INTERNET/AGGELIA for advanced editing
4. **Step 4 (Watermark)**: Apply watermark to all INTERNET images, output to WATERMARK folders

## Important Implementation Details

### Tauri Command Pattern
All frontend-to-backend communication uses Tauri commands:
```typescript
// Frontend (TypeScript)
const result = await invoke<CommandResult>('command_name', { param: value });

// Backend (Rust)
#[tauri::command]
pub async fn command_name(app: tauri::AppHandle, param: Type) -> Result<CommandResult, String> { }
```

### Database Access
- Database pool is managed via Tauri's state management: `app.manage(pool)`
- All commands get the pool with: `get_database_pool(&app)?`
- Uses SQLx for async database operations with compile-time checked queries

### Image Processing
- Base64 encoding for image preview in frontend
- Watermarking uses `image` crate with alpha blending
- Supports: JPEG, PNG, BMP, GIF, WebP, HEIC

### Configuration Storage
- Config stored at: `{app_data_dir}/config.json`
- Database at: `{app_data_dir}/properties.db`
- Use `app.path().app_data_dir()` to get platform-specific app data location

### Clippy Rules
The project uses strict Rust linting:
- `unsafe_code = "forbid"` - No unsafe code allowed
- `unwrap_used = "warn"` - Avoid `.unwrap()`, use proper error handling
- `pedantic` and `nursery` clippy lints enabled

## Common Tasks

### Adding a New Tauri Command
1. Add the command function in appropriate Rust module (`database.rs` or `config.rs`)
2. Register it in `src-tauri/src/main.rs` in the `invoke_handler!` macro
3. Add TypeScript types in `src/lib/types/database.ts`
4. Add service method in `src/lib/services/databaseService.ts`

### Database Migrations
- Migrations are run automatically on startup via `run_migrations()` in `database.rs`
- Use `CREATE TABLE IF NOT EXISTS` for safety
- To reset database during development, delete `{app_data_dir}/properties.db`

### Working with Images
- Images are loaded from filesystem using configured `root_path`
- Path construction: `root_path + folder_path + subfolder + filename`
- Always validate paths exist before operations
- Use `fs::read()` for base64 encoding, `image::open()` for processing

### Testing the Application
Since this is a highly specialized internal tool, testing is primarily manual:
1. Start with `npm run tauri dev`
2. Configure root path in Settings
3. Test each workflow step with sample property folders
