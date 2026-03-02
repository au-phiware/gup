// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GPU compute shader for selection-based alpha dimming.
//
// Reads an input instance buffer (viewed as array<f32>), a per-instance
// selection mask buffer, and a config uniform.  Copies every instance to
// an output buffer, multiplying specific alpha-channel floats by
// `dim_opacity` when the mask flag is 0 (unselected).
//
// Pipeline: single dispatch of `apply_dim` — one thread per instance.

struct DimConfig {
    // Total number of instances to process.
    instance_count: u32,
    // Number of f32 values per instance (sizeof(Instance) / 4).
    floats_per_instance: u32,
    // Opacity multiplier applied to unselected instances (e.g. 0.2).
    dim_opacity: f32,
    // How many entries in alpha_offsets are valid (0..8).
    num_alpha_offsets: u32,
    // Float indices of alpha channels within each instance struct.
    // Packed into two vec4<u32> (supports up to 8 offsets).
    alpha_offsets_0: vec4<u32>,
    alpha_offsets_1: vec4<u32>,
}

@group(0) @binding(0) var<storage, read> src_instances: array<f32>;
@group(0) @binding(1) var<storage, read_write> dst_instances: array<f32>;
@group(0) @binding(2) var<storage, read> mask: array<u32>;
@group(0) @binding(3) var<uniform> config: DimConfig;

fn get_alpha_offset(idx: u32) -> u32 {
    switch idx {
        case 0u: { return config.alpha_offsets_0.x; }
        case 1u: { return config.alpha_offsets_0.y; }
        case 2u: { return config.alpha_offsets_0.z; }
        case 3u: { return config.alpha_offsets_0.w; }
        case 4u: { return config.alpha_offsets_1.x; }
        case 5u: { return config.alpha_offsets_1.y; }
        case 6u: { return config.alpha_offsets_1.z; }
        case 7u: { return config.alpha_offsets_1.w; }
        default: { return 0u; }
    }
}

@compute @workgroup_size(256)
fn apply_dim(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let idx = global_id.x;
    if (idx >= config.instance_count) {
        return;
    }

    let base = idx * config.floats_per_instance;
    let is_selected = mask[idx];

    // Copy all floats from source to destination.
    for (var i = 0u; i < config.floats_per_instance; i++) {
        dst_instances[base + i] = src_instances[base + i];
    }

    // If unselected (mask == 0) and there is an active selection,
    // multiply alpha channels by dim_opacity.
    if (is_selected == 0u) {
        for (var j = 0u; j < config.num_alpha_offsets; j++) {
            let offset = get_alpha_offset(j);
            dst_instances[base + offset] = src_instances[base + offset] * config.dim_opacity;
        }
    }
}
