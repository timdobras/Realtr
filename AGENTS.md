# AGENTS.md

Guidance for AI coding agents working in this repository.

## Project Overview

Realtr is a Tauri v2 desktop app for real estate photography workflow management.
- **Frontend**: SvelteKit 2 + Svelte 5 (runes) + Tailwind CSS 4 (SPA mode via adapter-static)
- **Backend**: Rust (Tauri 2) with SQLite (sqlx), wgpu compute shaders, turbojpeg, SIMD resizing
- **Platform**: Windows-only (NSIS installer, custom title bar, no native decorations)

## Build / Lint / Test Commands

### Frontend
```bash
npm run dev              # Vite dev server on port 1420
npm run build            # Production SvelteKit build
npm run check            # svelte-check type checking
npm run lint             # Prettier check + ESLint
npm run lint:fix         # Auto-fix ESLint + Prettier
npm run format           # Prettier --write src/
npm run lint:css         # Stylelint for CSS/Svelte
```

### Rust Backend
```bash
cd src-tauri && cargo build          # Build Rust backend
cd src-tauri && cargo clippy -- -D warnings   # Clippy (strict: pedantic + nursery)
cd src-tauri && cargo fmt            # Format Rust code
cd src-tauri && cargo test           # Run Rust tests (none exist yet)
```

### Full App
```bash
npm run tauri dev        # Run full Tauri app in dev mode
npm run tauri build      # Production build
npm run lint:all         # Lint frontend + Rust clippy
npm run format:all       # Format frontend + Rust
```

### Running a Single Test
No test framework is configured. There are no frontend tests (no vitest/jest) and no Rust
`#[cfg(test)]` modules. To add a Rust test, add a `#[cfg(test)]` module in the relevant file
and run: `cd src-tauri && cargo test test_name`. For frontend, install vitest first.

## Code Style: TypeScript / Svelte

### Formatting (Prettier)
- Spaces (no tabs), 2-space indent, `printWidth: 100`
- Single quotes, no trailing commas, no semicolons enforcement (Prettier default: semicolons on)
- Plugins auto-sort imports (`prettier-plugin-organize-imports`) and Tailwind classes

### Import Order (auto-enforced by prettier-plugin-organize-imports)
1. Svelte imports (`onMount`, `$app/stores`)
2. Tauri API (`@tauri-apps/api/core`, `@tauri-apps/plugin-*`)
3. Third-party (`bits-ui`, `svelte-dnd-action`)
4. Internal (`$lib/services/...`, `$lib/components/...`, `$lib/types/...`, `$lib/stores/...`)
5. Relative imports (`./Component.svelte`)

### Naming Conventions
- Variables/functions: `camelCase` (`isLoading`, `loadProperties`, `handleWindowBlur`)
- Components: `PascalCase` files (`PropertyCard.svelte`, `AddPropertyModal.svelte`)
- Types/interfaces: `PascalCase` (`Property`, `CommandResult`, `PropertyStatus`)
- Constants: `UPPER_SNAKE_CASE` (`STATUS_LABELS`, `SORT_OPTIONS`)
- CSS: Tailwind utility classes with semantic color tokens (`foreground-*`, `background-*`, `accent-*`)

### Svelte 5 Patterns
- Use runes: `$state()`, `$derived()`, `$effect()`, `$props()`
- Typed state: `let items = $state<Item[]>([]);`
- Typed props: `interface Props { ... }; let { prop1, prop2 = default }: Props = $props();`
- Use `{@render children()}` instead of `<slot />`
- Use `{#snippet name()}` for render props
- Use `<script lang="ts">` in all components

### Types
- Use `import type { ... }` for type-only imports
- Type all `invoke<T>()` calls with the expected return type
- Define interfaces in `src/lib/types/database.ts` matching Rust structs
- Use union literal types for enums: `type Status = 'NEW' | 'DONE' | 'ARCHIVE'`

