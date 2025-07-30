import { invoke } from "@tauri-apps/api/core";
import type {
	Property,
	City,
	CommandResult,
	ScanResult,
} from "../types/database";
import type { AppConfig } from "$lib/utils/configManager";

export class DatabaseService {
	// Property operations
	static async createProperty(
		name: string,
		city: string,
		notes?: string
	): Promise<CommandResult> {
		return await invoke<CommandResult>("create_property", {
			name,
			city,
			notes,
		});
	}

	static async getProperties(): Promise<Property[]> {
		const result = await invoke<CommandResult>("get_properties");
		if (result.success && result.data) {
			return result.data as Property[];
		}
		return [];
	}

	static async getPropertiesByStatus(
		completed: boolean
	): Promise<Property[]> {
		const result = await invoke<CommandResult>("get_properties_by_status", {
			completed,
		});
		if (result.success && result.data) {
			return result.data as Property[];
		}
		return [];
	}

	static async updatePropertyStatus(
		propertyId: number,
		completed: boolean
	): Promise<CommandResult> {
		return await invoke<CommandResult>("update_property_status", {
			propertyId,
			completed,
		});
	}

	static async deleteProperty(propertyId: number): Promise<CommandResult> {
		return await invoke<CommandResult>("delete_property", {
			propertyId,
		});
	}

	static async getPropertyById(propertyId: number): Promise<Property | null> {
		const result = await invoke<CommandResult>("get_property_by_id", {
			propertyId,
		});
		if (result.success && result.data) {
			return result.data as Property;
		}
		return null;
	}

	// City operations for autocomplete
	static async getCities(): Promise<City[]> {
		const result = await invoke<CommandResult>("get_cities");
		if (result.success && result.data) {
			return result.data as City[];
		}
		return [];
	}

	static async searchCities(query: string): Promise<City[]> {
		const result = await invoke<CommandResult>("search_cities", {
			query,
		});
		if (result.success && result.data) {
			return result.data as City[];
		}
		return [];
	}

	static async scanAndImportProperties(): Promise<ScanResult | null> {
		const result = await invoke<CommandResult>(
			"scan_and_import_properties"
		);
		if (result.success && result.data) {
			return result.data as ScanResult;
		}
		throw new Error(result.error || "Failed to scan properties");
	}

	// Add this method to the DatabaseService class
	static async openImagesInFolder(
		folderPath: string,
		selectedImage: string
	): Promise<CommandResult> {
		return await invoke<CommandResult>("open_images_in_folder", {
			folderPath,
			selectedImage,
		});
	}

	// Add these methods to your DatabaseService class
	static async getEditorConfig(): Promise<AppConfig | null> {
		const result = await invoke<CommandResult>("load_config");
		if (result.success && result.data) {
			return result.data as AppConfig;
		}
		return null;
	}

	static async openWithConfiguredEditor(
		propertyId: number,
		filename: string,
		editorType: "fast" | "complex",
		folderType: "original" | "internet" | "aggelia" = "internet"
	): Promise<CommandResult> {
		const property = await this.getPropertyById(propertyId);
		if (!property) {
			throw new Error("Property not found");
		}

		if (editorType === "fast") {
			return await invoke<CommandResult>("open_image_in_editor", {
				folderPath: property.folder_path,
				filename,
				isFromInternet: folderType === "internet",
			});
		} else {
			return await invoke<CommandResult>(
				"open_image_in_advanced_editor",
				{
					folderPath: property.folder_path,
					filename,
					fromAggelia: folderType === "aggelia",
				}
			);
		}
	}
}
