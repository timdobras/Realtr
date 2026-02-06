export type PropertyStatus = 'NEW' | 'DONE' | 'NOT_FOUND' | 'ARCHIVE';

export interface Property {
  id?: number;
  name: string;
  city: string;
  status: PropertyStatus;
  folder_path: string;
  notes?: string;
  code?: string; // Website listing code (e.g., "45164")
  created_at: number; // Milliseconds since epoch
  updated_at: number; // Milliseconds since epoch
}

export interface City {
  id?: number;
  name: string;
  usageCount: number;
  created_at: number; // Milliseconds since epoch
}

export interface ScanResult {
  foundProperties: number;
  newProperties: number;
  existingProperties: number;
  errors: string[];
}

export interface CommandResult {
  success: boolean;
  error?: string;
  data?: any;
}

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

// Sets Types
export interface Set {
  id?: number;
  name: string;
  zip_path: string;
  property_count: number;
  created_at: number; // Milliseconds since epoch
}

export interface SetProperty {
  id?: number;
  setId: number;
  propertyId?: number;
  propertyName: string;
  propertyCity: string;
  propertyCode?: string;
}

export interface CompleteSetResult {
  setId: number;
  setName: string;
  zipPath: string;
  propertiesArchived: number;
  propertiesMovedToNotFound: number;
}

// Repair Types
export interface RepairResult {
  propertiesChecked: number;
  propertiesFixed: number;
  errors: string[];
}

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
