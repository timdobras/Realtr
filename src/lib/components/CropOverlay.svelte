<script lang="ts">
  // Props
  interface Props {
    cropX: number;
    cropY: number;
    cropWidth: number;
    cropHeight: number;
    zoom: number;
    panX: number;
    panY: number;
    fitScalePercent: number;
    imageWidth: number;
    imageHeight: number;
    containerWidth: number;
    containerHeight: number;
    rotation?: number; // Fine rotation in degrees (-10 to 10)
    isEditing?: boolean; // If false, shows preview mode (no handles, not interactive)
    onCropChange?: (x: number, y: number, width: number, height: number) => void;
    onCropCommit?: () => void;
  }

  let {
    cropX,
    cropY,
    cropWidth,
    cropHeight,
    zoom,
    panX,
    panY,
    fitScalePercent,
    imageWidth,
    imageHeight,
    containerWidth,
    containerHeight,
    rotation = 0,
    isEditing = true,
    onCropChange,
    onCropCommit
  }: Props = $props();

  // Drag state
  type DragMode =
    | 'none'
    | 'move'
    | 'resize-nw'
    | 'resize-ne'
    | 'resize-sw'
    | 'resize-se'
    | 'resize-n'
    | 'resize-s'
    | 'resize-e'
    | 'resize-w';

  let dragMode = $state<DragMode>('none');
  let startMouse = { x: 0, y: 0 };
  let startCrop = { x: 0, y: 0, width: 0, height: 0 };

  // Minimum crop size (5% of image dimension)
  const MIN_CROP_SIZE = 0.05;

  // Calculate the valid crop boundary based on rotation
  // This is the largest axis-aligned rectangle that fits inside the rotated image
  let validCropBounds = $derived.by(() => {
    if (rotation === 0) {
      return { minX: 0, minY: 0, maxX: 1, maxY: 1 };
    }

    const radians = (Math.abs(rotation) * Math.PI) / 180;
    const cosA = Math.cos(radians);
    const sinA = Math.sin(radians);
    const aspect = imageWidth / imageHeight;

    // Calculate the scale factor for the largest inscribed rectangle
    const scaleFromWidth = 1 / (cosA + sinA / aspect);
    const scaleFromHeight = 1 / (cosA + sinA * aspect);
    const cropScale = Math.min(scaleFromWidth, scaleFromHeight);

    // The valid region is centered and scaled
    const validWidth = cropScale;
    const validHeight = cropScale;
    const minX = (1 - validWidth) / 2;
    const minY = (1 - validHeight) / 2;

    return {
      minX,
      minY,
      maxX: minX + validWidth,
      maxY: minY + validHeight
    };
  });

  // Handle sizes
  const HANDLE_SIZE = 8;
  const EDGE_HANDLE_WIDTH = 12;

  // Calculate image display rect in screen coordinates (accounting for rotation)
  function getImageRect(): { left: number; top: number; width: number; height: number } {
    if (imageWidth === 0 || imageHeight === 0 || containerWidth === 0 || containerHeight === 0) {
      return { left: 0, top: 0, width: 0, height: 0 };
    }

    const cx = containerWidth / 2;
    const cy = containerHeight / 2;
    const imageAspect = imageWidth / imageHeight;
    const containerAspect = containerWidth / containerHeight;
    const padding = 0.95;

    // Calculate rotated bounding box (same as WebGL shader)
    const radians = (Math.abs(rotation) * Math.PI) / 180;
    const cosA = Math.cos(radians);
    const sinA = Math.sin(radians);

    // Rotated bounding box dimensions (normalized, height = 1)
    const boundingWidth = imageAspect * cosA + sinA;
    const boundingHeight = imageAspect * sinA + cosA;
    const rotatedAspect = boundingWidth / boundingHeight;

    // Calculate fit scale (same as WebGL shader)
    let fitScale: number;
    if (rotatedAspect > containerAspect) {
      // Width-constrained
      fitScale = (containerAspect / boundingWidth) * padding;
    } else {
      // Height-constrained
      fitScale = (1 / boundingHeight) * padding;
    }

    // The displayed image dimensions (the original image, not the bounding box)
    let displayWidth = imageAspect * fitScale * containerHeight;
    let displayHeight = fitScale * containerHeight;

    // Apply zoom
    displayWidth *= zoom;
    displayHeight *= zoom;

    // Calculate pan offset in pixels
    const panOffsetX = panX * displayWidth;
    const panOffsetY = panY * displayHeight;

    const left = cx - displayWidth / 2 + panOffsetX;
    const top = cy - displayHeight / 2 + panOffsetY;

    return { left, top, width: displayWidth, height: displayHeight };
  }

  // Convert normalized crop coords to screen coords
  function normalizedToScreen(normX: number, normY: number): { x: number; y: number } {
    const rect = getImageRect();
    return {
      x: rect.left + normX * rect.width,
      y: rect.top + normY * rect.height
    };
  }

  // Convert screen coords to normalized image coords
  function screenToNormalized(screenX: number, screenY: number): { x: number; y: number } {
    const rect = getImageRect();
    if (rect.width === 0 || rect.height === 0) {
      return { x: 0, y: 0 };
    }
    return {
      x: (screenX - rect.left) / rect.width,
      y: (screenY - rect.top) / rect.height
    };
  }

  // Get crop rect in screen coordinates
  let screenCrop = $derived.by(() => {
    const topLeft = normalizedToScreen(cropX, cropY);
    const bottomRight = normalizedToScreen(cropX + cropWidth, cropY + cropHeight);
    return {
      x: topLeft.x,
      y: topLeft.y,
      width: bottomRight.x - topLeft.x,
      height: bottomRight.y - topLeft.y
    };
  });

  // Common aspect ratios to detect
  const COMMON_RATIOS = [
    { ratio: 1, label: '1:1' },
    { ratio: 16 / 9, label: '16:9' },
    { ratio: 9 / 16, label: '9:16' },
    { ratio: 4 / 3, label: '4:3' },
    { ratio: 3 / 4, label: '3:4' },
    { ratio: 3 / 2, label: '3:2' },
    { ratio: 2 / 3, label: '2:3' },
    { ratio: 5 / 4, label: '5:4' },
    { ratio: 4 / 5, label: '4:5' }
  ];

  // Detect aspect ratio of crop area
  let aspectRatioLabel = $derived.by(() => {
    // Calculate actual pixel dimensions of crop
    const actualWidth = cropWidth * imageWidth;
    const actualHeight = cropHeight * imageHeight;

    if (actualWidth === 0 || actualHeight === 0) return '';

    const ratio = actualWidth / actualHeight;
    const tolerance = 0.02; // 2% tolerance for matching

    // Check against common ratios
    for (const { ratio: commonRatio, label } of COMMON_RATIOS) {
      if (Math.abs(ratio - commonRatio) / commonRatio < tolerance) {
        return label;
      }
    }

    // If no common ratio matches, show the simplified ratio
    // Find GCD-like simplification for display
    const gcd = (a: number, b: number): number => (b < 0.01 ? a : gcd(b, a % b));
    const divisor = gcd(actualWidth, actualHeight);
    const w = Math.round(actualWidth / divisor);
    const h = Math.round(actualHeight / divisor);

    // If numbers are too large, just show decimal ratio
    if (w > 20 || h > 20) {
      return ratio.toFixed(2);
    }

    return `${w}:${h}`;
  });

  // Grid lines (rule of thirds)
  let gridLines = $derived.by(() => {
    const lines: { x1: number; y1: number; x2: number; y2: number }[] = [];
    const sc = screenCrop;

    // Vertical lines at 1/3 and 2/3
    for (const ratio of [1 / 3, 2 / 3]) {
      const x = sc.x + sc.width * ratio;
      lines.push({ x1: x, y1: sc.y, x2: x, y2: sc.y + sc.height });
    }

    // Horizontal lines at 1/3 and 2/3
    for (const ratio of [1 / 3, 2 / 3]) {
      const y = sc.y + sc.height * ratio;
      lines.push({ x1: sc.x, y1: y, x2: sc.x + sc.width, y2: y });
    }

    return lines;
  });

  // Corner handles
  let cornerHandles = $derived.by(() => {
    const sc = screenCrop;
    const half = HANDLE_SIZE / 2;
    return [
      { mode: 'resize-nw' as DragMode, x: sc.x - half, y: sc.y - half, cursor: 'nwse-resize' },
      { mode: 'resize-ne' as DragMode, x: sc.x + sc.width - half, y: sc.y - half, cursor: 'nesw-resize' },
      { mode: 'resize-sw' as DragMode, x: sc.x - half, y: sc.y + sc.height - half, cursor: 'nesw-resize' },
      { mode: 'resize-se' as DragMode, x: sc.x + sc.width - half, y: sc.y + sc.height - half, cursor: 'nwse-resize' }
    ];
  });

  // Edge handles (invisible touch areas)
  let edgeHandles = $derived.by(() => {
    const sc = screenCrop;
    const hw = EDGE_HANDLE_WIDTH / 2;
    return [
      // Top edge
      {
        mode: 'resize-n' as DragMode,
        x: sc.x + HANDLE_SIZE,
        y: sc.y - hw,
        width: sc.width - HANDLE_SIZE * 2,
        height: EDGE_HANDLE_WIDTH,
        cursor: 'ns-resize'
      },
      // Bottom edge
      {
        mode: 'resize-s' as DragMode,
        x: sc.x + HANDLE_SIZE,
        y: sc.y + sc.height - hw,
        width: sc.width - HANDLE_SIZE * 2,
        height: EDGE_HANDLE_WIDTH,
        cursor: 'ns-resize'
      },
      // Left edge
      {
        mode: 'resize-w' as DragMode,
        x: sc.x - hw,
        y: sc.y + HANDLE_SIZE,
        width: EDGE_HANDLE_WIDTH,
        height: sc.height - HANDLE_SIZE * 2,
        cursor: 'ew-resize'
      },
      // Right edge
      {
        mode: 'resize-e' as DragMode,
        x: sc.x + sc.width - hw,
        y: sc.y + HANDLE_SIZE,
        width: EDGE_HANDLE_WIDTH,
        height: sc.height - HANDLE_SIZE * 2,
        cursor: 'ew-resize'
      }
    ];
  });

  // Constrain crop to valid bounds (accounting for rotation)
  function constrainCrop(
    x: number,
    y: number,
    w: number,
    h: number
  ): { x: number; y: number; width: number; height: number } {
    const bounds = validCropBounds;
    const maxWidth = bounds.maxX - bounds.minX;
    const maxHeight = bounds.maxY - bounds.minY;

    // Ensure minimum size
    w = Math.max(MIN_CROP_SIZE, w);
    h = Math.max(MIN_CROP_SIZE, h);

    // Clamp size to valid region
    w = Math.min(w, maxWidth);
    h = Math.min(h, maxHeight);

    // Clamp position to valid region
    x = Math.max(bounds.minX, Math.min(bounds.maxX - w, x));
    y = Math.max(bounds.minY, Math.min(bounds.maxY - h, y));

    // Re-check width/height after position adjustment
    w = Math.min(w, bounds.maxX - x);
    h = Math.min(h, bounds.maxY - y);

    return { x, y, width: w, height: h };
  }

  // Auto-constrain crop when bounds change (e.g., when rotation changes or on mount)
  $effect(() => {
    // Access validCropBounds to create dependency
    const bounds = validCropBounds;

    // Check if current crop is outside valid bounds
    const needsConstraint =
      cropX < bounds.minX ||
      cropY < bounds.minY ||
      cropX + cropWidth > bounds.maxX ||
      cropY + cropHeight > bounds.maxY;

    if (needsConstraint && onCropChange) {
      const constrained = constrainCrop(cropX, cropY, cropWidth, cropHeight);
      // Only update if actually changed to avoid infinite loops
      if (
        constrained.x !== cropX ||
        constrained.y !== cropY ||
        constrained.width !== cropWidth ||
        constrained.height !== cropHeight
      ) {
        onCropChange(constrained.x, constrained.y, constrained.width, constrained.height);
      }
    }
  });

  function startDrag(e: MouseEvent, mode: DragMode) {
    e.preventDefault();
    e.stopPropagation();
    dragMode = mode;
    startMouse = { x: e.clientX, y: e.clientY };
    startCrop = { x: cropX, y: cropY, width: cropWidth, height: cropHeight };
  }

  function handleMouseMove(e: MouseEvent) {
    if (dragMode === 'none' || !isEditing) return;

    const rect = getImageRect();
    if (rect.width === 0 || rect.height === 0) return;

    // Calculate delta in normalized coordinates
    const deltaX = (e.clientX - startMouse.x) / rect.width;
    const deltaY = (e.clientY - startMouse.y) / rect.height;

    let newX = startCrop.x;
    let newY = startCrop.y;
    let newW = startCrop.width;
    let newH = startCrop.height;

    switch (dragMode) {
      case 'move':
        newX = startCrop.x + deltaX;
        newY = startCrop.y + deltaY;
        break;

      case 'resize-nw':
        newX = startCrop.x + deltaX;
        newY = startCrop.y + deltaY;
        newW = startCrop.width - deltaX;
        newH = startCrop.height - deltaY;
        break;

      case 'resize-ne':
        newY = startCrop.y + deltaY;
        newW = startCrop.width + deltaX;
        newH = startCrop.height - deltaY;
        break;

      case 'resize-sw':
        newX = startCrop.x + deltaX;
        newW = startCrop.width - deltaX;
        newH = startCrop.height + deltaY;
        break;

      case 'resize-se':
        newW = startCrop.width + deltaX;
        newH = startCrop.height + deltaY;
        break;

      case 'resize-n':
        newY = startCrop.y + deltaY;
        newH = startCrop.height - deltaY;
        break;

      case 'resize-s':
        newH = startCrop.height + deltaY;
        break;

      case 'resize-w':
        newX = startCrop.x + deltaX;
        newW = startCrop.width - deltaX;
        break;

      case 'resize-e':
        newW = startCrop.width + deltaX;
        break;
    }

    const constrained = constrainCrop(newX, newY, newW, newH);
    onCropChange?.(constrained.x, constrained.y, constrained.width, constrained.height);
  }

  function handleMouseUp() {
    if (dragMode !== 'none') {
      dragMode = 'none';
      onCropCommit?.();
    }
  }
