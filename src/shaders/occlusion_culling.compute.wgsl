// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GPU compute shader for occlusion culling using a Hierarchical Z-Buffer (Hi-Z).
//
// Determines which instances are fully hidden behind other instances in
// screen space, allowing them to be skipped during rendering. Uses instance
// index as implicit z-order (higher index = drawn later = on top).
//
// Pipeline:
//   1. build_coverage      — populate level-0 coverage map via atomicMax(z)
//   2. generate_hiz_level  — build one Hi-Z mip level from the previous level
//   3. occlusion_test      — two-level test: coarse Hi-Z first, level-0 fallback
//
// The Hi-Z buffer is stored as a flat array with consecutive mip levels.
// Level 0 is the base coverage map at tile resolution. Each subsequent
// level stores the MINIMUM z-value of its 2×2 children, so a mark is
// occluded only if ALL covering cells have a higher z (i.e., something
// drawn later covers them).
//
// Two-level approach (GUP-223):
// For large marks that cover >= 4 cells per axis at a coarse mip level,
// the occlusion test first checks interior cells at the coarse level
// (shrunk by 1 cell on each edge to avoid boundary effects). If all
// interior cells agree (all visible or all occluded), the result is
// returned immediately without testing level-0 cells. For small marks
// or ambiguous coarse results, the test falls back to level-0.

struct InstanceData {
    transform: mat4x4<f32>,  // 64 bytes
    color: vec4<f32>,        // 16 bytes
    custom_data: vec4<f32>,  // 16 bytes
}

struct OcclusionConfig {
    base_width: u32,
    base_height: u32,
    num_levels: u32,
    instance_count: u32,
    viewport_min_x: f32,
    viewport_max_x: f32,
    viewport_min_y: f32,
    viewport_max_y: f32,
    pixel_width: f32,
    pixel_height: f32,
    conservative_margin: f32,
    current_level: u32,
    // Level offsets packed into vec4s (up to 12 levels).
    level_offsets_0: vec4<u32>,
    level_offsets_1: vec4<u32>,
    level_offsets_2: vec4<u32>,
}

@group(0) @binding(0) var<storage, read> instances: array<InstanceData>;
@group(0) @binding(1) var<storage, read_write> hiz_buffer: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> visibility: array<u32>;
@group(0) @binding(3) var<uniform> config: OcclusionConfig;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_level_offset(level: u32) -> u32 {
    let group = level / 4u;
    let idx = level % 4u;
    switch group {
        case 0u: { return config.level_offsets_0[idx]; }
        case 1u: { return config.level_offsets_1[idx]; }
        case 2u: { return config.level_offsets_2[idx]; }
        default: { return 0u; }
    }
}

fn level_dim(base: u32, level: u32) -> u32 {
    let divisor = 1u << level;
    return max((base + divisor - 1u) / divisor, 1u);
}

fn lev_width(level: u32) -> u32 {
    return level_dim(config.base_width, level);
}

fn lev_height(level: u32) -> u32 {
    return level_dim(config.base_height, level);
}

// Convert clip-space position to cell coordinates at a given level.
// Returns signed ints so callers can detect out-of-bounds.
fn clip_to_cell(clip_x: f32, clip_y: f32, level: u32) -> vec2<i32> {
    let w = f32(lev_width(level));
    let h = f32(lev_height(level));
    let vp_w = config.viewport_max_x - config.viewport_min_x;
    let vp_h = config.viewport_max_y - config.viewport_min_y;
    let nx = (clip_x - config.viewport_min_x) / vp_w;
    let ny = (clip_y - config.viewport_min_y) / vp_h;
    return vec2<i32>(
        i32(floor(nx * w)),
        i32(floor(ny * h))
    );
}

// ---------------------------------------------------------------------------
// Pass 1: Build coverage map (level 0)
// ---------------------------------------------------------------------------

