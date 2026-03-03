// LOD Aggregate Compute Shader
//
// Grid-based point aggregation for building LOD pyramid levels.
//
// Two-pass design:
//   1. assign_main — maps each input point to a grid cell using atomicMin
//      to deterministically select the lowest-indexed point per cell.
//   2. compact_main — scans the grid and writes occupied cells' representative
//      points to a compact output buffer.

// Per-point vertex data — must match the Rust VertexData layout (16 bytes).
struct VertexData {
    x: f32,
    y: f32,
    weight: f32,
    _padding: f32,
}

// Uniform parameters from the host.
struct Params {
    grid_width: u32,
    grid_height: u32,
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
    input_count: u32,
    _padding: u32,
}

@group(0) @binding(0) var<storage, read>       input:          array<VertexData>;
@group(0) @binding(1) var<storage, read_write>  output:         array<VertexData>;
@group(0) @binding(2) var<uniform>              params:         Params;
@group(0) @binding(3) var<storage, read_write>  grid:           array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write>  output_counter: array<atomic<u32>>;

// ---------------------------------------------------------------------------
// Pass 1 — Assign each input point to a grid cell.
// ---------------------------------------------------------------------------

@compute @workgroup_size(256)
fn assign_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if idx >= params.input_count {
        return;
    }

    let p = input[idx];
    let extent_x = params.max_x - params.min_x;
    let extent_y = params.max_y - params.min_y;

    // Map point to grid cell coordinates.
    var cx = u32(((p.x - params.min_x) / extent_x) * f32(params.grid_width));
    var cy = u32(((p.y - params.min_y) / extent_y) * f32(params.grid_height));
    cx = min(cx, params.grid_width - 1u);
    cy = min(cy, params.grid_height - 1u);

    let cell = cy * params.grid_width + cx;

    // Deterministic first-point-wins: keep the lowest index via atomicMin.
    atomicMin(&grid[cell], idx);
}

// ---------------------------------------------------------------------------
// Pass 2 — Compact occupied cells into the output buffer.
// ---------------------------------------------------------------------------

@compute @workgroup_size(256)
fn compact_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cell = gid.x;
    let total_cells = params.grid_width * params.grid_height;
    if cell >= total_cells {
        return;
    }

    let rep = atomicLoad(&grid[cell]);
    if rep != 0xFFFFFFFFu {
        // Occupied cell — write the representative point to the output.
        let out_idx = atomicAdd(&output_counter[0], 1u);
        output[out_idx] = input[rep];
    }
}
