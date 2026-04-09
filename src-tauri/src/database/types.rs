//! Wire-format struct definitions for everything stored in the SQLite
//! database or returned to the frontend. Each struct also derives ts-rs
//! `TS` so the matching TypeScript file in src/lib/types/generated/
//! stays in sync — run `cargo test export_bindings` to regenerate.
//!
//! Extracted from database.rs in the database-module split.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// All TS-derived structs export to src/lib/types/generated/. Run
// `cargo test export_bindings` (or just `cargo test`) to regenerate.
// chrono::DateTime<Utc> is serialized to JSON as a millisecond number via
// the ts_milliseconds serde adapter, so we override the TS type to `number`.

// All TS-derived structs export to src/lib/types/generated/. Run
// `cargo test export_bindings` (or just `cargo test`) to regenerate.
//
// Notes on type mappings:
// - chrono::DateTime<Utc> serializes as a JSON millisecond number via the
//   ts_milliseconds adapter, so we override the TS type to `number`.
// - i64 and usize default to `bigint` in ts-rs but the JSON wire format
//   emits them as plain numbers, and every value here fits comfortably in
//   Number.MAX_SAFE_INTEGER (row IDs, counts, epoch ms). We override each
//   integer field to `number` to match runtime reality.

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../../../src/lib/types/generated/")]
pub struct Property {
    #[ts(type = "number | null")]
    pub id: Option<i64>,
    pub name: String,
    pub city: String,
    pub status: String, // "NEW", "DONE", "NOT_FOUND", "ARCHIVE"
    pub folder_path: String,
    pub notes: Option<String>,
    pub code: Option<String>, // Website listing code (e.g., "45164")
    #[serde(with = "chrono::serde::ts_milliseconds")]
    #[ts(type = "number")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    #[ts(type = "number")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // Legacy field for backward compatibility during migration. Skipped in
    // both serde output and the generated TS types.
    #[serde(skip_serializing)]
    #[serde(default)]
    #[ts(skip)]
    pub completed: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../../../src/lib/types/generated/")]
pub struct City {
    #[ts(type = "number | null")]
    pub id: Option<i64>,
    pub name: String,
    #[ts(type = "number")]
    pub usage_count: i64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    #[ts(type = "number")]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/lib/types/generated/")]
pub struct ScanResult {
    #[ts(type = "number")]
    pub found_properties: usize,
    #[ts(type = "number")]
    pub new_properties: usize,
    #[ts(type = "number")]
    pub existing_properties: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/lib/types/generated/")]
pub struct CommandResult {
    pub success: bool,
    pub error: Option<String>,
    // serde_json::Value serializes as arbitrary JSON; mirror that with
    // unknown on the TS side and let call sites narrow it.
    #[ts(type = "unknown")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export, export_to = "../../../src/lib/types/generated/")]
pub struct Set {
    #[ts(type = "number | null")]
    pub id: Option<i64>,
    pub name: String,
    pub zip_path: String,
    #[ts(type = "number")]
    pub property_count: i64,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    #[ts(type = "number")]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/lib/types/generated/")]
pub struct SetProperty {
    #[ts(type = "number | null")]
    pub id: Option<i64>,
    #[ts(type = "number")]
    pub set_id: i64,
    #[ts(type = "number | null")]
    pub property_id: Option<i64>,
    pub property_name: String,
    pub property_city: String,
    pub property_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/lib/types/generated/")]
pub struct CompleteSetResult {
    #[ts(type = "number")]
    pub set_id: i64,
    pub set_name: String,
    pub zip_path: String,
    #[ts(type = "number")]
    pub properties_archived: usize,
    #[ts(type = "number")]
    pub properties_moved_to_not_found: usize,
}

/// Request item for batch thumbnail path resolution.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/lib/types/generated/")]
pub struct ThumbnailBatchRequest {
    pub folder_path: String,
    pub status: String,
    #[ts(type = "number | null")]
    pub limit: Option<usize>,
}

/// Result item from batch thumbnail path resolution.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/lib/types/generated/")]
pub struct ThumbnailBatchResult {
    pub folder_path: String,
    #[ts(type = "number")]
    pub total_count: usize,
    pub paths: Vec<String>,
}