@compute @workgroup_size(256)
fn build_coverage(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= config.instance_count) {
        return;
    }

    let inst = instances[idx];

    // Only opaque marks contribute to coverage.
    if (inst.color.w < 1.0) {
        return;
    }

    // Extract position and bounding radius from transform.
    let cx = inst.transform[3].x;
    let cy = inst.transform[3].y;
    let sx = length(inst.transform[0].xy);
    let sy = length(inst.transform[1].xy);
    let radius = max(sx, sy);

    // Skip if completely outside viewport.
    if (cx + radius < config.viewport_min_x || cx - radius > config.viewport_max_x ||
        cy + radius < config.viewport_min_y || cy - radius > config.viewport_max_y) {
        return;
    }

    // Z-value: instance index + 1 (0 = empty cell).
    let z_value = idx + 1u;

    // Bounding box in cell coordinates.
    let cell_min = clip_to_cell(cx - radius, cy - radius, 0u);
    let cell_max = clip_to_cell(cx + radius, cy + radius, 0u);

    let w = i32(lev_width(0u));
    let h = i32(lev_height(0u));
    let base = get_level_offset(0u);

    let cmin_x = max(cell_min.x, 0);
    let cmin_y = max(cell_min.y, 0);
    let cmax_x = min(cell_max.x, w - 1);
    let cmax_y = min(cell_max.y, h - 1);

    // Limit per-instance cell writes to avoid very long loops for huge marks.
    let max_cells = 4096;
    var cells_written = 0;

    for (var y = cmin_y; y <= cmax_y; y = y + 1) {
        for (var x = cmin_x; x <= cmax_x; x = x + 1) {
            let cell_idx = base + u32(y) * u32(w) + u32(x);
            atomicMax(&hiz_buffer[cell_idx], z_value);
            cells_written = cells_written + 1;
            if (cells_written >= max_cells) {
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pass 2: Generate one Hi-Z mip level
// ---------------------------------------------------------------------------
//
// Reads from level (current_level - 1) and writes the minimum z of each
// 2×2 block to current_level. Empty cells (z=0) are included in the min,
// ensuring that regions with ANY uncovered cell remain "open" (min=0).
// Dispatched once per level (1, 2, ..., num_levels-1).

@compute @workgroup_size(256)
fn generate_hiz_level(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let level = config.current_level;
    let dst_w = lev_width(level);
    let dst_h = lev_height(level);
    let cell_count = dst_w * dst_h;

    let idx = global_id.x;
    if (idx >= cell_count) {
        return;
    }

    let dst_x = idx % dst_w;
    let dst_y = idx / dst_w;

    let src_level = level - 1u;
    let src_w = lev_width(src_level);
    let src_h = lev_height(src_level);
    let src_offset = get_level_offset(src_level);
    let dst_offset = get_level_offset(level);

    // Read 2×2 block from source level and take the minimum.
    let sx = dst_x * 2u;
    let sy = dst_y * 2u;

    var min_z = 0xFFFFFFFFu;

    for (var dy = 0u; dy < 2u; dy = dy + 1u) {
        for (var dx = 0u; dx < 2u; dx = dx + 1u) {
            let src_x = sx + dx;
            let src_y = sy + dy;
            var val = 0u; // Out-of-bounds → uncovered.
            if (src_x < src_w && src_y < src_h) {
                let src_idx = src_offset + src_y * src_w + src_x;
                val = atomicLoad(&hiz_buffer[src_idx]);
            }
            min_z = min(min_z, val);
        }
    }

    // If no children existed at all (shouldn't happen), write 0.
    if (min_z == 0xFFFFFFFFu) {
        min_z = 0u;
    }

    let dst_idx = dst_offset + dst_y * dst_w + dst_x;
    atomicStore(&hiz_buffer[dst_idx], min_z);
}

// ---------------------------------------------------------------------------
// Coarse Hi-Z helper
// ---------------------------------------------------------------------------
//
// Result codes for the two-level Hi-Z test.
const COARSE_VISIBLE: u32 = 0u;
const COARSE_OCCLUDED: u32 = 1u;
const COARSE_AMBIGUOUS: u32 = 2u;

// Perform a two-level Hi-Z test using interior cells at a coarse mip level.
//
// Finds the coarsest mip level where the mark covers >= 4 cells per axis,
// then tests only interior cells (inset by 1 cell on each edge) to avoid
// edge effects from cells that straddle the bounding box boundary.
//
// Returns COARSE_VISIBLE (all interior cells show the mark is visible),
//         COARSE_OCCLUDED (all interior cells show the mark is occluded),
//      or COARSE_AMBIGUOUS (mixed result or no suitable coarse level).
fn coarse_hiz_test(
    mark_min_x: f32, mark_min_y: f32,
    mark_max_x: f32, mark_max_y: f32,
    z_value: u32
) -> u32 {
    // Find the coarsest mip level where the mark covers >= 4 cells per axis.
    // Iterate from level 1 upward; higher levels are coarser (fewer cells).
    // Once the mark covers < 4 cells on any axis, all coarser levels will
    // also fail the threshold, so we break early.
    var coarse_level = 0u;
    for (var level = 1u; level < config.num_levels; level = level + 1u) {
        let cmin = clip_to_cell(mark_min_x, mark_min_y, level);
        let cmax = clip_to_cell(mark_max_x, mark_max_y, level);
        if (cmax.x - cmin.x >= 3 && cmax.y - cmin.y >= 3) {
            coarse_level = level;
        } else {
            break;
        }
    }

    // No suitable coarse level: mark is too small for the two-level test.
    if (coarse_level == 0u) {
        return COARSE_AMBIGUOUS;
    }

    let cmin = clip_to_cell(mark_min_x, mark_min_y, coarse_level);
    let cmax = clip_to_cell(mark_max_x, mark_max_y, coarse_level);
    let w = i32(lev_width(coarse_level));
    let h = i32(lev_height(coarse_level));
    let level_off = get_level_offset(coarse_level);

    // Shrink by 1 cell on each edge for interior-only test.
    let inner_min_x = max(cmin.x + 1, 0);
    let inner_min_y = max(cmin.y + 1, 0);
    let inner_max_x = min(cmax.x - 1, w - 1);
    let inner_max_y = min(cmax.y - 1, h - 1);

    // Interior must cover at least 2×2 cells after shrinking.
    if (inner_max_x < inner_min_x || inner_max_y < inner_min_y) {
        return COARSE_AMBIGUOUS;
    }

    var any_visible = false;
    var any_occluded = false;

    for (var y = inner_min_y; y <= inner_max_y; y = y + 1) {
        for (var x = inner_min_x; x <= inner_max_x; x = x + 1) {
            let cell_idx = level_off + u32(y) * u32(w) + u32(x);
            let hiz_z = atomicLoad(&hiz_buffer[cell_idx]);
            if (hiz_z == 0u || z_value >= hiz_z) {
                any_visible = true;
            } else {
                any_occluded = true;
            }
            // Early exit: mixed result means we must fall back to level-0.
            if (any_visible && any_occluded) {
                return COARSE_AMBIGUOUS;
            }
        }
    }

    if (any_visible && !any_occluded) {
        return COARSE_VISIBLE;
    }
    if (any_occluded && !any_visible) {
        return COARSE_OCCLUDED;
    }
    return COARSE_AMBIGUOUS;
}

// ---------------------------------------------------------------------------
// Pass 3: Occlusion test
// ---------------------------------------------------------------------------
//
// Tests each instance's screen-space bounding box against the Hi-Z buffer.
// Uses a two-level approach: first tries a coarse mip level for large marks
// (early reject/accept), then falls back to level-0 for small marks or
// ambiguous coarse results.
// Writes visibility[idx] = 1 (visible) or 0 (occluded).

@compute @workgroup_size(256)
fn occlusion_test(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= config.instance_count) {
        return;
    }

    let inst = instances[idx];
    let cx = inst.transform[3].x;
    let cy = inst.transform[3].y;
    let sx = length(inst.transform[0].xy);
    let sy = length(inst.transform[1].xy);
    let radius = max(sx, sy);

    // Add conservative margin to avoid false positives.
    let padded_radius = radius + config.conservative_margin;
    let z_value = idx + 1u;

    // Skip if completely outside viewport — mark as visible (frustum
    // culling handles these separately).
    if (cx + padded_radius < config.viewport_min_x ||
        cx - padded_radius > config.viewport_max_x ||
        cy + padded_radius < config.viewport_min_y ||
        cy - padded_radius > config.viewport_max_y) {
        visibility[idx] = 1u;
        return;
    }

    // Two-level test: try coarse Hi-Z first for large marks.
    let coarse_result = coarse_hiz_test(
        cx - padded_radius, cy - padded_radius,
        cx + padded_radius, cy + padded_radius,
        z_value
    );
    if (coarse_result == COARSE_VISIBLE) {
        visibility[idx] = 1u;
        return;
    }
    if (coarse_result == COARSE_OCCLUDED) {
        visibility[idx] = 0u;
        return;
    }

    // Fallback: level-0 test for small marks or ambiguous coarse results.
    let test_level = 0u;

    let cell_min = clip_to_cell(cx - padded_radius, cy - padded_radius, test_level);
    let cell_max = clip_to_cell(cx + padded_radius, cy + padded_radius, test_level);
    let w = i32(lev_width(test_level));
    let h = i32(lev_height(test_level));
    let level_off = get_level_offset(test_level);

    // If mark extends beyond the Hi-Z grid → cannot determine occlusion.
    if (cell_min.x < 0 || cell_min.y < 0 || cell_max.x >= w || cell_max.y >= h) {
        visibility[idx] = 1u;
        return;
    }

    // Test all covered cells (with an iteration limit for very large marks).
    var is_occluded = true;
    let max_test_cells = 4096;
    var cells_tested = 0;

    for (var y = cell_min.y; y <= cell_max.y && is_occluded; y = y + 1) {
        for (var x = cell_min.x; x <= cell_max.x && is_occluded; x = x + 1) {
            let cell_idx = level_off + u32(y) * u32(w) + u32(x);
            let hiz_z = atomicLoad(&hiz_buffer[cell_idx]);
            // Cell is empty OR this instance is at/in front → visible.
            if (hiz_z == 0u || z_value >= hiz_z) {
                is_occluded = false;
            }
            cells_tested = cells_tested + 1;
            if (cells_tested >= max_test_cells) {
                // Cannot determine full occlusion; assume visible.
                is_occluded = false;
            }
        }
    }

    if (is_occluded) {
        visibility[idx] = 0u;
    } else {
        visibility[idx] = 1u;
    }
}

// ---------------------------------------------------------------------------
// Pass 3b: Combined occlusion test (for unified frustum+occlusion pipeline)
// ---------------------------------------------------------------------------
//
// Like occlusion_test, but preserves existing visibility flags from a prior
// frustum culling pass. Instances already marked invisible (visibility == 0)
// are skipped. Visible instances that are found to be occluded are set to 0.
// Visible instances that are NOT occluded are left unchanged (remain 1).
//
// Uses the same two-level approach: coarse Hi-Z first, level-0 fallback.
//
// This avoids overwriting frustum-cull decisions, allowing both stages to
// share a single visibility buffer.

@compute @workgroup_size(256)
fn occlusion_test_combined(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= config.instance_count) {
        return;
    }

    // Skip instances already culled by frustum pass.
    if (visibility[idx] == 0u) {
        return;
    }

    let inst = instances[idx];
    let cx = inst.transform[3].x;
    let cy = inst.transform[3].y;
    let sx = length(inst.transform[0].xy);
    let sy = length(inst.transform[1].xy);
    let radius = max(sx, sy);

    let padded_radius = radius + config.conservative_margin;
    let z_value = idx + 1u;

    // Skip if completely outside viewport — already visible from frustum pass.
    if (cx + padded_radius < config.viewport_min_x ||
        cx - padded_radius > config.viewport_max_x ||
        cy + padded_radius < config.viewport_min_y ||
        cy - padded_radius > config.viewport_max_y) {
        return;
    }

    // Two-level test: try coarse Hi-Z first for large marks.
    let coarse_result = coarse_hiz_test(
        cx - padded_radius, cy - padded_radius,
        cx + padded_radius, cy + padded_radius,
        z_value
    );
    if (coarse_result == COARSE_VISIBLE) {
        // Visible — leave visibility flag as 1 (from frustum pass).
        return;
    }
    if (coarse_result == COARSE_OCCLUDED) {
        visibility[idx] = 0u;
        return;
    }

    // Fallback: level-0 test.
    let test_level = 0u;

    let cell_min = clip_to_cell(cx - padded_radius, cy - padded_radius, test_level);
    let cell_max = clip_to_cell(cx + padded_radius, cy + padded_radius, test_level);
    let w = i32(lev_width(test_level));
    let h = i32(lev_height(test_level));
    let level_off = get_level_offset(test_level);

    if (cell_min.x < 0 || cell_min.y < 0 || cell_max.x >= w || cell_max.y >= h) {
        return;
    }

    var is_occluded = true;
    let max_test_cells = 4096;
    var cells_tested = 0;

    for (var y = cell_min.y; y <= cell_max.y && is_occluded; y = y + 1) {
        for (var x = cell_min.x; x <= cell_max.x && is_occluded; x = x + 1) {
            let cell_idx = level_off + u32(y) * u32(w) + u32(x);
            let hiz_z = atomicLoad(&hiz_buffer[cell_idx]);
            if (hiz_z == 0u || z_value >= hiz_z) {
                is_occluded = false;
            }
            cells_tested = cells_tested + 1;
            if (cells_tested >= max_test_cells) {
                is_occluded = false;
            }
        }
    }

    // Only clear to 0 if occluded; never write 1 (preserve frustum result).
    if (is_occluded) {
        visibility[idx] = 0u;
    }
}
