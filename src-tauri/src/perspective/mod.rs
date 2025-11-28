//! Perspective correction module for automatic image straightening.
//!
//! Uses Line Segment Detection (LSD) and RANSAC to detect vertical lines
//! and applies rotation transforms to make them truly vertical.

pub mod commands;
pub mod detection;
pub mod model;
pub mod rectification;

use serde::{Deserialize, Serialize};

/// A detected vanishing point in the image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VanishingPoint {
    /// X coordinate (can be outside image bounds or at infinity)
    pub x: f64,
    /// Y coordinate (can be outside image bounds or at infinity)
    pub y: f64,
    /// Confidence score 0.0-1.0
    pub confidence: f32,
    /// Type of vanishing point
    pub vp_type: VanishingPointType,
}

/// Type of vanishing point
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VanishingPointType {
    /// Vertical lines (walls, door frames)
    Vertical,
    /// Horizontal left vanishing point
    HorizontalLeft,
    /// Horizontal right vanishing point
    HorizontalRight,
}

/// Result of perspective analysis for a single image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerspectiveAnalysis {
    /// Detected vanishing points
    pub vanishing_points: Vec<VanishingPoint>,
    /// Suggested rotation in degrees
    pub suggested_rotation: f64,
    /// Confidence in the analysis (0.0-1.0)
    pub confidence: f32,
    /// Whether the image needs correction
    pub needs_correction: bool,
    /// Number of vertical lines detected
    pub lines_detected: usize,
}

/// Result of processing a single image for perspective correction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionResult {
    /// Original filename
    pub original_filename: String,
    /// Full path to original image
    pub original_path: String,
    /// Path to the corrected image in temp storage
    pub corrected_temp_path: String,
    /// Confidence of the detection (0.0-1.0)
    pub confidence: f32,
    /// Rotation applied in degrees
    pub rotation_applied: f64,
    /// Whether correction was needed/applied
    pub needs_correction: bool,
    /// Base64 encoded preview of corrected image (for UI display)
    pub corrected_preview_base64: Option<String>,
}

/// Request to accept specific corrections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedCorrection {
    /// Path to the original image to overwrite
    pub original_path: String,
    /// Path to the corrected temp image
    pub corrected_temp_path: String,
}

/// Standard command result for perspective operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerspectiveCommandResult {
    pub success: bool,
    pub error: Option<String>,
    pub message: Option<String>,
}

impl PerspectiveCommandResult {
    pub fn success(message: &str) -> Self {
        Self {
            success: true,
            error: None,
            message: Some(message.to_string()),
        }
    }

    pub fn error(error: &str) -> Self {
        Self {
            success: false,
            error: Some(error.to_string()),
            message: None,
        }
    }
}

// ============================================================================
// LSD + RANSAC Algorithm Parameters (Conservative settings for reliability)
// ============================================================================

/// Lines within ±10° of vertical are considered vertical (very strict)
pub const VERTICAL_TOLERANCE_DEG: f64 = 10.0;

/// Minimum line length as ratio of image height (20% = only very long lines)
/// This ensures we're detecting architectural elements, not furniture/decor
pub const MIN_LINE_LENGTH_RATIO: f64 = 0.20;

/// Number of RANSAC iterations (higher = more thorough search)
pub const RANSAC_ITERATIONS: usize = 500;

/// Angle tolerance for RANSAC inliers (±2° = tight clustering required)
pub const RANSAC_INLIER_THRESHOLD_DEG: f64 = 2.0;

/// Minimum rotation to apply (skip if less than 0.3°)
pub const MIN_ROTATION_THRESHOLD_DEG: f64 = 0.3;

/// Maximum rotation to apply (reject if more than 15° - such images need manual review)
pub const MAX_ROTATION_DEG: f64 = 15.0;

/// Minimum confidence required to apply correction (0.5 = at least 50% of line weight agrees)
pub const CONFIDENCE_THRESHOLD: f32 = 0.5;

/// Minimum number of inlier lines required for reliable detection
pub const MIN_INLIER_COUNT: usize = 3;

/// Maximum standard deviation of inlier angles (in degrees)
/// If lines vary too much, the detection is ambiguous
pub const MAX_ANGLE_STDDEV_DEG: f64 = 2.5;
