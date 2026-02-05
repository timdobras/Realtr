// Image Editor Types

export interface EditParams {
  // Crop (normalized coordinates 0-1)
  cropEnabled: boolean;
  cropX: number;
  cropY: number;
  cropWidth: number;
  cropHeight: number;

  // Rotation
  fineRotation: number; // -45 to +45 degrees
  quarterTurns: number; // 0, 1, 2, or 3

  // Adjustments (-100 to 100, default 0)
  brightness: number;
  exposure: number;
  contrast: number;
  highlights: number;
  shadows: number;
}

export interface EditorState {
  crop: {
    enabled: boolean;
    x: number;
    y: number;
    width: number;
    height: number;
  };
  rotation: {
    fine: number;
    quarterTurns: number;
  };
  adjustments: {
    brightness: number;
    exposure: number;
    contrast: number;
    highlights: number;
    shadows: number;
  };
}

export interface HistoryEntry {
  state: EditorState;
  label: string;
}

export type EditorTool = 'rotate' | 'crop' | 'adjust';

// Helper to convert EditorState to EditParams for backend
export function stateToParams(state: EditorState): EditParams {
  return {
    cropEnabled: state.crop.enabled,
    cropX: state.crop.x,
    cropY: state.crop.y,
    cropWidth: state.crop.width,
    cropHeight: state.crop.height,
    fineRotation: state.rotation.fine,
    quarterTurns: state.rotation.quarterTurns,
    brightness: state.adjustments.brightness,
    exposure: state.adjustments.exposure,
    contrast: state.adjustments.contrast,
    highlights: state.adjustments.highlights,
    shadows: state.adjustments.shadows
  };
}

// Create default editor state
export function createDefaultState(): EditorState {
  return {
    crop: {
      enabled: false,
      x: 0,
      y: 0,
      width: 1,
      height: 1
    },
    rotation: {
      fine: 0,
      quarterTurns: 0
    },
    adjustments: {
      brightness: 0,
      exposure: 0,
      contrast: 0,
      highlights: 0,
      shadows: 0
    }
  };
}

// Deep clone state for history
export function cloneState(state: EditorState): EditorState {
  return {
    crop: { ...state.crop },
    rotation: { ...state.rotation },
    adjustments: { ...state.adjustments }
  };
}

// Check if state has any modifications
export function hasModifications(state: EditorState): boolean {
  // Check if crop has been applied (coordinates differ from default full-frame)
  const hasCropModification =
    state.crop.enabled ||
    state.crop.x !== 0 ||
    state.crop.y !== 0 ||
    state.crop.width !== 1 ||
    state.crop.height !== 1;

  return (
    hasCropModification ||
    state.rotation.fine !== 0 ||
    state.rotation.quarterTurns !== 0 ||
    state.adjustments.brightness !== 0 ||
    state.adjustments.exposure !== 0 ||
    state.adjustments.contrast !== 0 ||
    state.adjustments.highlights !== 0 ||
    state.adjustments.shadows !== 0
  );
}
