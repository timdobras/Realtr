# Auto-Straighten Algorithm Documentation

## Overview

The auto-straighten feature automatically detects and corrects slight tilts in real estate photos. It uses an **ensemble approach** combining 4 different detection methods, where agreement between methods increases confidence.

## File Location

- **Backend (Rust)**: `src-tauri/src/image_editor.rs`
- **Frontend (Svelte)**: `src/routes/(editor)/editor/+page.svelte` (function `applyAutoStraighten`)

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        detect_straighten_angle()                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌─────────┐│
│   │   Rotation   │  │  Histogram   │  │   Region     │  │  Long   ││
│   │    Search    │  │   Voting     │  │  Analysis    │  │  Line   ││
│   │   Method     │  │   Method     │  │   Method     │  │ Method  ││
│   └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └────┬────┘│
│          │                 │                 │                │     │
│          └─────────────────┴─────────────────┴────────────────┘     │
│                                    │                                 │
│                       ┌────────────▼────────────┐                   │
│                       │ combine_ensemble_results │                   │
│                       └────────────┬────────────┘                   │
│                                    │                                 │
│                       ┌────────────▼────────────┐                   │
│                       │apply_rotation_safety_limits│                │
│                       └────────────┬────────────┘                   │
│                                    │                                 │
│                             { angle, confidence }                    │
└─────────────────────────────────────────────────────────────────────┘
```

## Core Concepts

### Sobel Gradient Detection

The algorithm detects edges using the **Sobel operator**, a 3x3 kernel that computes gradients:

```rust
// Sobel kernels (compute_sobel_gradient_fast)
// Gx kernel: detects vertical edges (gradient in X direction)
// [-1  0 +1]
// [-2  0 +2]
// [-1  0 +1]

// Gy kernel: detects horizontal edges (gradient in Y direction)
// [-1 -2 -1]
// [ 0  0  0]
// [+1 +2 +1]

let gx = -p(-1,-1) + p(1,-1) - 2*p(-1,0) + 2*p(1,0) - p(-1,1) + p(1,1);
let gy = -p(-1,-1) - 2*p(0,-1) - p(1,-1) + p(-1,1) + 2*p(0,1) + p(1,1);
let magnitude = sqrt(gx² + gy²);
let angle = atan2(gy, gx);  // Gradient direction in degrees
```

**Key insight**: The gradient angle is **perpendicular** to the edge direction.
- Gradient at 0° or ±180° = **Vertical edge** (wall)
- Gradient at ±90° = **Horizontal edge** (floor/ceiling)

### Edge Threshold

Only pixels with gradient magnitude ≥ **35.0** are considered edges. This filters out noise and weak gradients.

### Position-Based Weighting

Edges at different positions have different reliability:

| Position | Weight | Reasoning |
|----------|--------|-----------|
| Left/right border (outer 15%) | 2.5x | Walls are typically at edges |
| Top/bottom border | 1.5x | Ceiling/floor lines |
| Center | 1.0x | Furniture, decorations - less reliable |

---

## The 4 Detection Methods

### Method 1: Rotation Search

**Function**: `search_best_rotation()`

The most sophisticated method. Tests 121 candidate rotations from -6° to +6° in 0.1° steps, finding which rotation maximizes vertical/horizontal edge alignment.

**Process**:
1. For each candidate angle θ, calculate alignment score:
   ```
   score = Σ (weight × alignment_quality) for all edges within 1° of V/H
   ```
2. Find θ with maximum score
3. Refine with finer search (±0.15° in 0.02° steps)
4. Analyze the score distribution to assess confidence

**Confidence Metrics**:
- `improvement_ratio` - How much better is best angle vs 0°?
- `peak_ratio` - How sharp is the peak? (best/average)
- `local_maxima` - Are there multiple peaks? (indicates ambiguity)
- `score_span` - Overall variation in scores

**Base Confidence Table**:
| Condition | Base Confidence |
|-----------|----------------|
| improvement > 15%, peak_ratio > 1.35, single peak | 50% |
| improvement > 12%, peak_ratio > 1.25, ≤2 peaks | 40% |
| improvement > 8%, peak_ratio > 1.15, ≤3 peaks | 32% |
| improvement > 4%, peak_ratio > 1.08 | 22% |
| improvement > 2% OR score_span > 0.1 | 15% |
| else | 8% |

**Penalties Applied**:
- Rotation > 4°: ×0.6
- Rotation > 2.5°: ×0.75
- Rotation > 1°: ×0.9
- Multiple peaks (≥3): ×0.65
- Multiple peaks (2): ×0.80
- Weak peak (ratio < 1.1): ×0.7
- Low improvement (<5%): ×0.75

**Max confidence**: 55%

---

### Method 2: Histogram Voting

**Function**: `detect_angle_histogram()`

Bins edge angles into a histogram and finds the dominant angle.

**Process**:
1. Create 100 bins covering -10° to +10° (0.2° per bin)
2. Divide image into 8×8 grid cells
3. For each edge pixel:
   - Determine which cell it belongs to
   - Classify as vertical or horizontal edge
   - Add vote to appropriate angle bin
4. Apply per-cell vote caps (prevents single area dominating)
5. Find peak angle from aggregated histogram

**Grid Cell Vote Caps**:
| Cell Type | Max Contribution |
|-----------|-----------------|
| Border cells | 8% of total |
| Middle zone | 4% of total |
| Center cells | 2% of total |

**Border Weight (within histogram)**:
- Left/right edges: 10x weight
- Top/bottom edges: 3x weight
- Deep center: 0.3x weight (penalized)

**Confidence Calculation**:
```rust
// With border support (≥15% of peak votes from borders):
confidence = (concentration × 0.4 + vote_factor × 0.25).clamp(0, 0.55)

