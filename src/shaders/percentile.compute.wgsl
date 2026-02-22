// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GPU compute shader for percentile and quantile calculation
// Uses parallel sorting and selection algorithms

struct PercentileQuery {
    percentile: f32,  // 0.0 to 1.0
    result: f32,      // Output value
    _padding: vec2<f32>,
}

@group(0) @binding(0) var<storage, read> data: array<f32>;
@group(0) @binding(1) var<storage, read_write> sorted_data: array<f32>;
@group(0) @binding(2) var<storage, read_write> query: PercentileQuery;

// Shared memory for local sorting
var<workgroup> shared_values: array<f32, 256>;

// Bitonic sort implementation for parallel sorting
@compute @workgroup_size(256)
fn bitonic_sort_step(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>
) {
    let thread_id = local_id.x;
    let global_index = global_id.x;
    let data_size = arrayLength(&data);

    // Load data into shared memory
    if (global_index < data_size) {
        shared_values[thread_id] = data[global_index];
    } else {
        shared_values[thread_id] = 3.40282e+38; // f32::MAX for padding
    }
    workgroupBarrier();

    // Bitonic sort iterations
    // This is a simplified version - full implementation would need multiple dispatches
    for (var k: u32 = 2u; k <= 256u; k = k * 2u) {
        for (var j: u32 = k / 2u; j > 0u; j = j / 2u) {
            let ixj = thread_id ^ j;
            if (ixj > thread_id) {
                let should_swap = ((thread_id & k) == 0u) == (shared_values[thread_id] > shared_values[ixj]);
                if (should_swap) {
                    let temp = shared_values[thread_id];
                    shared_values[thread_id] = shared_values[ixj];
                    shared_values[ixj] = temp;
                }
            }
            workgroupBarrier();
        }
    }

    // Write sorted results back
    if (global_index < data_size) {
        sorted_data[global_index] = shared_values[thread_id];
    }
}

// Compute percentile from sorted data
@compute @workgroup_size(1)
fn compute_percentile(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let data_size = arrayLength(&sorted_data);
    if (data_size == 0u) {
        query.result = 0.0;
        return;
    }

    let percentile = clamp(query.percentile, 0.0, 1.0);
    let index = u32(percentile * f32(data_size - 1u));
    query.result = sorted_data[index];
}

// Optimized median calculation (50th percentile)
@compute @workgroup_size(1)
fn compute_median(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let data_size = arrayLength(&sorted_data);
    if (data_size == 0u) {
        query.result = 0.0;
        return;
    }

    if (data_size % 2u == 1u) {
        // Odd number of elements - return middle element
        query.result = sorted_data[data_size / 2u];
    } else {
        // Even number - return average of two middle elements
        let mid1 = sorted_data[(data_size / 2u) - 1u];
        let mid2 = sorted_data[data_size / 2u];
        query.result = (mid1 + mid2) / 2.0;
    }
}

// Compute multiple quantiles efficiently
struct QuantileResult {
    q25: f32,  // 25th percentile
    q50: f32,  // 50th percentile (median)
    q75: f32,  // 75th percentile
    _padding: f32,
}

@group(0) @binding(3) var<storage, read_write> quantile_result: QuantileResult;

@compute @workgroup_size(1)
fn compute_quantiles(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let data_size = arrayLength(&sorted_data);
    if (data_size == 0u) {
        quantile_result.q25 = 0.0;
        quantile_result.q50 = 0.0;
        quantile_result.q75 = 0.0;
        return;
    }

    // Compute indices for each quantile
    let idx_25 = u32(0.25 * f32(data_size - 1u));
    let idx_50 = data_size / 2u;
    let idx_75 = u32(0.75 * f32(data_size - 1u));

    quantile_result.q25 = sorted_data[idx_25];
    quantile_result.q50 = sorted_data[idx_50];
    quantile_result.q75 = sorted_data[idx_75];
}
