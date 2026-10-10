// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Logarithmic position scale over an absolute column. Log is scale
// invariant, so absolute f32 keeps relative precision. The base only
// affects ticks; the mapping uses log2 ratios.
#define_import_path gup::scale::log

struct Params {
    // log2(d0), the domain start.
    log_lo: f32,
    // 1 / (log2(d1) - log2(d0)).
    inv_log_span: f32,
    // Range start r0 (px).
    range_start: f32,
    // r1 - r0 (px).
    r_span: f32,
}

fn map(v: f32, p: Params) -> f32 {
    return p.range_start + (log2(v) - p.log_lo) * p.inv_log_span * p.r_span;
}
