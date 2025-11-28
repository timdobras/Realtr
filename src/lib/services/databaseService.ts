import type { AppConfig } from '$lib/utils/configManager';
import { invoke } from '@tauri-apps/api/core';
import type {
  AcceptedCorrection,
  City,
  CommandResult,
  CorrectionResult,
  OpenCVStatus,
  PerspectiveCommandResult,
  Property,
  PropertyStatus,
  ScanResult,
  SetupProgress
} from '../types/database';

export class DatabaseService {
  // Property operations
  static async createProperty(name: string, city: string, notes?: string): Promise<CommandResult> {
    return await invoke<CommandResult>('create_property', {
      name,
      city,
      notes
    });
  }

  static async getProperties(): Promise<Property[]> {
    const result = await invoke<CommandResult>('get_properties');
    if (result.success && result.data) {
      return result.data as Property[];
    }
    return [];
  }

  static async getPropertiesByStatus(status: PropertyStatus): Promise<Property[]> {
    const result = await invoke<CommandResult>('get_properties_by_status', {
      status
    });
    if (result.success && result.data) {
      return result.data as Property[];
    }
    return [];
  }

  static async updatePropertyStatus(
    propertyId: number,
    newStatus: PropertyStatus
  ): Promise<CommandResult> {
    return await invoke<CommandResult>('update_property_status', {
      propertyId,
      newStatus
    });
  }

  static async deleteProperty(propertyId: number): Promise<CommandResult> {
    return await invoke<CommandResult>('delete_property', {
      propertyId
    });
  }

  static async setPropertyCode(propertyId: number, code: string): Promise<CommandResult> {
    return await invoke<CommandResult>('set_property_code', {
      propertyId,
      code
    });
  }

  static async getPropertyById(propertyId: number): Promise<Property | null> {
    const result = await invoke<CommandResult>('get_property_by_id', {
      propertyId
    });
    if (result.success && result.data) {
      return result.data as Property;
    }
    return null;
  }

  // City operations for autocomplete
  static async getCities(): Promise<City[]> {
    const result = await invoke<CommandResult>('get_cities');
    if (result.success && result.data) {
      return result.data as City[];
    }
    return [];
  }

  static async searchCities(query: string): Promise<City[]> {
    const result = await invoke<CommandResult>('search_cities', {
      query
    });
    if (result.success && result.data) {
      return result.data as City[];
    }
    return [];
  }

  static async scanAndImportProperties(): Promise<ScanResult | null> {
    const result = await invoke<CommandResult>('scan_and_import_properties');
    if (result.success && result.data) {
      return result.data as ScanResult;
    }
    throw new Error(result.error || 'Failed to scan properties');
  }

  // Add this method to the DatabaseService class
  static async openImagesInFolder(
    folderPath: string,
    status: string,
    selectedImage: string
  ): Promise<CommandResult> {
    return await invoke<CommandResult>('open_images_in_folder', {
      folderPath,
      status,
      selectedImage
    });
  }

  // Add these methods to your DatabaseService class
  static async getEditorConfig(): Promise<AppConfig | null> {
    const result = await invoke<CommandResult>('load_config');
    if (result.success && result.data) {
      return result.data as AppConfig;
    }
    return null;
  }

  static async openWithConfiguredEditor(
    propertyId: number,
    filename: string,
    editorType: 'fast' | 'complex',
    folderType: 'original' | 'internet' | 'aggelia' = 'internet'
  ): Promise<CommandResult> {
    const property = await this.getPropertyById(propertyId);
    if (!property) {
      throw new Error('Property not found');
    }

    if (editorType === 'fast') {
      return await invoke<CommandResult>('open_image_in_editor', {
        folderPath: property.folder_path,
        status: property.status,
        filename,
        isFromInternet: folderType === 'internet'
      });
    } else {
      return await invoke<CommandResult>('open_image_in_advanced_editor', {
        folderPath: property.folder_path,
        status: property.status,
        filename,
        fromAggelia: folderType === 'aggelia'
      });
    }
  }

  // Perspective Correction Operations (LSD + RANSAC)
  static async processImagesForPerspective(
    folderPath: string,
    status: string,
    propertyId: number
  ): Promise<CorrectionResult[]> {
    return await invoke<CorrectionResult[]>('process_images_for_perspective', {
      folderPath,
      status,
      propertyId
    });
  }

  static async acceptPerspectiveCorrections(
    corrections: AcceptedCorrection[]
  ): Promise<PerspectiveCommandResult> {
    return await invoke<PerspectiveCommandResult>('accept_perspective_corrections', {
      corrections
    });
  }

  static async cleanupPerspectiveTemp(): Promise<void> {
    return await invoke<void>('cleanup_perspective_temp');
  }

  static async getOriginalImageForComparison(imagePath: string): Promise<string> {
    return await invoke<string>('get_original_image_for_comparison', {
      imagePath
    });
  }

  // OpenCV Setup Operations
  static async checkOpenCVStatus(): Promise<OpenCVStatus> {
    return await invoke<OpenCVStatus>('check_opencv_status');
  }

  static async runOpenCVSetup(): Promise<SetupProgress> {
    return await invoke<SetupProgress>('run_opencv_setup');
  }

  static async skipOpenCVSetup(): Promise<void> {
    return await invoke<void>('skip_opencv_setup');
  }

  static async wasOpenCVSetupSkipped(): Promise<boolean> {
    return await invoke<boolean>('was_opencv_setup_skipped');
  }

  static async resetOpenCVSetupSkip(): Promise<void> {
    return await invoke<void>('reset_opencv_setup_skip');
  }

  // Fill AGGELIA folders to 25 images
  static async fillAggeliaTo25(folderPath: string, status: string): Promise<CommandResult> {
    return await invoke<CommandResult>('fill_aggelia_to_25', {
      folderPath,
      status
    });
  }
}
