// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GPU compute shader for parallel statistical aggregations
// Computes mean, min, max, variance, and standard deviation efficiently

struct StatisticsResult {
    count: u32,
    sum: f32,
    min: f32,
    max: f32,
    mean: f32,
    variance: f32,
    std_dev: f32,
    _padding: u32,
}

@group(0) @binding(0) var<storage, read> data: array<f32>;
@group(0) @binding(1) var<storage, read_write> result: StatisticsResult;

// Shared memory for local reduction within workgroup
var<workgroup> shared_sum: array<f32, 256>;
var<workgroup> shared_min: array<f32, 256>;
var<workgroup> shared_max: array<f32, 256>;
var<workgroup> shared_count: array<u32, 256>;

// Workgroup size of 256 for maximum GPU compatibility
@compute @workgroup_size(256)
fn compute_basic_stats(@builtin(global_invocation_id) global_id: vec3<u32>, @builtin(local_invocation_id) local_id: vec3<u32>) {
    let thread_id = local_id.x;
    let global_index = global_id.x;
    let data_size = arrayLength(&data);

    // Initialize shared memory for this thread
    if (global_index < data_size) {
        let value = data[global_index];
        shared_sum[thread_id] = value;
        shared_min[thread_id] = value;
        shared_max[thread_id] = value;
        shared_count[thread_id] = 1u;
    } else {
        shared_sum[thread_id] = 0.0;
        shared_min[thread_id] = 3.40282e+38; // f32::MAX
        shared_max[thread_id] = -3.40282e+38; // f32::MIN
        shared_count[thread_id] = 0u;
    }
    
    workgroupBarrier();

    // Parallel reduction within workgroup
    for (var stride: u32 = 128u; stride > 0u; stride = stride / 2u) {
        if (thread_id < stride) {
            let other_id = thread_id + stride;
            shared_sum[thread_id] += shared_sum[other_id];
            shared_min[thread_id] = min(shared_min[thread_id], shared_min[other_id]);
            shared_max[thread_id] = max(shared_max[thread_id], shared_max[other_id]);
            shared_count[thread_id] += shared_count[other_id];
        }
        workgroupBarrier();
    }

    // First thread in workgroup writes results
    if (thread_id == 0u) {
        let count = shared_count[0];
        if (count > 0u) {
            let sum = shared_sum[0];
            let min_val = shared_min[0];
            let max_val = shared_max[0];
            let mean = sum / f32(count);

            // Atomic updates to global result (first workgroup initializes, others accumulate)
            atomicAdd(&result.count, count);
            atomicAdd(&result.sum, sum);
            atomicMin(&result.min, bitcast<u32>(min_val));
            atomicMax(&result.max, bitcast<u32>(max_val));

            // Note: Variance/std_dev computation requires two-pass algorithm
            // This simplified version computes only first-pass statistics
            // A second pass would compute variance using the mean
        }
    }

    // After all workgroups complete, compute final mean
    // (This would typically be done in a separate dispatch or on CPU)
    workgroupBarrier();
    if (global_id.x == 0u && result.count > 0u) {
        result.mean = result.sum / f32(result.count);
    }
}

// Second pass for variance calculation (requires mean from first pass)
@compute @workgroup_size(256)
fn compute_variance(@builtin(global_invocation_id) global_id: vec3<u32>, @builtin(local_invocation_id) local_id: vec3<u32>) {
    let thread_id = local_id.x;
    let global_index = global_id.x;
    let data_size = arrayLength(&data);

    var local_squared_diff: f32 = 0.0;

    if (global_index < data_size) {
        let value = data[global_index];
        let diff = value - result.mean;
        local_squared_diff = diff * diff;
    }

    // Store in shared memory for reduction
    shared_sum[thread_id] = local_squared_diff;
    workgroupBarrier();

    // Parallel reduction
    for (var stride: u32 = 128u; stride > 0u; stride = stride / 2u) {
        if (thread_id < stride) {
            shared_sum[thread_id] += shared_sum[thread_id + stride];
        }
        workgroupBarrier();
    }

    // First thread writes result
    if (thread_id == 0u && shared_sum[0] > 0.0) {
        let variance_contribution = shared_sum[0];
        atomicAdd(&result.variance, variance_contribution);
    }

    // Compute standard deviation from variance
    workgroupBarrier();
    if (global_id.x == 0u && result.count > 0u) {
        result.variance = result.variance / f32(result.count);
        result.std_dev = sqrt(result.variance);
    }
}
