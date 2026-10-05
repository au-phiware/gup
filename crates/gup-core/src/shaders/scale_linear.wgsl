// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Linear position scale over a relative column.
//
// The column stores v = x - origin (per-chunk origin, f32). The CPU computes
// base = origin - d0 in f64 for each chunk, so the shader evaluates
// px = r0 + (x - d0) * k without ever forming the large absolute value x.
#define_import_path gup::scale::linear

struct Params {
    // Pixels per domain unit: (r1 - r0) / (d1 - d0).
    k: f32,
    // Range start r0 (px).
    range_start: f32,
}

fn map_rel(v: f32, base: f32, p: Params) -> f32 {
    return p.range_start + (v + base) * p.k;
}
