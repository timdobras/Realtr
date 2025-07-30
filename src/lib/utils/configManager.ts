import { invoke } from '@tauri-apps/api/core';

export interface AppConfig {
  rootPath: string;
  isValidPath: boolean;
  lastUpdated: string | null;
}

export interface CommandResult {
  success: boolean;
  error?: string;
}

export class ConfigManager {
  static async loadConfig(): Promise<AppConfig | null> {
    try {
      return await invoke<AppConfig | null>('load_config');
    } catch (error) {
      console.error('Error loading config:', error);
      return null;
    }
  }

  static async saveConfig(config: AppConfig): Promise<CommandResult> {
    try {
      return await invoke<CommandResult>('save_config', { config });
    } catch (error) {
      console.error('Error saving config:', error);
      throw error;
    }
  }

  static async resetConfig(): Promise<CommandResult> {
    try {
      return await invoke<CommandResult>('reset_config');
    } catch (error) {
      console.error('Error resetting config:', error);
      throw error;
    }
  }

  static async setupFolderStructure(rootPath: string): Promise<CommandResult> {
    try {
      return await invoke<CommandResult>('setup_folder_structure', {
        rootPath
      });
    } catch (error) {
      console.error('Error setting up folder structure:', error);
      throw error;
    }
  }
}
