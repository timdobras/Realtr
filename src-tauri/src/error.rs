//! Application-wide error type for Tauri commands.
//!
//! Replaces the historical `Result<_, String>` pattern. `AppError`:
//! - is `thiserror`-derived so call sites can use `?` against `std::io`,
//!   `sqlx`, `serde_json`, `image`, and our own [`crate::paths::PathError`];
//! - serializes to a tagged JSON object (`{ kind, message }`) so the
//!   frontend can switch on `kind` for localized messages or recovery UI;
//! - implements `Display` so legacy `Result<_, String>` commands can still
//!   convert via `.to_string()` during the migration.

// AppError is introduced ahead of the command-by-command migration
// from `Result<_, String>`. Suppress dead-code warnings on the
// unused-yet variants and the AppResult alias until call sites adopt them.
#![allow(dead_code)]

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),

    #[error("image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("invalid path: {0}")]
    Path(#[from] crate::paths::PathError),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("{0}")]
    Other(String),
}

impl AppError {
    /// Stable machine-readable kind, used by the frontend for branching.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Config(_) => "config",
            AppError::Database(_) => "database",
            AppError::Io(_) => "io",
            AppError::Image(_) => "image",
            AppError::Path(_) => "path",
            AppError::Serde(_) => "serde",
            AppError::NotFound(_) => "not_found",
            AppError::InvalidInput(_) => "invalid_input",
            AppError::Other(_) => "other",
        }
    }
}

/// Tauri serializes command errors via `serde::Serialize`. We emit a tagged
/// object so the frontend can do `if (err.kind === "not_found") ...`.
impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

/// Convenience alias for command return types.
pub type AppResult<T> = Result<T, AppError>;

/// Bridge for legacy commands that still return `Result<_, String>`.
/// Lets us migrate sites incrementally without a flag-day rewrite.
impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_is_stable() {
        let e = AppError::NotFound("property 42".into());
        assert_eq!(e.kind(), "not_found");
    }

    #[test]
    fn serializes_as_tagged_object() {
        let e = AppError::InvalidInput("missing name".into());
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["kind"], "invalid_input");
        assert_eq!(json["message"], "invalid input: missing name");
    }

    #[test]
    fn from_io_error_works() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        let e: AppError = io.into();
        assert_eq!(e.kind(), "io");
    }

    #[test]
    fn from_path_error_works() {
        let pe = crate::paths::PathError::UnsafeSegment("..".into());
        let e: AppError = pe.into();
        assert_eq!(e.kind(), "path");
    }

    #[test]
    fn legacy_string_bridge_preserves_message() {
        let e = AppError::Other("boom".into());
        let s: String = e.into();
        assert_eq!(s, "boom");
    }
}
