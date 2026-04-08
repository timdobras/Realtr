//! Centralized folder-name constants for the on-disk property structure.
//!
//! The app expects a specific layout with Greek-derived names that are
//! historically scattered as string literals across the codebase. Concentrating
//! them here makes a future rename or i18n a single-file change.
//!
//! ```text
//! {root}/
//! ├── FOTOGRAFIES - NEW/
//! │   └── {City}/
//! │       └── {Property}/
//! │           ├── INTERNET/
//! │           │   └── AGGELIA/
//! │           └── WATERMARK/
//! │               └── AGGELIA/
//! └── FOTOGRAFIES - DONE/
//!     └── ...
//! ```

/// Folder containing processed (renamed/ordered) property images.
pub const INTERNET: &str = "INTERNET";

/// Subfolder of INTERNET / WATERMARK holding the advanced-edit selection.
pub const AGGELIA: &str = "AGGELIA";

/// Folder containing watermarked outputs.
pub const WATERMARK: &str = "WATERMARK";

/// Top-level folder for incomplete properties.
pub const FOTOGRAFIES_NEW: &str = "FOTOGRAFIES - NEW";

/// Top-level folder for completed properties.
pub const FOTOGRAFIES_DONE: &str = "FOTOGRAFIES - DONE";

/// Top-level folder for properties whose source files are missing.
pub const NOT_FOUND: &str = "NOT FOUND";

/// Top-level folder for archived properties.
pub const ARCHIVE: &str = "ARCHIVE";
