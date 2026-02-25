// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Gather compute shader for GPU-resident candidate pipeline.
//
// Copies candidate elements from the full element buffer into a compacted
// output buffer using candidate indices produced by the Morton range query.
// Also writes indirect dispatch arguments so the subsequent hit test can
// be dispatched without any CPU readback.
//
// Flow:
//   1. Morton range query writes candidate indices + count to GPU buffers.
//   2. This shader reads those outputs and gathers the corresponding
//      ElementData entries into a contiguous output buffer.
//   3. Thread 0 also writes the indirect dispatch args for the hit test
//      compute shader (workgroup_x = ceil(count / 256), y = 1, z = 1).

struct ElementData {
    position: vec2<f32>,
    size: vec2<f32>,
    mark_type: u32,
    element_id: u32,
    selection_id: u32,
    _padding: u32,
}

// Full element buffer (all elements uploaded by the CPU).
@group(0) @binding(0) var<storage, read> all_elements: array<ElementData>;
// Candidate element indices produced by the Morton range query.
@group(0) @binding(1) var<storage, read> candidate_indices: array<u32>;
// Number of candidates (written atomically by the Morton query; read here
// as a plain u32 since the query has completed before this pass starts).
@group(0) @binding(2) var<storage, read> candidate_count: u32;
// Compacted output: only the candidate elements, in contiguous order.
@group(0) @binding(3) var<storage, read_write> gathered_elements: array<ElementData>;
// Indirect dispatch arguments for the hit test: [workgroup_x, workgroup_y, workgroup_z].
@group(0) @binding(4) var<storage, read_write> dispatch_indirect: array<u32, 3>;

@compute @workgroup_size(256)
fn gather_candidates(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let tid = global_id.x;
    let count = candidate_count;

    // Thread 0 writes the indirect dispatch args for the hit test shader.
    // The hit test uses @workgroup_size(256), so dispatch_x = ceil(count / 256).
    if tid == 0u {
        let hit_test_workgroup_size = 256u;
        dispatch_indirect[0] = (count + hit_test_workgroup_size - 1u) / hit_test_workgroup_size;
        dispatch_indirect[1] = 1u; // one query
        dispatch_indirect[2] = 1u;
    }

    if tid >= count {
        return;
    }

    let src_index = candidate_indices[tid];
    if src_index < arrayLength(&all_elements) {
        gathered_elements[tid] = all_elements[src_index];
    }
}
