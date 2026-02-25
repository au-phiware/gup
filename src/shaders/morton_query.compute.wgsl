// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GPU-side Morton range query compute shader.
//
// Performs binary search on a sorted array of Morton entries entirely on
// the GPU, eliminating the CPU roundtrip for candidate narrowing.
//
// Strategy:
//   1. A small number of threads (1 per query) compute Morton key ranges
//      from the query position/region.
//   2. Binary search finds the start and end of matching entries in the
//      sorted buffer.
//   3. Matching element indices are written to the output candidate buffer
//      along with a count.
//
// The caller then dispatches the hit test compute shader using only the
// candidates found here.

// A single entry in the sorted Morton key buffer: (key, element_index).
struct MortonEntry {
    key: u32,
    element_index: u32,
}

// Query configuration uploaded by the CPU.
struct MortonQueryConfig {
    // Query type: 0 = point, 1 = region
    query_type: u32,
    // Search radius in grid cells for point queries
    search_radius: u32,
    // Total number of entries in the sorted Morton buffer
    entry_count: u32,
    // Maximum candidates to output
    max_candidates: u32,
    // Query position in world coordinates
    query_position: vec2<f32>,
    // Query region half-extents (for region queries)
    query_half_extent: vec2<f32>,
    // World bounds min
    world_bounds_min: vec2<f32>,
    // World bounds max
    world_bounds_max: vec2<f32>,
}

@group(0) @binding(0) var<storage, read> morton_entries: array<MortonEntry>;
@group(0) @binding(1) var<uniform> config: MortonQueryConfig;
@group(0) @binding(2) var<storage, read_write> candidates: array<u32>;
@group(0) @binding(3) var<storage, read_write> candidate_count: atomic<u32>;

// --- Morton encoding utilities ---

fn interleave_bits(v_in: u32) -> u32 {
    var v = v_in & 0x0000FFFFu;
    v = (v | (v << 8u)) & 0x00FF00FFu;
    v = (v | (v << 4u)) & 0x0F0F0F0Fu;
    v = (v | (v << 2u)) & 0x33333333u;
    v = (v | (v << 1u)) & 0x55555555u;
    return v;
}

fn morton_encode(x: u32, y: u32) -> u32 {
    return interleave_bits(x) | (interleave_bits(y) << 1u);
}

fn deinterleave_bits(v_in: u32) -> u32 {
    var v = v_in & 0x55555555u;
    v = (v | (v >> 1u)) & 0x33333333u;
    v = (v | (v >> 2u)) & 0x0F0F0F0Fu;
    v = (v | (v >> 4u)) & 0x00FF00FFu;
    v = (v | (v >> 8u)) & 0x0000FFFFu;
    return v;
}

fn morton_decode(key: u32) -> vec2<u32> {
    let x = deinterleave_bits(key);
    let y = deinterleave_bits(key >> 1u);
    return vec2<u32>(x, y);
}

// Convert a world position to a 16-bit grid coordinate pair.
fn world_to_grid(world_pos: vec2<f32>) -> vec2<u32> {
    let range = config.world_bounds_max - config.world_bounds_min;
    let safe_range = max(range, vec2<f32>(0.0001, 0.0001));
    let normalized = clamp(
        (world_pos - config.world_bounds_min) / safe_range,
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.9999, 0.9999)
    );
    let grid_pos = normalized * vec2<f32>(65536.0, 65536.0);
    return vec2<u32>(u32(grid_pos.x), u32(grid_pos.y));
}

// --- Binary search ---

// Find the first index where entries[i].key >= key_val (lower bound).
fn lower_bound(key_val: u32, count: u32) -> u32 {
    var lo: u32 = 0u;
    var hi: u32 = count;
    loop {
        if lo >= hi {
            break;
        }
        let mid = lo + (hi - lo) / 2u;
        if morton_entries[mid].key < key_val {
            lo = mid + 1u;
        } else {
            hi = mid;
        }
    }
    return lo;
}

// Find the first index where entries[i].key > key_val (upper bound).
fn upper_bound(key_val: u32, count: u32) -> u32 {
    var lo: u32 = 0u;
    var hi: u32 = count;
    loop {
        if lo >= hi {
            break;
        }
        let mid = lo + (hi - lo) / 2u;
        if morton_entries[mid].key <= key_val {
            lo = mid + 1u;
        } else {
            hi = mid;
        }
    }
    return lo;
}

// --- Main entry point ---
//
// We use a small workgroup because the query itself is lightweight; the
// parallelism payoff comes from eliminating the GPU↔CPU roundtrip and
// keeping the entire query path on the GPU.

@compute @workgroup_size(64)
fn morton_range_query(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.x;

    // Only the first thread performs the query (single query per dispatch).
    if thread_id != 0u {
        return;
    }

    let count = config.entry_count;
    if count == 0u {
        return;
    }

    // Compute the Morton key range based on query type.
    var lo_key: u32;
    var hi_key: u32;

    if config.query_type == 0u {
        // --- Point query ---
        let grid_pos = world_to_grid(config.query_position);
        let radius = config.search_radius;

        let min_x = select(0u, grid_pos.x - radius, grid_pos.x >= radius);
        let min_y = select(0u, grid_pos.y - radius, grid_pos.y >= radius);
        let max_x = min(grid_pos.x + radius, 65535u);
        let max_y = min(grid_pos.y + radius, 65535u);

        lo_key = morton_encode(min_x, min_y);
        hi_key = morton_encode(max_x, max_y);
    } else {
        // --- Region query ---
        let region_min = config.query_position - config.query_half_extent;
        let region_max = config.query_position + config.query_half_extent;

        let grid_min = world_to_grid(region_min);
        let grid_max = world_to_grid(region_max);

        lo_key = morton_encode(grid_min.x, grid_min.y);
        hi_key = morton_encode(grid_max.x, grid_max.y);
    }

    // Ensure lo <= hi
    if lo_key > hi_key {
        let tmp = lo_key;
        lo_key = hi_key;
        hi_key = tmp;
    }

    // Binary search for the key range.
    let start = lower_bound(lo_key, count);
    let end = upper_bound(hi_key, count);

    // Write matching element indices to the candidate buffer.
    let max_out = config.max_candidates;
    var written: u32 = 0u;

    for (var i: u32 = start; i < end; i = i + 1u) {
        if written >= max_out {
            break;
        }
        let slot = atomicAdd(&candidate_count, 1u);
        if slot < max_out {
            candidates[slot] = morton_entries[i].element_index;
            written = written + 1u;
        } else {
            break;
        }
    }
}
