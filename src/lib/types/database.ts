// ─── Generated types ─────────────────────────────────────────────────
// These come from src-tauri/src/database.rs via ts-rs. Do NOT edit them
// here — change the Rust struct and run `cargo test export_bindings`.
// The generated files live under ./generated/.
//
// We re-export them as `interface` extensions so the rest of the
// codebase keeps using familiar type names like `Property` and the
// PropertyStatus union still narrows the `status` field.
import type { Property as GeneratedProperty } from './generated/Property';
import type { City } from './generated/City';
import type { ScanResult } from './generated/ScanResult';
import type { CommandResult as GeneratedCommandResult } from './generated/CommandResult';
import type { Set } from './generated/Set';
import type { SetProperty } from './generated/SetProperty';
import type { CompleteSetResult } from './generated/CompleteSetResult';
import type { RepairResult } from './generated/RepairResult';
import type { ThumbnailBatchRequest } from './generated/ThumbnailBatchRequest';
import type { ThumbnailBatchResult } from './generated/ThumbnailBatchResult';

export type { City, ScanResult, Set, SetProperty, CompleteSetResult, RepairResult };
export type { ThumbnailBatchRequest, ThumbnailBatchResult };

export type PropertyStatus = 'NEW' | 'DONE' | 'NOT_FOUND' | 'ARCHIVE';

// Property is generated with `status: string`; narrow it to the literal
// union here so call sites get exhaustiveness checking.
export type Property = Omit<GeneratedProperty, 'status'> & { status: PropertyStatus };

// CommandResult is generated with `data: unknown`; widen back to `any`
// for the legacy call sites that pass a serde_json::Value through and
// dot-access arbitrary fields. The Rust source of truth is still strict.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type CommandResult = Omit<GeneratedCommandResult, 'data'> & { data?: any };

export interface WatermarkConfig {
  sizeMode: 'proportional' | 'fit' | 'stretch' | 'tile';
  sizePercentage: number; // 0.0 to 1.0
  relativeTo: 'longest-side' | 'shortest-side' | 'width' | 'height';
  positionAnchor:
    | 'top-left'
    | 'top-center'
    | 'top-right'
    | 'center-left'
    | 'center'
    | 'center-right'
    | 'bottom-left'
    | 'bottom-center'
    | 'bottom-right';
  offsetX: number;
  offsetY: number;
  opacity: number; // 0.0 to 1.0
  useAlphaChannel: boolean;
}

export interface AppConfig {
  // Legacy field for backward compatibility
  rootPath?: string;
  // New modular folder paths
  newFolderPath: string;
  doneFolderPath: string;
  notFoundFolderPath: string;
  archiveFolderPath: string;
  setsFolderPath: string;
  isValidPath: boolean;
  lastUpdated: string | null;
  // Image editor settings
  use_builtin_editor?: boolean;
  fast_editor_path?: string;
  fast_editor_name?: string;
  complex_editor_path?: string;
  complex_editor_name?: string;
  watermark_image_path?: string;
  watermarkConfig: WatermarkConfig;
  // Legacy field for backward compatibility
  watermark_opacity?: number;
}

// Perspective Correction Types (LSD + RANSAC)
export interface CorrectionResult {
  original_filename: string;
  original_path: string;
  corrected_temp_path: string;
  confidence: number; // 0.0-1.0
  rotation_applied: number; // degrees
  needs_correction: boolean;
  corrected_preview_base64?: string;
}

export interface AcceptedCorrection {
  original_path: string;
  corrected_temp_path: string;
}

export interface PerspectiveCommandResult {
  success: boolean;
  error?: string;
  message?: string;
}

// OpenCV Setup Types
export interface OpenCVStatus {
  installed: boolean;
  dll_path?: string;
  message: string;
}

export interface SetupProgress {
  step: number;
  total_steps: number;
  message: string;
  complete: boolean;
  error?: string;
}

// Sets / Repair types are now generated from Rust — see the
// re-exports at the top of this file.

// Batch Auto-Enhance Types
export interface StraightenAnalysis {
  rotation: number; // degrees
  confidence: number; // 0.0-1.0
  lines_used: number;
  vh_agreement: boolean;
}

export interface AdjustmentAnalysis {
  brightness: number; // -100 to 100
  exposure: number; // -100 to 100
  contrast: number; // -100 to 100
  highlights: number; // -100 to 100
  shadows: number; // -100 to 100
  magnitude: number; // 0.0-1.0
}

export interface EnhanceAnalysisResult {
  filename: string;
  original_path: string;
  straighten: StraightenAnalysis;
  adjustments: AdjustmentAnalysis;
  combined_confidence: number; // 0.0-1.0
  needs_enhancement: boolean;
  preview_base64: string;
  original_preview_base64: string;
}

export interface EnhanceRequest {
  filename: string;
  original_path: string;
  rotation: number;
  brightness: number;
  exposure: number;
  contrast: number;
  highlights: number;
  shadows: number;
}

export interface EnhanceApplyResult {
  filename: string;
  success: boolean;
  error?: string;
}

// ThumbnailBatchRequest / ThumbnailBatchResult are now generated from
// Rust — see the re-exports at the top of this file.