// Without border support:
confidence = (concentration × 0.15 + vote_factor × 0.08).clamp(0, 0.20)
```

**Max confidence**: 55% (with border support), 20% (without)

---

### Method 3: Region Analysis

**Function**: `detect_angle_from_border_strips()`

Analyzes left, right, and center regions separately, then combines based on agreement.

**Regions**:
- **Left strip**: 12% of image width from left edge
- **Right strip**: 12% of image width from right edge
- **Center**: Middle third of image

**Per-Region Analysis** (`analyze_region()`):
1. Collect vertical edges (gradient near 0° or ±180°)
2. Filter edges with magnitude ≥ 45
3. Calculate weighted median angle
4. Compute agreement ratio (edges within 1.5° of median)
5. Calculate variance and standard deviation

**Region Confidence**:
```rust
confidence = (agreement_ratio × 0.35 + vote_factor × 0.15 + consistency × 0.15).clamp(0, 0.50)
```

**Combination Rules** (`combine_region_results()`):

| Agreement Pattern | Result | Max Confidence |
|------------------|--------|----------------|
| All 3 agree | Weighted average of all | 55% |
| Left + Right agree (center disagrees) | Average of borders | 50% |
| Left + Center agree | Average of pair | 40% |
| Right + Center agree | Average of pair | 40% |
| No agreement | Best region, heavy penalty | 25% |

---

### Method 4: Long Line Detection

**Function**: `detect_angle_from_long_lines()`

Traces connected edge segments to find long lines spanning significant portions of the image.

**Process**:
1. Scan vertically at each X position
2. Trace runs of consistent gradient direction
3. Keep lines ≥ 20% of image dimension
4. Filter for near-vertical edges only (gradient < 20° from vertical)
5. Sort by length, use top 10 longest
6. Calculate length-weighted average angle

**Confidence**:
```rust
count_factor = (num_lines / 5).min(1.0)
consistency_factor = (1 - std_dev / 3).clamp(0, 1)
length_factor = (longest_line / image_dimension).min(1.0)

confidence = (count × 0.2 + consistency × 0.25 + length × 0.2).clamp(0, 0.50)
```

**Max confidence**: 50%

---

## Ensemble Combination

**Function**: `combine_ensemble_results()`

Combines the 4 methods using agreement-based weighting.

**Algorithm**:
1. Filter methods with confidence > 15%
2. For each method's angle, count how many other methods agree within 1.5°
3. Find consensus angle with most agreeing methods
4. Calculate weighted average of agreeing methods
5. Apply agreement bonus

**Agreement Bonuses**:
| Methods Agreeing | Bonus |
|-----------------|-------|
| 4 | +18% |
| 3 | +10% |
| 2 | +3% |
| 1 | +0% |

**Final Confidence**:
```rust
base_conf = (max_conf + avg_conf) / 2.0  // Average of max and mean
confidence = (base_conf + agreement_bonus).min(0.75)
```

**Max ensemble confidence**: 75%

---

## Safety Limits

**Function**: `apply_rotation_safety_limits()`

Prevents bad large rotations by capping angle based on confidence.

**Rotation Caps**:
| Confidence | Max Allowed Rotation |
|------------|---------------------|
| ≥ 70% | 6° |
| ≥ 55% | 4° |
| ≥ 40% | 2.5° |
| ≥ 25% | 1.5° |
| < 25% | 0.8° |

**Additional Penalties**:
- Angle capped → confidence ×0.8
- Angle > 3° → confidence ×0.85

---

## Frontend Integration

The frontend (`editor/+page.svelte`) calls the backend and displays confidence-based feedback:

```typescript
const result = await invoke<{ angle: number; confidence: number }>('editor_auto_straighten');

if (result.confidence >= 0.6) {
  showSuccess(`Rotated ${angle}° (${confidence}% confident)`);
} else if (result.confidence >= 0.3) {
  showSuccess(`Rotated ${angle}° (${confidence}% confident - please verify)`);
} else {
  showError(`Low confidence (${confidence}%) - rotated ${angle}°, please verify`);
}
```

---

## Constants Reference

| Constant | Value | Purpose |
|----------|-------|---------|
| `EDGE_THRESHOLD` | 35.0 | Minimum gradient magnitude |
| `BORDER_ZONE_RATIO` | 0.25 | 25% outer region = border |
| `BORDER_WEIGHT_MULTIPLIER` | 2.5 | Border edge weight boost |
| `MIN_LINE_LENGTH_RATIO` | 0.10 | 10% min for "long" line |
| `AGREEMENT_THRESHOLD_DEG` | 1.5° | Max diff for "agreement" |

---

## Why Confidence is Conservative

The algorithm is intentionally conservative because:

1. **Consistent ≠ Correct**: A wrong angle can have very consistent signals if dominated by furniture, railings, or textures.

2. **Real estate photos rarely need large corrections**: Most professional cameras are reasonably level. Large detected angles are suspicious.

3. **Manual verification is easy**: A slightly under-corrected image is better than an over-rotated one.

4. **Method agreement is key**: High confidence only when multiple independent methods agree.

---

## Known Limitations

1. **Staircase railings**: Diagonal bars can dominate the signal with a wrong angle
2. **Patterned wallpaper/floors**: Repeating textures create many false edges
3. **No clear walls**: Images without vertical references (outdoor, abstract rooms)
4. **Very small images**: Skipped if < 50px in either dimension
5. **Extreme tilts**: Only searches ±6°, won't detect larger tilts

---

## Performance

- **Preview image used**: ~800px for processing (not full resolution)
- **Sampling**: Every 2nd pixel for speed
- **Parallel processing**: Uses Rayon for multi-threaded edge collection
- **Typical processing time**: 20-50ms on modern hardware
