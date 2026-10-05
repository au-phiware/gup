// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GPU compute shader for 8-bit radix sort of instances by Z-depth.
//
// Sorts compacted visible instances from the instance filter pipeline
// in back-to-front order (descending Z) for correct transparent rendering.
//
// Pipeline (per radix pass, 4 passes total for 32-bit keys):
//   1. extract_sort_keys       — convert Z-depth to sortable u32 keys (once)
//   2. radix_histogram         — per-workgroup histogram of current 8-bit digit
//   3. histogram_scan_*        — multi-level prefix sum over histogram
//   4. radix_scatter           — scatter keys+values to sorted positions
//   5. reorder_instances       — final instance reorder using sorted indices (once)

// Must match Rust `InstanceAttributes` layout (96 bytes, #[repr(C)]).
struct InstanceData {
    transform: mat4x4<f32>,  // 64 bytes
    color: vec4<f32>,        // 16 bytes
    custom_data: vec4<f32>,  // 16 bytes
}

// DrawIndirect parameters (matches wgpu::DrawIndirectArgs layout).
struct DrawIndirectArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

// Sort configuration uniform (32 bytes = 8 × u32).
struct SortConfig {
    // Total number of elements to process (= instance_count from filter).
    num_elements: u32,
    // Current radix pass (0..3). Determines which 8-bit digit to sort on.
    radix_pass: u32,
    // Number of sort workgroups = ceil(num_elements / 256).
    num_sort_wg: u32,
    // For prefix sum: total number of histogram entries to scan.
    prefix_count: u32,
    // For prefix sum: offset in histograms[] where block totals are stored.
    prefix_block_offset: u32,
    // For prefix sum: offset in histograms[] where the data to scan starts.
    prefix_data_offset: u32,
    _pad0: u32,
    _pad1: u32,
}

const RADIX_BITS: u32 = 8u;
const RADIX_SIZE: u32 = 256u;  // 2^RADIX_BITS
const WG_SIZE: u32 = 256u;

@group(0) @binding(0) var<storage, read> instances_src: array<InstanceData>;
@group(0) @binding(1) var<storage, read_write> instances_dst: array<InstanceData>;
@group(0) @binding(2) var<storage, read_write> keys_a: array<u32>;
@group(0) @binding(3) var<storage, read_write> keys_b: array<u32>;
@group(0) @binding(4) var<storage, read_write> vals_a: array<u32>;
@group(0) @binding(5) var<storage, read_write> vals_b: array<u32>;
@group(0) @binding(6) var<storage, read_write> histograms: array<u32>;
@group(0) @binding(7) var<storage, read> draw_indirect: DrawIndirectArgs;
@group(0) @binding(8) var<uniform> sort_config: SortConfig;

// Shared memory for workgroup operations.
var<workgroup> shared_data: array<u32, 256>;
var<workgroup> shared_hist: array<atomic<u32>, 256>;
// Per-digit thread membership bitmask (256 digits × 8 u32 words = 256 bits per digit).
// Used by radix_scatter for O(n) local rank computation via popcount.
var<workgroup> digit_member_bits: array<atomic<u32>, 2048>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// Convert a float to a sortable u32 in descending order.
// Large Z values produce small keys → sorted first (back-to-front).
fn float_to_descending_key(f: f32) -> u32 {
    let bits = bitcast<u32>(f);
    let mask = select(0x80000000u, 0xFFFFFFFFu, (bits & 0x80000000u) != 0u);
    let ascending_key = bits ^ mask;
    return ~ascending_key;
}

// Extract the current 8-bit digit from a key.
fn extract_digit(key: u32, radix_pass: u32) -> u32 {
    return (key >> (radix_pass * RADIX_BITS)) & 0xFFu;
}

// Ping-pong read/write: even passes read A→write B, odd passes read B→write A.
fn read_key(idx: u32) -> u32 {
    if (sort_config.radix_pass % 2u == 0u) {
        return keys_a[idx];
    } else {
        return keys_b[idx];
    }
}

fn write_key(idx: u32, val: u32) {
    if (sort_config.radix_pass % 2u == 0u) {
        keys_b[idx] = val;
    } else {
        keys_a[idx] = val;
    }
}

fn read_val(idx: u32) -> u32 {
    if (sort_config.radix_pass % 2u == 0u) {
        return vals_a[idx];
    } else {
        return vals_b[idx];
    }
}

fn write_val(idx: u32, val: u32) {
    if (sort_config.radix_pass % 2u == 0u) {
        vals_b[idx] = val;
    } else {
        vals_a[idx] = val;
    }
}

// ---- Entry point 1: Extract sort keys from compacted instances ----

@compute @workgroup_size(256)
fn extract_sort_keys(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let idx = global_id.x;
    if (idx >= sort_config.num_elements) {
        return;
    }

    let visible_count = draw_indirect.instance_count;

    if (idx < visible_count) {
        let z = instances_src[idx].transform[3].z;
        keys_a[idx] = float_to_descending_key(z);
    } else {
        // Non-visible slots get max key → sort to end.
        keys_a[idx] = 0xFFFFFFFFu;
    }
    vals_a[idx] = idx;
}

// ---- Entry point 2: Build per-workgroup histogram ----
//
// Layout: histograms[digit * num_sort_wg + wg_id]

