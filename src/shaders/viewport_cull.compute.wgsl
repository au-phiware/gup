// Viewport frustum culling for VertexData (LOD pyramid points).
//
// Takes an array of VertexData points and produces a compacted output buffer
// containing only the points that fall within the viewport bounds. An indirect
// draw argument buffer is also populated.
//
// Designed to reuse the same architectural pattern as instance_filter.compute.wgsl
// (frustum test + prefix-sum + compaction) but for the simpler 16-byte VertexData
// layout rather than the 96-byte InstanceAttributes.

struct VertexData {
    x: f32,
    y: f32,
    weight: f32,
    _padding: f32,
}

struct CullConfig {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    point_count: u32,
    vertex_count: u32,
    _pad0: u32,
    _pad1: u32,
}

// Binding 0: input points (read-only)
@group(0) @binding(0) var<storage, read> input_points: array<VertexData>;
// Binding 1: output points (compacted)
@group(0) @binding(1) var<storage, read_write> output_points: array<VertexData>;
// Binding 2: configuration
@group(0) @binding(2) var<uniform> config: CullConfig;
// Binding 3: visibility flags (1 = visible, 0 = culled)
@group(0) @binding(3) var<storage, read_write> visibility: array<u32>;
// Binding 4: prefix sums for compaction
@group(0) @binding(4) var<storage, read_write> prefix_sums: array<u32>;
// Binding 5: draw indirect args [vertex_count, instance_count, first_vertex, first_instance]
@group(0) @binding(5) var<storage, read_write> draw_indirect: array<u32>;
// Binding 6: block sums for multi-workgroup prefix scan
@group(0) @binding(6) var<storage, read_write> block_sums: array<u32>;
// Binding 7: super-block sums for two-level prefix scan
@group(0) @binding(7) var<storage, read_write> super_block_sums: array<u32>;

const WORKGROUP_SIZE: u32 = 256u;

var<workgroup> shared_data: array<u32, 256>;

// --- Pass 1: Frustum cull ---

@compute @workgroup_size(256)
fn cull_points(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if idx >= config.point_count {
        if idx < arrayLength(&visibility) {
            visibility[idx] = 0u;
        }
        return;
    }

    let pt = input_points[idx];
    let inside = pt.x >= config.min_x && pt.x <= config.max_x
              && pt.y >= config.min_y && pt.y <= config.max_y;

    visibility[idx] = select(0u, 1u, inside);
}

// --- Pass 2: Workgroup-level prefix sum ---

@compute @workgroup_size(256)
fn prefix_sum_workgroup(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    let idx = gid.x;
    let local_idx = lid.x;

    // Load visibility flag into shared memory.
    if idx < config.point_count {
        shared_data[local_idx] = visibility[idx];
    } else {
        shared_data[local_idx] = 0u;
    }
    workgroupBarrier();

    // Hillis-Steele inclusive scan.
    for (var offset = 1u; offset < WORKGROUP_SIZE; offset = offset << 1u) {
        var val = 0u;
        if local_idx >= offset {
            val = shared_data[local_idx - offset];
        }
        workgroupBarrier();
        shared_data[local_idx] += val;
        workgroupBarrier();
    }

    // Write result. Convert inclusive to exclusive by subtracting original value.
    if idx < config.point_count {
        let inclusive = shared_data[local_idx];
        let original = visibility[idx];
        prefix_sums[idx] = inclusive - original;
    }

    // Last thread writes the block sum.
    if local_idx == WORKGROUP_SIZE - 1u {
        block_sums[wid.x] = shared_data[local_idx];
    }
}

// --- Pass 3: Scan block sums ---
// Handles up to 256×256 = 65536 blocks (16.7M points).
// For larger workload, the block scan is performed in two levels:
// Level 1: scan each chunk of 256 block sums
// Level 2: scan the chunk totals

@compute @workgroup_size(256)
fn prefix_sum_blocks(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    let local_idx = lid.x;
    let chunk_idx = wid.x;
    let num_blocks = (config.point_count + WORKGROUP_SIZE - 1u) / WORKGROUP_SIZE;
    let block_offset = chunk_idx * WORKGROUP_SIZE;
    let global_block_idx = block_offset + local_idx;

    if global_block_idx < num_blocks {
        shared_data[local_idx] = block_sums[global_block_idx];
    } else {
        shared_data[local_idx] = 0u;
    }
    workgroupBarrier();

    // Hillis-Steele inclusive scan within this chunk.
    for (var offset = 1u; offset < WORKGROUP_SIZE; offset = offset << 1u) {
        var val = 0u;
        if local_idx >= offset {
            val = shared_data[local_idx - offset];
        }
        workgroupBarrier();
        shared_data[local_idx] += val;
        workgroupBarrier();
    }

    // Convert to exclusive scan and write back.
    if global_block_idx < num_blocks {
        if local_idx == 0u {
            block_sums[global_block_idx] = 0u;
        } else {
            block_sums[global_block_idx] = shared_data[local_idx - 1u];
        }
    }

    // Store this chunk's total in the super_block_sums for inter-chunk fixup.
    if local_idx == WORKGROUP_SIZE - 1u {
        let items_in_chunk = min(WORKGROUP_SIZE, num_blocks - block_offset);
        super_block_sums[chunk_idx] = shared_data[items_in_chunk - 1u];
    }
}

// --- Pass 3b: Scan super-block sums (single workgroup) ---

@compute @workgroup_size(256)
fn prefix_sum_super_blocks(
    @builtin(local_invocation_id) lid: vec3<u32>,
) {
    let local_idx = lid.x;
    let num_blocks = (config.point_count + WORKGROUP_SIZE - 1u) / WORKGROUP_SIZE;
    let num_chunks = (num_blocks + WORKGROUP_SIZE - 1u) / WORKGROUP_SIZE;

    if local_idx < num_chunks {
        shared_data[local_idx] = super_block_sums[local_idx];
    } else {
        shared_data[local_idx] = 0u;
    }
    workgroupBarrier();

    for (var offset = 1u; offset < WORKGROUP_SIZE; offset = offset << 1u) {
        var val = 0u;
        if local_idx >= offset {
            val = shared_data[local_idx - offset];
        }
        workgroupBarrier();
        shared_data[local_idx] += val;
        workgroupBarrier();
    }

    // Write exclusive prefix sum for super-block offsets.
    if local_idx < num_chunks {
        if local_idx == 0u {
            super_block_sums[local_idx] = 0u;
        } else {
            super_block_sums[local_idx] = shared_data[local_idx - 1u];
        }
    }

    // Write total count to draw_indirect.
    if local_idx == 0u {
        let total = shared_data[min(num_chunks - 1u, WORKGROUP_SIZE - 1u)];
        draw_indirect[0] = config.vertex_count;
        draw_indirect[1] = total;
        draw_indirect[2] = 0u;
        draw_indirect[3] = 0u;
    }
}

// --- Pass 4: Add block and super-block offsets to per-element prefix sums ---

@compute @workgroup_size(256)
fn prefix_sum_add_offsets(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    let idx = gid.x;
    if idx >= config.point_count {
        return;
    }
    let chunk_idx = wid.x / WORKGROUP_SIZE;
    prefix_sums[idx] += block_sums[wid.x] + super_block_sums[chunk_idx];
}

// --- Pass 5: Compact visible points ---

@compute @workgroup_size(256)
fn compact_points(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if idx >= config.point_count {
        return;
    }
    if visibility[idx] == 1u {
        let out_idx = prefix_sums[idx];
        output_points[out_idx] = input_points[idx];
    }
}
