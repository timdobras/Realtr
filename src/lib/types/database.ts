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