</script>

<svelte:window onmousemove={handleMouseMove} onmouseup={handleMouseUp} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<svg class="pointer-events-none absolute inset-0 h-full w-full">
  <!-- Dimmed overlay outside crop area using clip-path -->
  <defs>
    <mask id="crop-mask">
      <rect width="100%" height="100%" fill="white" />
      <rect
        x={screenCrop.x}
        y={screenCrop.y}
        width={screenCrop.width}
        height={screenCrop.height}
        fill="black"
      />
    </mask>
  </defs>

  <!-- Dimmed overlay -->
  <rect width="100%" height="100%" fill="rgba(0,0,0,0.6)" mask="url(#crop-mask)" />

  <!-- Crop boundary -->
  <rect
    x={screenCrop.x}
    y={screenCrop.y}
    width={screenCrop.width}
    height={screenCrop.height}
    fill="none"
    stroke="white"
    stroke-width="1.5"
  />

  <!-- Aspect ratio indicator -->
  {#if aspectRatioLabel}
    <foreignObject
      x={screenCrop.x + screenCrop.width / 2 - 30}
      y={screenCrop.y + 8}
      width="60"
      height="24"
      class="pointer-events-none"
    >
      <div class="flex h-full w-full items-center justify-center rounded-sm bg-black/70 px-2 py-0.5 text-xs font-medium text-white">
        {aspectRatioLabel}
      </div>
    </foreignObject>
  {/if}

  <!-- Grid lines (rule of thirds) -->
  {#each gridLines as line}
    <line
      x1={line.x1}
      y1={line.y1}
      x2={line.x2}
      y2={line.y2}
      stroke="white"
      stroke-opacity="0.4"
      stroke-width="1"
      stroke-dasharray="4 4"
    />
  {/each}

  {#if isEditing}
    <!-- Center drag area (for moving the entire crop) -->
    <rect
      x={screenCrop.x}
      y={screenCrop.y}
      width={screenCrop.width}
      height={screenCrop.height}
      fill="transparent"
      class="pointer-events-auto cursor-move"
      onmousedown={(e) => startDrag(e, 'move')}
    />

    <!-- Edge handles (invisible touch areas) -->
    {#each edgeHandles as edge}
      <rect
        x={edge.x}
        y={edge.y}
        width={edge.width}
        height={edge.height}
        fill="transparent"
        class="pointer-events-auto"
        style="cursor: {edge.cursor}"
        onmousedown={(e) => startDrag(e, edge.mode)}
      />
    {/each}

    <!-- Corner handles -->
    {#each cornerHandles as handle}
      <rect
        x={handle.x}
        y={handle.y}
        width={HANDLE_SIZE}
        height={HANDLE_SIZE}
        fill="white"
        stroke="#333"
        stroke-width="1"
        class="pointer-events-auto"
        style="cursor: {handle.cursor}"
        onmousedown={(e) => startDrag(e, handle.mode)}
      />
    {/each}
  {/if}
</svg>
