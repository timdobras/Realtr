//! Path-safety helpers.
//!
//! All filesystem operations that combine a trusted root (e.g. the configured
//! `new_folder_path`) with untrusted user-supplied segments (property names,
//! city names, image filenames) MUST go through [`safe_join`] so that a
//! malicious or accidental `..`, absolute path, or drive prefix cannot escape
//! the configured root.
//!
//! The check is purely lexical — it does not require the path to exist on
//! disk, so it is safe to use for both read and write operations.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("unsafe path segment: {0:?}")]
    UnsafeSegment(PathBuf),
}

/// Joins one or more untrusted path segments onto a trusted root, rejecting
/// any segment that would escape the root.
///
/// Rejected components: `..` (ParentDir), absolute roots (`/`, `\\`),
/// Windows drive prefixes (`C:`). `.` (CurDir) is silently skipped.
/// Normal components are pushed as-is.
///
/// # Errors
/// Returns [`PathError::UnsafeSegment`] if `untrusted` contains any
/// component that could escape `root`.
pub fn safe_join(root: &Path, untrusted: impl AsRef<Path>) -> Result<PathBuf, PathError> {
    let untrusted = untrusted.as_ref();
    let mut result = PathBuf::from(root);
    for component in untrusted.components() {
        match component {
            Component::Normal(seg) => result.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PathError::UnsafeSegment(untrusted.to_path_buf()));
            }
        }
    }
    Ok(result)
}

/// Validates a stored relative folder path (e.g. `"Athens/Villa Alpha"`)
/// and returns it as a [`PathBuf`]. Splits on `/` so the same database
/// strings work on both Unix and Windows. Each component is checked
/// against the same rules as [`safe_join`].
///
/// Use this for any path read out of the database before joining it
/// onto a trusted base path.
///
/// # Errors
/// Returns [`PathError::UnsafeSegment`] if any component would escape
/// a containing root.
pub fn validate_relative_folder_path(folder_path: &str) -> Result<PathBuf, PathError> {
    let mut result = PathBuf::new();
    for raw in folder_path.split('/') {
        if raw.is_empty() {
            // Skip empty segments from leading/trailing/double slashes.
            continue;
        }
        let candidate = Path::new(raw);
        for component in candidate.components() {
            match component {
                Component::Normal(seg) => result.push(seg),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(PathError::UnsafeSegment(folder_path.into()));
                }
            }
        }
    }
    Ok(result)
}

/// Convenience for joining multiple untrusted segments in order. Each segment
/// is validated independently with the same rules as [`safe_join`].
///
/// # Errors
/// Returns [`PathError::UnsafeSegment`] for any segment that could escape `root`.
pub fn safe_join_all<I, P>(root: &Path, segments: I) -> Result<PathBuf, PathError>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut result = PathBuf::from(root);
    for seg in segments {
        result = safe_join(&result, seg)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(r"C:\photos")
        } else {
            PathBuf::from("/photos")
        }
    }

    #[test]
    fn joins_normal_segments() {
        let r = root();
        let p = safe_join(&r, "Athens").unwrap();
        assert_eq!(p, r.join("Athens"));
    }

    #[test]
    fn joins_multi_segment_relative_path() {
        let r = root();
        let p = safe_join(&r, "Athens/PropertyOne").unwrap();
        assert_eq!(p, r.join("Athens").join("PropertyOne"));
    }

    #[test]
    fn rejects_parent_dir() {
        let r = root();
        assert!(safe_join(&r, "..").is_err());
        assert!(safe_join(&r, "Athens/../..").is_err());
        assert!(safe_join(&r, "../escape").is_err());
    }

    #[test]
    fn rejects_absolute_unix_path() {
        let r = root();
        assert!(safe_join(&r, "/etc/passwd").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn rejects_windows_drive_prefix() {
        let r = root();
        assert!(safe_join(&r, r"D:\Windows").is_err());
        assert!(safe_join(&r, r"C:\Windows\System32").is_err());
    }

    #[test]
    fn skips_cur_dir() {
        let r = root();
        let p = safe_join(&r, "./Athens").unwrap();
        assert_eq!(p, r.join("Athens"));
    }

    #[test]
    fn safe_join_all_chains_segments() {
        let r = root();
        let p = safe_join_all(&r, ["Athens", "PropertyOne", "INTERNET"]).unwrap();
        assert_eq!(p, r.join("Athens").join("PropertyOne").join("INTERNET"));
    }

    #[test]
    fn safe_join_all_rejects_any_bad_segment() {
        let r = root();
        assert!(safe_join_all(&r, ["Athens", "..", "INTERNET"]).is_err());
    }

    #[test]
    fn validate_relative_accepts_typical_db_path() {
        let p = validate_relative_folder_path("Athens/Villa Alpha").unwrap();
        assert_eq!(p, PathBuf::from("Athens").join("Villa Alpha"));
    }

    #[test]
    fn validate_relative_accepts_single_segment() {
        let p = validate_relative_folder_path("OnlyCity").unwrap();
        assert_eq!(p, PathBuf::from("OnlyCity"));
    }

    #[test]
    fn validate_relative_accepts_empty_string() {
        let p = validate_relative_folder_path("").unwrap();
        assert_eq!(p, PathBuf::new());
    }

    #[test]
    fn validate_relative_skips_double_slashes() {
        let p = validate_relative_folder_path("Athens//Villa").unwrap();
        assert_eq!(p, PathBuf::from("Athens").join("Villa"));
    }

    #[test]
    fn validate_relative_rejects_parent_traversal() {
        assert!(validate_relative_folder_path("Athens/../etc").is_err());
        assert!(validate_relative_folder_path("../escape").is_err());
        assert!(validate_relative_folder_path("..").is_err());
    }

    #[test]
    fn validate_relative_normalizes_leading_slash_to_relative() {
        // The split('/') design treats '/' purely as the DB segment separator,
        // so a leading slash collapses into a leading empty segment that we
        // skip. The result is a *relative* path that cannot escape the root
        // it eventually gets joined onto, which is the security guarantee
        // we care about here.
        let p = validate_relative_folder_path("/etc/passwd").unwrap();
        assert_eq!(p, PathBuf::from("etc").join("passwd"));
        assert!(p.is_relative());
    }

    #[cfg(windows)]
    #[test]
    fn validate_relative_rejects_drive_prefix() {
        assert!(validate_relative_folder_path(r"C:\Windows").is_err());
    }

    #[test]
    fn rejects_traversal_via_embedded_parent() {
        let r = root();
        // Even if hidden mid-path, ParentDir is rejected.
        assert!(safe_join(&r, "Athens/../../etc").is_err());
    }
}
