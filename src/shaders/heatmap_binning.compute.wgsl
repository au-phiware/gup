// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GPU compute shader for parallel 2D heatmap binning.
//
// Bins input data records into an x_bins × y_bins grid using atomic
// operations.  A single dispatch handles Count, Sum, Min, and Max
// aggregation modes simultaneously by maintaining four atomic buffers.
// The CPU reads back whichever buffer matches the requested aggregate
// and (for Mean) divides sum by count.

struct BinConfig {
    x_bins:      u32,
    y_bins:      u32,
    x_min:       f32,
    x_max:       f32,
    y_min:       f32,
    y_max:       f32,
    data_length: u32,
    _padding:    u32,
}

// Per-record input: (x, y, fill_value).
@group(0) @binding(0) var<storage, read> x_data: array<f32>;
@group(0) @binding(1) var<storage, read> y_data: array<f32>;
@group(0) @binding(2) var<storage, read> fill_data: array<f32>;
@group(0) @binding(3) var<uniform>       config: BinConfig;

// Output accumulators — one element per grid cell.
// count stored as u32; sum/min/max stored as bitcast<u32>(f32).
@group(0) @binding(4) var<storage, read_write> out_count: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> out_sum:   array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> out_min:   array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> out_max:   array<atomic<u32>>;

// Workgroup size 256 — standard for broad GPU compatibility.
@compute @workgroup_size(256)
fn bin_data(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= config.data_length) {
        return;
    }

    let x_val = x_data[idx];
    let y_val = y_data[idx];
    let fill  = fill_data[idx];

    // Compute bin indices, clamped to [0, bins-1].
    let x_range = config.x_max - config.x_min;
    let y_range = config.y_max - config.y_min;

    var xi: u32 = 0u;
    if (x_range > 0.0) {
        let xf = floor(((x_val - config.x_min) / x_range) * f32(config.x_bins));
        xi = clamp(u32(xf), 0u, config.x_bins - 1u);
    }

    var yi: u32 = 0u;
    if (y_range > 0.0) {
        let yf = floor(((y_val - config.y_min) / y_range) * f32(config.y_bins));
        yi = clamp(u32(yf), 0u, config.y_bins - 1u);
    }

    let cell = yi * config.x_bins + xi;

    // Count — simple atomic increment.
    atomicAdd(&out_count[cell], 1u);

    // Sum — atomic add on the bit-representation of f32.
    // WGSL lacks atomicAdd for floats, so we use a compare-and-swap loop.
    var sum_old = atomicLoad(&out_sum[cell]);
    loop {
        let sum_new = bitcast<u32>(bitcast<f32>(sum_old) + fill);
        let result  = atomicCompareExchangeWeak(&out_sum[cell], sum_old, sum_new);
        if (result.exchanged) {
            break;
        }
        sum_old = result.old_value;
    }

    // Min — CAS loop: keep the smaller value.
    var min_old = atomicLoad(&out_min[cell]);
    loop {
        let old_f = bitcast<f32>(min_old);
        if (fill >= old_f) {
            break; // current min is already <= fill
        }
        let result = atomicCompareExchangeWeak(&out_min[cell], min_old, bitcast<u32>(fill));
        if (result.exchanged) {
            break;
        }
        min_old = result.old_value;
    }

    // Max — CAS loop: keep the larger value.
    var max_old = atomicLoad(&out_max[cell]);
    loop {
        let old_f = bitcast<f32>(max_old);
        if (fill <= old_f) {
            break; // current max is already >= fill
        }
        let result = atomicCompareExchangeWeak(&out_max[cell], max_old, bitcast<u32>(fill));
        if (result.exchanged) {
            break;
        }
        max_old = result.old_value;
    }
}
