// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GPU compute shader for instance culling, LOD classification, and Z-order sorting.
//
// Processes InstanceAttributes on the GPU to produce a compact output buffer
// of visible instances plus DrawIndirect parameters, eliminating CPU overhead
// for datasets exceeding 1M instances.
//
// Pipeline:
//   1. cull_and_classify — frustum test + LOD classification per instance
//   2. prefix_sum        — parallel exclusive prefix sum over visibility flags
//   3. compact_and_sort  — scatter visible instances to output + write DrawIndirect

// Must match Rust `InstanceAttributes` layout (96 bytes, #[repr(C)]).
struct InstanceData {
    transform: mat4x4<f32>,  // 64 bytes
    color: vec4<f32>,        // 16 bytes
    custom_data: vec4<f32>,  // 16 bytes
}

struct FilterConfig {
    // Viewport frustum bounds (clip-space).
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    // Viewport dimensions in physical pixels.
    pixel_width: f32,
    pixel_height: f32,
    // LOD thresholds: [full→simplified, simplified→point, point→culled].
    lod_full: f32,
    lod_simplified: f32,
    lod_point: f32,
    // Total number of instances to process.
    instance_count: u32,
    // Vertex count per instance (for DrawIndirect).
    vertex_count: u32,
    // Whether to apply Z-order sorting (0 = no, 1 = yes).
    enable_sort: u32,
}

// DrawIndirect parameters (matches wgpu::DrawIndirectArgs layout).
struct DrawIndirectArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

@group(0) @binding(0) var<storage, read> instances: array<InstanceData>;
@group(0) @binding(1) var<storage, read_write> output_instances: array<InstanceData>;
@group(0) @binding(2) var<storage, read_write> visibility: array<u32>;
@group(0) @binding(3) var<storage, read_write> prefix_sums: array<u32>;
@group(0) @binding(4) var<storage, read_write> draw_indirect: DrawIndirectArgs;
@group(0) @binding(5) var<uniform> config: FilterConfig;

// Shared memory for workgroup-local prefix sum.
var<workgroup> shared_data: array<u32, 256>;

// ---- Pass 1: Frustum culling + LOD classification ----

fn is_visible(cx: f32, cy: f32, radius: f32) -> bool {
    return cx + radius >= config.min_x
        && cx - radius <= config.max_x
        && cy + radius >= config.min_y
        && cy - radius <= config.max_y;
}

// LOD levels encoded as: 0 = culled, 1 = point, 2 = simplified, 3 = full.
fn compute_lod(clip_radius: f32) -> u32 {
    let pixel_size = clip_radius * config.pixel_width / 2.0;
    if (pixel_size >= config.lod_full) {
        return 3u; // Full
    } else if (pixel_size >= config.lod_simplified) {
        return 2u; // Simplified
    } else if (pixel_size >= config.lod_point) {
        return 1u; // Point
    }
    return 0u; // Culled
}

@compute @workgroup_size(256)
fn cull_and_classify(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let idx = global_id.x;
    if (idx >= config.instance_count) {
        // Out-of-bounds threads write 0 so prefix sum is correct.
        if (idx < arrayLength(&visibility)) {
            visibility[idx] = 0u;
        }
        return;
    }

    let inst = instances[idx];

    // Extract position from column 3 of the transform matrix.
    let cx = inst.transform[3].x;
    let cy = inst.transform[3].y;

    // Bounding radius: max of x/y scale columns length.
    let sx = length(inst.transform[0].xy);
    let sy = length(inst.transform[1].xy);
    let radius = max(sx, sy);

    var vis = 0u;
    if (is_visible(cx, cy, radius)) {
        let lod = compute_lod(radius);
        if (lod > 0u) {
            vis = 1u;
        }
    }

    visibility[idx] = vis;
}

// ---- Pass 2: Prefix sum (Blelloch-style workgroup-level scan) ----
//
// For large arrays this must be called in multiple passes:
//   1. Per-workgroup prefix sum → write partial sums to prefix_sums[]
//   2. A small second dispatch scans the partial sums (block sums)
//   3. A third dispatch adds block offsets back
//
// This shader handles the per-workgroup scan. The Rust host orchestrates
// the multi-pass approach for arrays larger than one workgroup.

