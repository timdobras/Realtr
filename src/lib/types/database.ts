export interface Property {
  id?: number;
  name: string;
  city: string;
  completed: boolean;
  folder_path: string;
  notes?: string;
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
  positionAnchor: 'top-left' | 'top-center' | 'top-right' | 'center-left' | 'center' | 'center-right' | 'bottom-left' | 'bottom-center' | 'bottom-right';
  offsetX: number;
  offsetY: number;
  opacity: number; // 0.0 to 1.0
  useAlphaChannel: boolean;
}

export interface AppConfig {
  rootPath: string;
  isValidPath: boolean;
  lastUpdated: string | null;
  fast_editor_path?: string;
  fast_editor_name?: string;
  complex_editor_path?: string;
  complex_editor_name?: string;
  watermark_image_path?: string;
  watermarkConfig: WatermarkConfig;
  // Legacy field for backward compatibility
  watermark_opacity?: number;
}
