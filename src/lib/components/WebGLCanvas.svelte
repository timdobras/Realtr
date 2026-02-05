<script lang="ts">
  import { onMount, onDestroy } from 'svelte';

  // Props
  interface Props {
    imageBase64: string | null;
    brightness?: number;
    exposure?: number;
    contrast?: number;
    highlights?: number;
    shadows?: number;
    rotation?: number;
    quarterTurns?: number;
    showRotatedBackground?: boolean; // Show extended image beyond crop boundary (when rotating)
    // Crop parameters (normalized 0-1, only applied when cropEnabled is true)
    cropEnabled?: boolean;
    cropX?: number;
    cropY?: number;
    cropWidth?: number;
    cropHeight?: number;
    // Bindable outputs for crop overlay positioning
    zoom?: number;
    panX?: number;
    panY?: number;
    fitScalePercent?: number;
    canvasImageWidth?: number;
    canvasImageHeight?: number;
  }

  let {
    imageBase64,
    brightness = 0,
    exposure = 0,
    contrast = 0,
    highlights = 0,
    shadows = 0,
    rotation = 0,
    quarterTurns = 0,
    showRotatedBackground = false,
    cropEnabled = false,
    cropX = 0,
    cropY = 0,
    cropWidth = 1,
    cropHeight = 1,
    zoom = $bindable(1),
    panX = $bindable(0),
    panY = $bindable(0),
    fitScalePercent = $bindable(100),
    canvasImageWidth = $bindable(0),
    canvasImageHeight = $bindable(0)
  }: Props = $props();

  let canvas: HTMLCanvasElement;
  let gl: WebGLRenderingContext | null = null;
  let program: WebGLProgram | null = null;
  let texture: WebGLTexture | null = null;
  let imageLoaded = $state(false);

  // Internal tracking for panning
  let isPanning = $state(false);
  let lastMouseX = 0;
  let lastMouseY = 0;

  // Derived: actual zoom percentage for display
  let actualZoomPercent = $derived(Math.round(zoom * fitScalePercent));

  // Uniform locations
  let u_brightness: WebGLUniformLocation | null = null;
  let u_exposure: WebGLUniformLocation | null = null;
  let u_contrast: WebGLUniformLocation | null = null;
  let u_highlights: WebGLUniformLocation | null = null;
  let u_shadows: WebGLUniformLocation | null = null;
  let u_rotation: WebGLUniformLocation | null = null;
  let u_quarterTurns: WebGLUniformLocation | null = null;
  let u_imageAspect: WebGLUniformLocation | null = null;
  let u_canvasAspect: WebGLUniformLocation | null = null;
  let u_zoom: WebGLUniformLocation | null = null;
  let u_panX: WebGLUniformLocation | null = null;
  let u_panY: WebGLUniformLocation | null = null;
  let u_canvasHeight: WebGLUniformLocation | null = null;
  let u_showBackground: WebGLUniformLocation | null = null;
  let u_cropEnabled: WebGLUniformLocation | null = null;
  let u_cropX: WebGLUniformLocation | null = null;
  let u_cropY: WebGLUniformLocation | null = null;
  let u_cropWidth: WebGLUniformLocation | null = null;
  let u_cropHeight: WebGLUniformLocation | null = null;

  // Track canvas aspect ratio for shader
  let canvasAspectRatio = $state(1);

  // Vertex shader - handles positioning and texture coordinates
  const vertexShaderSource = `
    attribute vec2 a_position;
    attribute vec2 a_texCoord;
    varying vec2 v_texCoord;

    void main() {
      gl_Position = vec4(a_position, 0.0, 1.0);
      v_texCoord = a_texCoord;
    }
  `;

  // Fragment shader - handles all adjustments
  // Canvas fills the entire container; image is displayed centered within it
  const fragmentShaderSource = `
    precision mediump float;

    uniform sampler2D u_image;
    uniform float u_brightness;    // -1.0 to 1.0
    uniform float u_exposure;      // -2.0 to 2.0 (f-stops)
    uniform float u_contrast;      // 0.0 to 2.0 (1.0 = no change)
    uniform float u_highlights;    // -1.0 to 1.0
    uniform float u_shadows;       // -1.0 to 1.0
    uniform float u_rotation;      // rotation in radians (for fine rotation)
    uniform float u_quarterTurns;  // 0.0, 1.0, 2.0, or 3.0
    uniform float u_imageAspect;   // image width / height
    uniform float u_canvasAspect;  // canvas width / height
    uniform float u_zoom;          // zoom level (1.0 = fit, >1 = zoomed in)
    uniform float u_panX;          // pan offset X (normalized)
    uniform float u_panY;          // pan offset Y (normalized)
    uniform float u_canvasHeight;  // canvas height in pixels (for border thickness)
    uniform float u_showBackground;  // 1.0 = show background/dimmed areas, 0.0 = only show crop area
    uniform float u_cropEnabled;   // 1.0 = show only cropped region, 0.0 = show full image
    uniform float u_cropX;         // crop start X (0-1)
    uniform float u_cropY;         // crop start Y (0-1)
    uniform float u_cropWidth;     // crop width (0-1)
    uniform float u_cropHeight;    // crop height (0-1)

    varying vec2 v_texCoord;

    void main() {
      // Start with canvas coordinates centered at origin
      vec2 canvasCoord = v_texCoord - 0.5;

      // Apply zoom and pan
      canvasCoord = canvasCoord / u_zoom - vec2(u_panX, u_panY);

      // Convert to visual space (canvas aspect ratio)
      float x_vis = canvasCoord.x * u_canvasAspect;
      float y_vis = canvasCoord.y;

      // Calculate the effective image aspect ratio (accounting for quarter turns)
      float effectiveAspect = u_imageAspect;
      if (u_quarterTurns > 0.5 && u_quarterTurns < 1.5) {
        effectiveAspect = 1.0 / u_imageAspect;
      } else if (u_quarterTurns > 2.5) {
        effectiveAspect = 1.0 / u_imageAspect;
      }

      // When crop is enabled, use the cropped region's aspect ratio instead
      float displayAspect = effectiveAspect;
      if (u_cropEnabled > 0.5) {
        // The crop region's aspect ratio in image space
        float cropAspect = (u_cropWidth * u_imageAspect) / u_cropHeight;
        // Account for quarter turns
        if (u_quarterTurns > 0.5 && u_quarterTurns < 1.5) {
          cropAspect = u_cropHeight / (u_cropWidth * u_imageAspect);
        } else if (u_quarterTurns > 2.5) {
          cropAspect = u_cropHeight / (u_cropWidth * u_imageAspect);
        }
        displayAspect = cropAspect;
      }

      // Calculate the bounding box of the rotated image
      float absAngle = abs(u_rotation);
      float cosA = cos(absAngle);
      float sinA = sin(absAngle);

      // Rotated bounding box dimensions (normalized, height = 1)
      float boundingWidth = displayAspect * cosA + sinA;
      float boundingHeight = displayAspect * sinA + cosA;
      float rotatedAspect = boundingWidth / boundingHeight;

      // Scale to fit the rotated image within the canvas (with some padding)
      float padding = 0.95; // 5% padding
      float fitScale;
      if (rotatedAspect > u_canvasAspect) {
        // Width-constrained
        fitScale = u_canvasAspect / boundingWidth * padding;
      } else {
        // Height-constrained
        fitScale = 1.0 / boundingHeight * padding;
      }

      // Scale canvas visual coords to image space
      float x_scaled = x_vis / fitScale;
      float y_scaled = y_vis / fitScale;

      // Apply INVERSE rotation for fine rotation (to find source pixel)
      float c = cos(u_rotation);
      float s = sin(u_rotation);
      float x_rot = x_scaled * c + y_scaled * s;
      float y_rot = -x_scaled * s + y_scaled * c;

      // Convert to texture coordinates
      vec2 texCoord = vec2(x_rot / displayAspect + 0.5, y_rot + 0.5);

      // When crop is enabled, map the 0-1 texture coords to the crop region
      if (u_cropEnabled > 0.5) {
        texCoord = vec2(
          u_cropX + texCoord.x * u_cropWidth,
          u_cropY + texCoord.y * u_cropHeight
        );
      }

      // Apply quarter turn rotation to texture coordinates
      if (u_quarterTurns > 0.5 && u_quarterTurns < 1.5) {
        // 90 degrees clockwise
        texCoord = vec2(1.0 - texCoord.y, texCoord.x);
      } else if (u_quarterTurns > 1.5 && u_quarterTurns < 2.5) {
        // 180 degrees
        texCoord = vec2(1.0 - texCoord.x, 1.0 - texCoord.y);
      } else if (u_quarterTurns > 2.5) {
        // 270 degrees (or -90)
        texCoord = vec2(texCoord.y, 1.0 - texCoord.x);
      }

      // Calculate crop boundary (the area that will be saved after auto-crop)
      // The crop boundary should be FIXED on screen (not rotating with the image)
      // Always show the border to indicate the output area

      // Auto-crop scale factor - largest rectangle that fits inside rotated image
      // When rotation is 0, cropScale = 1.0 (no cropping needed)
      float scaleFromWidth = 1.0 / (cosA + sinA / displayAspect);
      float scaleFromHeight = 1.0 / (cosA + sinA * displayAspect);
      float cropScale = min(scaleFromWidth, scaleFromHeight);

      // Crop boundary dimensions in VISUAL space (before rotation is applied)
      // This makes the border fixed on screen while the image rotates
      float cropHalfWidth = displayAspect * cropScale * 0.5;
      float cropHalfHeight = cropScale * 0.5;

      // Check if point is inside crop area using x_scaled, y_scaled (BEFORE inverse rotation)
      // This keeps the crop boundary fixed on screen
      bool insideCropArea = abs(x_scaled) <= cropHalfWidth && abs(y_scaled) <= cropHalfHeight;

      // Check if on the crop border - also in visual space (before rotation)
      // Calculate thin border thickness in scaled coordinate space
      // u_canvasHeight is in pixels, fitScale is the scaling factor
      float pixelsToScaled = 1.0 / (u_canvasHeight * fitScale * 0.95);
      float borderThickness = max(0.5 * pixelsToScaled, 0.0003);
      float distToBorderX = abs(abs(x_scaled) - cropHalfWidth);
      float distToBorderY = abs(abs(y_scaled) - cropHalfHeight);
      bool nearBorderX = distToBorderX < borderThickness && abs(y_scaled) <= cropHalfHeight + borderThickness;
      bool nearBorderY = distToBorderY < borderThickness && abs(x_scaled) <= cropHalfWidth + borderThickness;
      bool onCropBorder = nearBorderX || nearBorderY;

      // When not rotating, hide everything outside crop area
      if (u_showBackground < 0.5 && !insideCropArea) {
        gl_FragColor = vec4(0.1, 0.1, 0.1, 1.0);  // Dark background
        return;
      }

      // Draw crop border only when showing background (actively rotating)
      if (u_showBackground > 0.5 && onCropBorder) {
        gl_FragColor = vec4(1.0, 1.0, 1.0, 1.0);  // White border
        return;
      }

      // Check if on grid line and determine dash/gap state
      bool onGridLine = false;
      bool onDashPart = false;  // true = dash (stronger), false = gap (lighter)
      if (u_showBackground > 0.5 && insideCropArea) {
        // Convert to 0-1 coordinates within the crop area
        float gridX = (x_scaled + cropHalfWidth) / (2.0 * cropHalfWidth);
        float gridY = (y_scaled + cropHalfHeight) / (2.0 * cropHalfHeight);

        // Dash pattern parameters (in normalized crop coordinates)
        float dashLength = 0.02;  // Length of each dash
        float gapLength = 0.015;   // Length of each gap
        float dashCycle = dashLength + gapLength;

        // Check if on any of the 8 grid lines (at 1/9, 2/9, ... 8/9)
        for (int i = 1; i <= 8; i++) {
          float linePos = float(i) / 9.0;
          // Vertical line - check X position, use Y for dash pattern
          if (abs(gridX - linePos) < borderThickness / (2.0 * cropHalfWidth)) {
            onGridLine = true;
            float dashPos = mod(gridY, dashCycle);
            if (dashPos < dashLength) {
              onDashPart = true;
            }
          }
          // Horizontal line - check Y position, use X for dash pattern
          if (abs(gridY - linePos) < borderThickness / (2.0 * cropHalfHeight)) {
            onGridLine = true;
            float dashPos = mod(gridX, dashCycle);
            if (dashPos < dashLength) {
              onDashPart = true;
            }
          }
        }
      }

      // Check if coordinate is outside texture bounds (dark background)
      bool outsideTexture = texCoord.x < 0.0 || texCoord.x > 1.0 || texCoord.y < 0.0 || texCoord.y > 1.0;

      if (outsideTexture) {
        gl_FragColor = vec4(0.1, 0.1, 0.1, 1.0);  // Dark background
        return;
      }

      vec4 color = texture2D(u_image, texCoord);
      vec3 rgb = color.rgb;

      // 1. Exposure (multiplicative, f-stops)
      rgb *= pow(2.0, u_exposure);

      // 2. Brightness (additive)
      rgb += u_brightness;

      // 3. Contrast (pivot around 0.5)
      rgb = (rgb - 0.5) * u_contrast + 0.5;

      // 4. Highlights/Shadows (luminance-based)
      float luminance = dot(rgb, vec3(0.2126, 0.7152, 0.0722));
      float highlightMask = smoothstep(0.3, 0.7, luminance);
      float shadowMask = 1.0 - highlightMask;
      float adjustment = u_highlights * highlightMask + u_shadows * shadowMask;
      rgb += adjustment * 0.5;

      // Clamp to valid range
      rgb = clamp(rgb, 0.0, 1.0);

      // When rotating, dim areas outside the crop boundary
      if (u_showBackground > 0.5 && !insideCropArea) {
        rgb *= 0.35;
      }

      // Blend grid lines over the image (alternating opacity for dash effect)
      if (onGridLine) {
        float gridOpacity = onDashPart ? 0.5 : 0.15;  // 50% for dash, 15% for gap
        rgb = mix(rgb, vec3(1.0, 1.0, 1.0), gridOpacity);
      }

      gl_FragColor = vec4(rgb, 1.0);
    }
  `;

  function createShader(
    gl: WebGLRenderingContext,
    type: number,
    source: string
  ): WebGLShader | null {
    const shader = gl.createShader(type);
    if (!shader) return null;

    gl.shaderSource(shader, source);
    gl.compileShader(shader);

    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      console.error('Shader compile error:', gl.getShaderInfoLog(shader));
      gl.deleteShader(shader);
      return null;
    }

    return shader;
  }

  function createProgram(
    gl: WebGLRenderingContext,
    vertexShader: WebGLShader,
    fragmentShader: WebGLShader
  ): WebGLProgram | null {
    const prog = gl.createProgram();
    if (!prog) return null;

    gl.attachShader(prog, vertexShader);
    gl.attachShader(prog, fragmentShader);
    gl.linkProgram(prog);

    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      console.error('Program link error:', gl.getProgramInfoLog(prog));
      gl.deleteProgram(prog);
      return null;
    }

    return prog;
  }

  function initWebGL() {
    if (!canvas) return;

    gl = canvas.getContext('webgl', { preserveDrawingBuffer: true });
    if (!gl) {
      console.error('WebGL not supported');
      return;
    }

    // Create shaders
    const vertexShader = createShader(gl, gl.VERTEX_SHADER, vertexShaderSource);
    const fragmentShader = createShader(gl, gl.FRAGMENT_SHADER, fragmentShaderSource);
    if (!vertexShader || !fragmentShader) return;

    // Create program
    program = createProgram(gl, vertexShader, fragmentShader);
    if (!program) return;

    gl.useProgram(program);

    // Set up geometry (two triangles forming a rectangle)
    const positions = new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]);

    const texCoords = new Float32Array([0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 0]);

    // Position buffer
    const positionBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, positions, gl.STATIC_DRAW);

    const positionLocation = gl.getAttribLocation(program, 'a_position');
    gl.enableVertexAttribArray(positionLocation);
    gl.vertexAttribPointer(positionLocation, 2, gl.FLOAT, false, 0, 0);

    // Texture coordinate buffer
    const texCoordBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, texCoordBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, texCoords, gl.STATIC_DRAW);

    const texCoordLocation = gl.getAttribLocation(program, 'a_texCoord');
    gl.enableVertexAttribArray(texCoordLocation);
    gl.vertexAttribPointer(texCoordLocation, 2, gl.FLOAT, false, 0, 0);

    // Get uniform locations
    u_brightness = gl.getUniformLocation(program, 'u_brightness');
    u_exposure = gl.getUniformLocation(program, 'u_exposure');
    u_contrast = gl.getUniformLocation(program, 'u_contrast');
    u_highlights = gl.getUniformLocation(program, 'u_highlights');
    u_shadows = gl.getUniformLocation(program, 'u_shadows');
    u_rotation = gl.getUniformLocation(program, 'u_rotation');
    u_quarterTurns = gl.getUniformLocation(program, 'u_quarterTurns');
    u_imageAspect = gl.getUniformLocation(program, 'u_imageAspect');
    u_canvasAspect = gl.getUniformLocation(program, 'u_canvasAspect');
    u_zoom = gl.getUniformLocation(program, 'u_zoom');
    u_panX = gl.getUniformLocation(program, 'u_panX');
    u_panY = gl.getUniformLocation(program, 'u_panY');
    u_canvasHeight = gl.getUniformLocation(program, 'u_canvasHeight');
    u_showBackground = gl.getUniformLocation(program, 'u_showBackground');
    u_cropEnabled = gl.getUniformLocation(program, 'u_cropEnabled');
    u_cropX = gl.getUniformLocation(program, 'u_cropX');
    u_cropY = gl.getUniformLocation(program, 'u_cropY');
    u_cropWidth = gl.getUniformLocation(program, 'u_cropWidth');
    u_cropHeight = gl.getUniformLocation(program, 'u_cropHeight');

    // Create texture
    texture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, texture);

    // Set texture parameters
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  }

  function resizeCanvas() {
    if (!gl || !canvas) return;

    const container = canvas.parentElement;
    if (!container) return;

    // Canvas fills the entire container
    const containerWidth = container.clientWidth;
    const containerHeight = container.clientHeight;

    if (containerWidth === 0 || containerHeight === 0) return;

    canvas.width = containerWidth;
    canvas.height = containerHeight;
    canvas.style.width = `${containerWidth}px`;
    canvas.style.height = `${containerHeight}px`;
    gl.viewport(0, 0, containerWidth, containerHeight);

    // Track canvas aspect ratio for shader
    canvasAspectRatio = containerWidth / containerHeight;

    // Calculate fitScalePercent: what percentage is the image at when it fits in view
    // This accounts for the 0.95 padding in the shader
    if (canvasImageWidth > 0 && canvasImageHeight > 0) {
      const imageAspect = canvasImageWidth / canvasImageHeight;
      const padding = 0.95;
      let displayedWidth: number;
      let displayedHeight: number;

      if (imageAspect > canvasAspectRatio) {
        // Width-constrained
        displayedWidth = containerWidth * padding;
        displayedHeight = displayedWidth / imageAspect;
      } else {
        // Height-constrained
        displayedHeight = containerHeight * padding;
        displayedWidth = displayedHeight * imageAspect;
      }

      // fitScalePercent = (displayed pixels / actual image pixels) * 100
      fitScalePercent = (displayedWidth / canvasImageWidth) * 100;
    }
  }

  async function loadImageToTexture(base64: string) {
    if (!gl || !texture) return;

    return new Promise<void>((resolve) => {
      const img = new Image();
      img.onload = () => {
        if (!gl || !texture) {
          resolve();
          return;
        }

        // Store dimensions
        canvasImageWidth = img.width;
        canvasImageHeight = img.height;

        // Resize canvas to match rotated image aspect ratio
        resizeCanvas();

        // Upload image to texture
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);

        imageLoaded = true;
        render();
        resolve();
      };
      img.onerror = () => {
        console.error('Failed to load image');
        resolve();
      };
      img.src = `data:image/jpeg;base64,${base64}`;
    });
  }

  function render() {
    if (!gl || !program || !imageLoaded) return;

    gl.useProgram(program);

    // Original image aspect ratio (shader handles quarter turns internally)
    const imageAspect = canvasImageWidth / canvasImageHeight;

    // Update uniforms - convert from -100..100 to shader ranges
    // Ranges calibrated to match Windows 11 Photo Editor behavior
    gl.uniform1f(u_brightness, brightness / 350); // -0.29 to 0.29 (softer)
    gl.uniform1f(u_exposure, exposure / 130); // -0.77 to 0.77 f-stops (~0.59x to 1.7x)
    gl.uniform1f(u_contrast, (contrast + 170) / 170); // 0.41 to 1.59 (softer)
    gl.uniform1f(u_highlights, highlights / 180); // -0.56 to 0.56 (softer)
    gl.uniform1f(u_shadows, shadows / 180); // -0.56 to 0.56 (softer)
    gl.uniform1f(u_rotation, (rotation * Math.PI) / 180); // degrees to radians
    gl.uniform1f(u_quarterTurns, quarterTurns); // 0.0, 1.0, 2.0, or 3.0
    gl.uniform1f(u_imageAspect, imageAspect); // original image aspect ratio
    gl.uniform1f(u_canvasAspect, canvasAspectRatio); // canvas aspect ratio
    gl.uniform1f(u_zoom, zoom); // zoom level
    gl.uniform1f(u_panX, panX); // pan offset X
    gl.uniform1f(u_panY, panY); // pan offset Y
    gl.uniform1f(u_canvasHeight, canvas.height || 600); // canvas height for border thickness
    gl.uniform1f(u_showBackground, showRotatedBackground ? 1.0 : 0.0); // show rotated background
    gl.uniform1f(u_cropEnabled, cropEnabled ? 1.0 : 0.0); // enable crop preview
    gl.uniform1f(u_cropX, cropX); // crop start X
    gl.uniform1f(u_cropY, cropY); // crop start Y
    gl.uniform1f(u_cropWidth, cropWidth); // crop width
    gl.uniform1f(u_cropHeight, cropHeight); // crop height

    // Clear and draw
    gl.clearColor(0.1, 0.1, 0.1, 1.0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }

  // Reactive effect: resize canvas and re-render when rotation changes
  $effect(() => {
    // Access rotation props to create dependency for resizing
    const _ = [rotation, quarterTurns];
    if (imageLoaded) {
      resizeCanvas();
      render();
    }
  });

  // Reactive effect: re-render whenever adjustment props change (no resize needed)
  $effect(() => {
    // Access adjustment props to create dependency
    const _ = [brightness, exposure, contrast, highlights, shadows, showRotatedBackground, cropEnabled, cropX, cropY, cropWidth, cropHeight];
    render();
  });

  // Reactive effect: re-render when zoom or pan changes
  $effect(() => {
    const _ = [zoom, panX, panY];
    render();
  });

  // Zoom functions
  // Calculate min zoom to not go smaller than fit
  const minZoom = 0.5;
  // Max zoom = 400% actual size
  const maxZoomPercent = 400;

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    const maxZoom = maxZoomPercent / fitScalePercent;
    const newZoom = Math.max(minZoom, Math.min(maxZoom, zoom * delta));
    zoom = newZoom;

    // Re-clamp pan values
    const clamped = clampPan(panX, panY, newZoom);
    panX = clamped.x;
    panY = clamped.y;
  }

  function zoomIn() {
    const maxZoom = maxZoomPercent / fitScalePercent;
    const newZoom = Math.min(maxZoom, zoom * 1.25);
    zoom = newZoom;
    const clamped = clampPan(panX, panY, newZoom);
    panX = clamped.x;
    panY = clamped.y;
  }

  function zoomOut() {
    const newZoom = Math.max(minZoom, zoom / 1.25);
    zoom = newZoom;
    const clamped = clampPan(panX, panY, newZoom);
    panX = clamped.x;
    panY = clamped.y;
  }

  function zoomToFit() {
    zoom = 1;
    panX = 0;
    panY = 0;
  }

  function zoomTo100() {
    // 100% actual size = internal zoom of (100 / fitScalePercent)
    const targetZoom = 100 / fitScalePercent;
    const maxZoom = maxZoomPercent / fitScalePercent;
    zoom = Math.min(maxZoom, Math.max(minZoom, targetZoom));
    // Keep pan clamped
    const clamped = clampPan(panX, panY, zoom);
    panX = clamped.x;
    panY = clamped.y;
  }

  // Clamp pan values so the image stays at least partially visible
  function clampPan(x: number, y: number, z: number): { x: number; y: number } {
    // Calculate max pan based on zoom level
    // At zoom 1, no pan allowed. At higher zoom, allow more pan but keep image visible.
    // maxPan ensures at least ~30% of the image remains visible
    const maxPan = Math.max(0, ((z - 1) / (2 * z)) * 0.9);
    return {
      x: Math.max(-maxPan, Math.min(maxPan, x)),
      y: Math.max(-maxPan, Math.min(maxPan, y))
    };
  }

  // Pan functions
  function handleMouseDown(e: MouseEvent) {
    if (zoom > 1) {
      isPanning = true;
      lastMouseX = e.clientX;
      lastMouseY = e.clientY;
      canvas.style.cursor = 'grabbing';
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (isPanning && zoom > 1) {
      const dx = (e.clientX - lastMouseX) / canvas.width;
      const dy = (e.clientY - lastMouseY) / canvas.height;
      const newPan = clampPan(panX + dx / zoom, panY + dy / zoom, zoom);
      panX = newPan.x;
      panY = newPan.y;
      lastMouseX = e.clientX;
      lastMouseY = e.clientY;
    }
  }

  function handleMouseUp() {
    isPanning = false;
    updateCursor();
  }

  function handleMouseLeave() {
    isPanning = false;
    updateCursor();
  }

  function updateCursor() {
    if (canvas) {
      // Allow panning when zoomed in beyond fit
      canvas.style.cursor = zoom > 1 ? 'grab' : 'default';
    }
  }

  // Update cursor based on zoom level
  $effect(() => {
    // Depend on zoom to trigger effect
    const _ = zoom;
    updateCursor();
  });

  // Effect to load new image when base64 changes
  $effect(() => {
    if (imageBase64 && gl) {
      loadImageToTexture(imageBase64);
    }
  });

  let resizeObserver: ResizeObserver | null = null;

  onMount(() => {
    initWebGL();
    if (imageBase64) {
      loadImageToTexture(imageBase64);
    }

    // Set up resize observer to handle container size changes
    if (canvas && canvas.parentElement) {
      resizeObserver = new ResizeObserver(() => {
        resizeCanvas();
        if (imageLoaded) {
          render();
        }
      });
      resizeObserver.observe(canvas.parentElement);
    }
  });

  onDestroy(() => {
    if (resizeObserver) {
      resizeObserver.disconnect();
    }
    if (gl && texture) {
      gl.deleteTexture(texture);
    }
    if (gl && program) {
      gl.deleteProgram(program);
    }
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="relative h-full w-full">
  <canvas
    bind:this={canvas}
    class="absolute inset-0 h-full w-full"
    onwheel={handleWheel}
    onmousedown={handleMouseDown}
    onmousemove={handleMouseMove}
    onmouseup={handleMouseUp}
    onmouseleave={handleMouseLeave}
  ></canvas>

  <!-- Zoom controls -->
  <div class="absolute left-4 top-4 flex items-center gap-1 rounded-sm bg-black/70 p-1">
    <button onclick={zoomOut} class="rounded-sm p-1.5 text-white hover:bg-white/20" title="Zoom out">
      <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 12H4" />
      </svg>
    </button>
    <button
      onclick={zoomToFit}
      class="rounded-sm px-2 py-1 text-xs text-white hover:bg-white/20"
      title="Fit to view"
    >
      Fit
    </button>
    <span class="min-w-10 text-center text-xs text-white">{actualZoomPercent}%</span>
    <button
      onclick={zoomTo100}
      class="rounded-sm px-2 py-1 text-xs text-white hover:bg-white/20"
      title="Actual size (100%)"
    >
      100%
    </button>
    <button onclick={zoomIn} class="rounded-sm p-1.5 text-white hover:bg-white/20" title="Zoom in">
      <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
      </svg>
    </button>
  </div>

  {#if !imageLoaded && imageBase64}
    <div class="bg-background-900 absolute inset-0 flex items-center justify-center">
      <div class="text-foreground-500 flex flex-col items-center gap-2">
        <svg class="h-8 w-8 animate-spin" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
          ></circle>
          <path
            class="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
          ></path>
        </svg>
        <span class="text-sm">Loading...</span>
      </div>
    </div>
  {/if}
</div>
