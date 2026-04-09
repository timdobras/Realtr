/**
 * Regression net for the Tauri command boundary.
 *
 * Each test mocks `@tauri-apps/api/core` and asserts that a public
 * DatabaseService method invokes the right command name with the right
 * argument shape. If anyone renames a Tauri command on the Rust side
 * without updating the service wrapper (or vice versa), at least one of
 * these tests will fail loudly instead of producing a runtime "command
 * not found" deep inside the workflow.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args)
}));

// Import AFTER the mock is registered so the static class binds to the
// mocked invoke.
import { DatabaseService } from './databaseService';

beforeEach(() => {
  invokeMock.mockReset();
});

afterEach(() => {
  invokeMock.mockReset();
});

describe('DatabaseService — property commands', () => {
  it('createProperty invokes create_property with name/city/notes', async () => {
    invokeMock.mockResolvedValue({ success: true });
    await DatabaseService.createProperty('Villa Alpha', 'Athens', 'note');
    expect(invokeMock).toHaveBeenCalledWith('create_property', {
      name: 'Villa Alpha',
      city: 'Athens',
      notes: 'note'
    });
  });

  it('getProperties unwraps result.data array on success', async () => {
    const fake = [
      { id: 1, name: 'A' },
      { id: 2, name: 'B' }
    ];
    invokeMock.mockResolvedValue({ success: true, data: fake });
    const result = await DatabaseService.getProperties();
    expect(invokeMock).toHaveBeenCalledWith('get_properties');
    expect(result).toEqual(fake);
  });

  it('getProperties returns [] on { success: false }', async () => {
    invokeMock.mockResolvedValue({ success: false, error: 'boom' });
    const result = await DatabaseService.getProperties();
    expect(result).toEqual([]);
  });

  it('getPropertiesByStatus passes the status arg', async () => {
    invokeMock.mockResolvedValue({ success: true, data: [] });
    await DatabaseService.getPropertiesByStatus('DONE');
    expect(invokeMock).toHaveBeenCalledWith('get_properties_by_status', { status: 'DONE' });
  });

  it('updatePropertyStatus uses camelCase propertyId/newStatus', async () => {
    invokeMock.mockResolvedValue({ success: true });
    await DatabaseService.updatePropertyStatus(42, 'ARCHIVE');
    expect(invokeMock).toHaveBeenCalledWith('update_property_status', {
      propertyId: 42,
      newStatus: 'ARCHIVE'
    });
  });

  it('deleteProperty passes propertyId', async () => {
    invokeMock.mockResolvedValue({ success: true });
    await DatabaseService.deleteProperty(7);
    expect(invokeMock).toHaveBeenCalledWith('delete_property', { propertyId: 7 });
  });

  it('setPropertyCode passes propertyId + code', async () => {
    invokeMock.mockResolvedValue({ success: true });
    await DatabaseService.setPropertyCode(99, '45164');
    expect(invokeMock).toHaveBeenCalledWith('set_property_code', {
      propertyId: 99,
      code: '45164'
    });
  });

  it('updateProperty coerces empty notes to null', async () => {
    invokeMock.mockResolvedValue({ success: true });
    await DatabaseService.updateProperty(1, 'N', 'C', undefined);
    expect(invokeMock).toHaveBeenCalledWith('update_property', {
      propertyId: 1,
      name: 'N',
      city: 'C',
      notes: null
    });
  });

  it('getPropertyById returns null on missing data', async () => {
    invokeMock.mockResolvedValue({ success: false });
    const result = await DatabaseService.getPropertyById(123);
    expect(result).toBeNull();
  });
});

describe('DatabaseService — city commands', () => {
  it('getCities invokes get_cities', async () => {
    invokeMock.mockResolvedValue({ success: true, data: [] });
    await DatabaseService.getCities();
    expect(invokeMock).toHaveBeenCalledWith('get_cities');
  });

  it('searchCities passes query', async () => {
    invokeMock.mockResolvedValue({ success: true, data: [] });
    await DatabaseService.searchCities('athens');
    expect(invokeMock).toHaveBeenCalledWith('search_cities', { query: 'athens' });
  });
});

describe('DatabaseService — thumbnail / asset-protocol commands', () => {
  it('getGalleryThumbnailPath passes all args', async () => {
    invokeMock.mockResolvedValue('/path/to/thumb.jpg');
    await DatabaseService.getGalleryThumbnailPath(
      'Athens/Villa',
      'NEW',
      'INTERNET',
      'IMG_001.jpg',
      800
    );
    expect(invokeMock).toHaveBeenCalledWith('get_gallery_thumbnail_path', {
      folderPath: 'Athens/Villa',
      status: 'NEW',
      subfolder: 'INTERNET',
      filename: 'IMG_001.jpg',
      maxDimension: 800
    });
  });

  it('getThumbnailPathsBatch passes the properties array', async () => {
    invokeMock.mockResolvedValue([]);
    const props = [{ folderPath: 'Athens/Villa', status: 'NEW' as const, limit: 6 }];
    await DatabaseService.getThumbnailPathsBatch(props);
    expect(invokeMock).toHaveBeenCalledWith('get_thumbnail_paths_batch', {
      properties: props
    });
  });

  it('pregenerateGalleryThumbnails passes folder/status/subfolder', async () => {
    invokeMock.mockResolvedValue({ success: true });
    await DatabaseService.pregenerateGalleryThumbnails('Athens/Villa', 'NEW', 'INTERNET');
    expect(invokeMock).toHaveBeenCalledWith('pregenerate_gallery_thumbnails', {
      folderPath: 'Athens/Villa',
      status: 'NEW',
      subfolder: 'INTERNET',
      maxDimension: undefined
    });
  });
});

describe('DatabaseService — sets commands', () => {
  it('completeSet invokes complete_set', async () => {
    invokeMock.mockResolvedValue({ setId: 1 });
    await DatabaseService.completeSet();
    expect(invokeMock).toHaveBeenCalledWith('complete_set');
  });

  it('getSets unwraps data on success', async () => {
    invokeMock.mockResolvedValue({ success: true, data: [{ id: 1 }] });
    const result = await DatabaseService.getSets();
    expect(invokeMock).toHaveBeenCalledWith('get_sets');
    expect(result).toEqual([{ id: 1 }]);
  });

  it('getSetProperties passes setId', async () => {
    invokeMock.mockResolvedValue({ success: true, data: [] });
    await DatabaseService.getSetProperties(5);
    expect(invokeMock).toHaveBeenCalledWith('get_set_properties', { setId: 5 });
  });

  it('deleteSet defaults deleteZip to false', async () => {
    invokeMock.mockResolvedValue({ success: true });
    await DatabaseService.deleteSet(5);
    expect(invokeMock).toHaveBeenCalledWith('delete_set', { setId: 5, deleteZip: false });
  });

  it('deleteSet passes deleteZip when explicit', async () => {
    invokeMock.mockResolvedValue({ success: true });
    await DatabaseService.deleteSet(5, true);
    expect(invokeMock).toHaveBeenCalledWith('delete_set', { setId: 5, deleteZip: true });
  });
});

describe('DatabaseService — editor commands', () => {
  it('openImagesInFolder passes folderPath/status/selectedImage', async () => {
    invokeMock.mockResolvedValue({ success: true });
    await DatabaseService.openImagesInFolder('Athens/Villa', 'NEW', 'IMG_001.jpg');
    expect(invokeMock).toHaveBeenCalledWith('open_images_in_folder', {
      folderPath: 'Athens/Villa',
      status: 'NEW',
      selectedImage: 'IMG_001.jpg'
    });
  });
});

describe('DatabaseService — scan + repair', () => {
  it('scanAndImportProperties returns scan result on success', async () => {
    invokeMock.mockResolvedValue({
      success: true,
      data: { foundProperties: 2, newProperties: 1, existingProperties: 1, errors: [] }
    });
    const result = await DatabaseService.scanAndImportProperties();
    expect(invokeMock).toHaveBeenCalledWith('scan_and_import_properties');
    expect(result?.foundProperties).toBe(2);
  });

  it('scanAndImportProperties throws on failure', async () => {
    invokeMock.mockResolvedValue({ success: false, error: 'oops' });
    await expect(DatabaseService.scanAndImportProperties()).rejects.toThrow('oops');
  });

  it('repairPropertyStatuses returns null on no data', async () => {
    invokeMock.mockResolvedValue({ success: false });
    const result = await DatabaseService.repairPropertyStatuses();
    expect(result).toBeNull();
  });
});
