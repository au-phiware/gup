// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GPU compute shader for parallel histogram generation
// Efficiently bins data into histogram buckets using atomic operations

struct HistogramConfig {
    bin_count: u32,
    min_value: f32,
    max_value: f32,
    normalize: u32,  // 0 = counts, 1 = probability
    data_length: u32,
    _padding: u32,
    _padding2: u32,
    _padding3: u32,
}

@group(0) @binding(0) var<storage, read> data: array<f32>;
@group(0) @binding(1) var<storage, read_write> bins: array<atomic<u32>>;
@group(0) @binding(2) var<uniform> config: HistogramConfig;

// Shared memory for workgroup-local histograms (reduces atomic contention)
var<workgroup> local_bins: array<atomic<u32>, 256>;

// Workgroup size of 256 for maximum GPU compatibility
@compute @workgroup_size(256)
fn compute_histogram(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) workgroup_id: vec3<u32>
) {
    let thread_id = local_id.x;
    let global_index = global_id.x;
    
    // Initialize all local bins to zero (entire workgroup shared memory)
    atomicStore(&local_bins[thread_id], 0u);
    workgroupBarrier();
    
    // Compute which bin this value belongs to and increment local histogram
    if (global_index < config.data_length) {
        let value = data[global_index];
        
        // Compute bin index
        let range = config.max_value - config.min_value;
        // Avoid floating point precision issues by using careful calculation
        // For values at exact bin boundaries, this ensures consistent binning
        let bin_float = ((value - config.min_value) / range) * f32(config.bin_count);
        var bin_index = u32(floor(bin_float));
        
        // Clamp to valid range [0, bin_count-1]
        bin_index = min(bin_index, config.bin_count - 1u);
        
        // Increment local histogram (less contention)
        atomicAdd(&local_bins[bin_index], 1u);
    }
    workgroupBarrier();
    
    // Merge local histograms into global histogram
    if (thread_id < config.bin_count) {
        let local_count = atomicLoad(&local_bins[thread_id]);
        if (local_count > 0u) {
            atomicAdd(&bins[thread_id], local_count);
        }
    }
}

// Normalization pass - convert counts to probabilities
@compute @workgroup_size(256)
fn normalize_histogram(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let bin_index = global_id.x;
    
    if (bin_index >= config.bin_count) {
        return;
    }
    
    // Sum all bins (could be optimized with parallel reduction)
    var total: u32 = 0u;
    for (var i: u32 = 0u; i < config.bin_count; i = i + 1u) {
        total = total + atomicLoad(&bins[i]);
    }
    
    // Normalize this bin
    if (total > 0u) {
        let count = atomicLoad(&bins[bin_index]);
        let probability = f32(count) / f32(total);
        // Store normalized value (reinterpreting u32 as f32 bits)
        atomicStore(&bins[bin_index], bitcast<u32>(probability));
    }
}