@compute @workgroup_size(256)
fn prefix_sum_workgroup(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
) {
    let tid = local_id.x;
    let gid = global_id.x;

    // Load input.
    if (gid < config.instance_count) {
        shared_data[tid] = visibility[gid];
    } else {
        shared_data[tid] = 0u;
    }
    workgroupBarrier();

    // Up-sweep (reduce) phase.
    for (var stride = 1u; stride < 256u; stride = stride * 2u) {
        let idx = (tid + 1u) * stride * 2u - 1u;
        if (idx < 256u) {
            shared_data[idx] = shared_data[idx] + shared_data[idx - stride];
        }
        workgroupBarrier();
    }

    // Store block total and clear last element for down-sweep.
    if (tid == 0u) {
        // Store block sum for inter-workgroup scan.
        let block_idx = workgroup_id.x;
        prefix_sums[config.instance_count + block_idx] = shared_data[255u];
        shared_data[255u] = 0u;
    }
    workgroupBarrier();

    // Down-sweep phase.
    for (var stride = 128u; stride >= 1u; stride = stride / 2u) {
        let idx = (tid + 1u) * stride * 2u - 1u;
        if (idx < 256u) {
            let temp = shared_data[idx - stride];
            shared_data[idx - stride] = shared_data[idx];
            shared_data[idx] = shared_data[idx] + temp;
        }
        workgroupBarrier();
    }

    // Write exclusive prefix sum output.
    if (gid < config.instance_count) {
        prefix_sums[gid] = shared_data[tid];
    }
}

// Scan block sums (called on a single workgroup for up to 256 blocks).
@compute @workgroup_size(256)
fn prefix_sum_blocks(
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let tid = local_id.x;
    let num_blocks = (config.instance_count + 255u) / 256u;

    // Load block sum.
    if (tid < num_blocks) {
        shared_data[tid] = prefix_sums[config.instance_count + tid];
    } else {
        shared_data[tid] = 0u;
    }
    workgroupBarrier();

    // Up-sweep.
    for (var stride = 1u; stride < 256u; stride = stride * 2u) {
        let idx = (tid + 1u) * stride * 2u - 1u;
        if (idx < 256u) {
            shared_data[idx] = shared_data[idx] + shared_data[idx - stride];
        }
        workgroupBarrier();
    }

    if (tid == 0u) {
        // Total visible count → write to draw indirect.
        draw_indirect.instance_count = shared_data[255u];
        draw_indirect.vertex_count = config.vertex_count;
        draw_indirect.first_vertex = 0u;
        draw_indirect.first_instance = 0u;
        shared_data[255u] = 0u;
    }
    workgroupBarrier();

    // Down-sweep.
    for (var stride = 128u; stride >= 1u; stride = stride / 2u) {
        let idx = (tid + 1u) * stride * 2u - 1u;
        if (idx < 256u) {
            let temp = shared_data[idx - stride];
            shared_data[idx - stride] = shared_data[idx];
            shared_data[idx] = shared_data[idx] + temp;
        }
        workgroupBarrier();
    }

    // Write scanned block sums back.
    if (tid < num_blocks) {
        prefix_sums[config.instance_count + tid] = shared_data[tid];
    }
}

// Add block offsets to per-element prefix sums.
@compute @workgroup_size(256)
fn prefix_sum_add_block_offsets(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
) {
    let gid = global_id.x;
    if (gid >= config.instance_count) {
        return;
    }

    let block_offset = prefix_sums[config.instance_count + workgroup_id.x];
    prefix_sums[gid] = prefix_sums[gid] + block_offset;
}

// ---- Pass 3: Compact visible instances into output buffer ----

@compute @workgroup_size(256)
fn compact_instances(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let idx = global_id.x;
    if (idx >= config.instance_count) {
        return;
    }

    if (visibility[idx] == 1u) {
        let out_idx = prefix_sums[idx];
        output_instances[out_idx] = instances[idx];
    }
}