### Error Handling (Frontend)
- Wrap all `invoke()` calls in `try/catch`
- Check `result.success` for `CommandResult` responses
- Use `showError()` / `showSuccess()` notification helpers from `$lib/stores/notification`
- Render loading/error/content states: `{#if loading}...{:else if error}...{:else}...{/if}`

## Code Style: Rust

### Lints (Cargo.toml `[lints]`)
- `unsafe_code = "forbid"` -- no unsafe code allowed
- `clippy::pedantic = "warn"` and `clippy::nursery = "warn"` -- strict linting
- `clippy::unwrap_used = "warn"` -- avoid `.unwrap()`, use proper error handling
- `clippy::enum_glob_use = "warn"`
- Run `cargo clippy -- -D warnings` (all warnings are errors in CI)

### Formatting (rustfmt)
- `max_width = 100`, spaces (4-space indent), Unix newlines
- `reorder_imports = true` (automatic alphabetical import sorting)
- Edition 2021

### Import Order (auto-sorted by rustfmt)
1. External crates: `base64`, `image`, `rayon`, `serde`, `sqlx`, `tauri`
2. Standard library: `std::collections`, `std::fs`, `std::path`, `std::sync`
3. Internal crate: `crate::config`, `crate::gpu`, `crate::turbo`

### Naming Conventions
- Functions: `snake_case` (`create_property`, `get_image_as_base64`)
- Types/structs: `PascalCase` (`AppConfig`, `CommandResult`)
- GPU functions: prefix `gpu_` for GPU ops, `cpu_` for CPU fallbacks
- Module sections: separated with `// ============` comment block headers

### Error Handling (Rust)
- All Tauri commands return `Result<T, String>`
- Use `.map_err(|e| format!("Descriptive message: {e}"))?` for error propagation
- Use `CommandResult { success, error, data }` for structured responses to frontend
- Never `.unwrap()` -- use `.ok_or()`, `.unwrap_or_default()`, `.unwrap_or_else()`
- GPU operations: try GPU first, fall back to CPU with `eprintln!` logging on failure

### Tauri Command Pattern
```rust
#[tauri::command]
pub async fn command_name(app: tauri::AppHandle, param: Type) -> Result<T, String> {
    let pool = get_database_pool(&app)?;
    // ...
}
```
- Register new commands in `src-tauri/src/main.rs` `invoke_handler!` macro
- Access database via `get_database_pool(&app)?`
- Access config via `crate::config::get_cached_config(&app).await`

### Data Structs
- Derive: `#[derive(Debug, Serialize, Deserialize, Clone)]`
- Use `#[serde(rename_all = "camelCase")]` on structs sent to the frontend
- Use `Option<T>` for nullable fields

## Architecture Notes

### Adding a New Feature End-to-End
1. Define TypeScript types in `src/lib/types/database.ts`
2. Add Rust command in `src-tauri/src/database.rs` (or new module)
3. Register command in `src-tauri/src/main.rs` `invoke_handler!`
4. Add service wrapper in `src/lib/services/databaseService.ts`
5. Use in Svelte component via `invoke()`

### Key Filesystem Convention
Properties live under `{root_path}/FOTOGRAFIES - NEW/{City}/{Property}/` (incomplete)
or `{root_path}/FOTOGRAFIES - DONE/{City}/{Property}/` (completed). Subfolders:
`INTERNET/`, `INTERNET/AGGELIA/`, `WATERMARK/`, `WATERMARK/AGGELIA/`.

### UI Design Rules
- Border radius: 2px everywhere (nearly square corners)
- Minimal: no decorative backgrounds, shadows, gradients, or ornamental elements
- Neutral gray palette with subtle blue accents (OKLCH color tokens in `app.css`)
- No emojis anywhere in the interface
- Compact layout with reduced padding for information density
- SVG icons inline (outlined, 2px stroke) -- no icon libraries

### Pre-commit Hook (Husky)
Automatically runs `node scripts/version.js sync --next` and stages version files.
Do not manually edit version numbers in `package.json`, `tauri.conf.json`, or `Cargo.toml`.
