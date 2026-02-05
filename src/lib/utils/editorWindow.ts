import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

interface OpenEditorParams {
  folderPath: string;
  status: string;
  subfolder: string;
  filename: string;
}

/**
 * Opens the image editor in a new window
 */
export async function openEditorWindow(params: OpenEditorParams): Promise<void> {
  const { folderPath, status, subfolder, filename } = params;

  // Create URL with query parameters
  const searchParams = new URLSearchParams({
    folderPath,
    status,
    subfolder,
    filename
  });

  // Generate a unique window label based on the filename
  const windowLabel = `editor-${Date.now()}`;

  console.log('Opening editor window:', { windowLabel, params });

  try {
    // Create the new window
    const editorWindow = new WebviewWindow(windowLabel, {
      url: `/editor?${searchParams.toString()}`,
      title: `Image Editor - ${filename}`,
      width: 1400,
      height: 900,
      minWidth: 800,
      minHeight: 600,
      center: true,
      resizable: true,
      decorations: false,
      focus: true
    });

    // Handle window creation errors
    editorWindow.once('tauri://error', (e) => {
      console.error('Failed to create editor window:', e);
    });

    // Wait for the window to be created
    editorWindow.once('tauri://created', () => {
      console.log('Editor window created successfully');
    });
  } catch (error) {
    console.error('Error creating editor window:', error);
    throw error;
  }
}