@compute @workgroup_size(256)
fn radix_histogram(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
) {
    let tid = local_id.x;
    let gid = global_id.x;
    let wg_id = workgroup_id.x;

    // Clear shared histogram.
    atomicStore(&shared_hist[tid], 0u);
    workgroupBarrier();

    // Count digit occurrences via atomic increment.
    if (gid < sort_config.num_elements) {
        let key = read_key(gid);
        let digit = extract_digit(key, sort_config.radix_pass);
        atomicAdd(&shared_hist[digit], 1u);
    }
    workgroupBarrier();

    // Write per-workgroup histogram to global memory.
    if (tid < RADIX_SIZE) {
        histograms[tid * sort_config.num_sort_wg + wg_id] = atomicLoad(&shared_hist[tid]);
    }
}

// ---- Entry points 3a-3c: Multi-level prefix sum over histogram ----
//
// These are Blelloch-style scans identical to instance_filter but
// operating on the histograms[] buffer. Uses prefix_count and
// prefix_block_offset from SortConfig.

@compute @workgroup_size(256)
fn histogram_scan_workgroup(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
) {
    let tid = local_id.x;
    let gid = global_id.x;
    let data_base = sort_config.prefix_data_offset;

    // Load input.
    if (gid < sort_config.prefix_count) {
        shared_data[tid] = histograms[data_base + gid];
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
        histograms[sort_config.prefix_block_offset + workgroup_id.x] = shared_data[255u];
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
    if (gid < sort_config.prefix_count) {
        histograms[data_base + gid] = shared_data[tid];
    }
}

// Scan block sums (called on a single workgroup for up to 256 blocks).
@compute @workgroup_size(256)
fn histogram_scan_blocks(
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let tid = local_id.x;
    let num_blocks = (sort_config.prefix_count + 255u) / 256u;
    let block_base = sort_config.prefix_block_offset;

    if (tid < num_blocks) {
        shared_data[tid] = histograms[block_base + tid];
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

    if (tid < num_blocks) {
        histograms[block_base + tid] = shared_data[tid];
    }
}

// Add block offsets to per-element prefix sums.
@compute @workgroup_size(256)
fn histogram_scan_add_offsets(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
) {
    let gid = global_id.x;
    if (gid >= sort_config.prefix_count) {
        return;
    }

    let data_base = sort_config.prefix_data_offset;
    let block_offset = histograms[sort_config.prefix_block_offset + workgroup_id.x];
    histograms[data_base + gid] = histograms[data_base + gid] + block_offset;
}

// ---- Entry point 4: Scatter keys and values to sorted positions ----
//
// Each thread determines its output position from:
//   global_offset = prefix_sum[digit * num_sort_wg + wg_id]
//   local_rank    = popcount of lower-TID bits in same-digit bitmask
//                   (ensures stability — threads with lower TID get lower rank)
//
// Uses a per-digit 256-bit bitmask in shared memory for O(n) total work
// per workgroup, replacing the previous O(n²) serial scan.

@compute @workgroup_size(256)
fn radix_scatter(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
) {
    let tid = local_id.x;
    let gid = global_id.x;
    let wg_id = workgroup_id.x;

    // Load this thread's key, value, and digit.
    var my_key = 0xFFFFFFFFu;
    var my_val = 0u;
    var my_digit = RADIX_SIZE - 1u;
    let in_range = gid < sort_config.num_elements;

    if (in_range) {
        my_key = read_key(gid);
        my_val = read_val(gid);
        my_digit = extract_digit(my_key, sort_config.radix_pass);
    }

    // Phase 1: Clear the per-digit bitmask.
    // 256 threads × 8 words each = 2048 words total.
    for (var i = 0u; i < 8u; i = i + 1u) {
        atomicStore(&digit_member_bits[tid * 8u + i], 0u);
    }
    workgroupBarrier();

    // Phase 2: Each in-range thread sets its bit in its digit's bitmask.
    if (in_range) {
        let word_idx = my_digit * 8u + tid / 32u;
        let bit = 1u << (tid % 32u);
        atomicOr(&digit_member_bits[word_idx], bit);
    }
    workgroupBarrier();

    // Phase 3: Compute stable local rank via popcount of lower-TID bits.
    // For each thread, count how many threads with lower TID share the same
    // digit. This is equivalent to the serial scan but runs in O(8) per thread
    // instead of O(tid).
    var local_rank = 0u;
    let base = my_digit * 8u;
    let full_words = tid / 32u;
    for (var w = 0u; w < full_words; w = w + 1u) {
        local_rank = local_rank + countOneBits(atomicLoad(&digit_member_bits[base + w]));
    }
    let partial_word = atomicLoad(&digit_member_bits[base + full_words]);
    let lower_mask = (1u << (tid % 32u)) - 1u;
    local_rank = local_rank + countOneBits(partial_word & lower_mask);

    // Look up global offset from prefix-summed histogram.
    // histograms[digit * num_sort_wg + wg_id] is the exclusive prefix sum
    // = global start offset for elements of this (digit, workgroup).
    if (in_range) {
        let global_offset = histograms[my_digit * sort_config.num_sort_wg + wg_id];
        let out_idx = global_offset + local_rank;
        write_key(out_idx, my_key);
        write_val(out_idx, my_val);
    }
}

// ---- Entry point 5: Reorder instances using sorted indices ----
//
// After 4 passes (even count), final sorted indices are in vals_a.

@compute @workgroup_size(256)
fn reorder_instances(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let idx = global_id.x;
    let visible_count = draw_indirect.instance_count;

    if (idx >= visible_count) {
        return;
    }

    let src_idx = vals_a[idx];
    instances_dst[idx] = instances_src[src_idx];
}
