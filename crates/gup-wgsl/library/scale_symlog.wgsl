// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Symmetric log position scale over an absolute column:
// f(x) = sign(x) * log2(1 + |x| / c). Linear through zero (f ≈ x / (c ln 2)
// for |x| much less than c), logarithmic beyond the constant c, continuous
// and smooth everywhere, so signed data has no gap or jump at the threshold.
#define_import_path gup::scale::symlog

struct Params {
    // 1 / c.
    inv_c: f32,
    // f(d0), the transformed domain start.
    lo: f32,
    // Pixels per transformed unit: (r1 - r0) / (f(d1) - f(d0)).
    k: f32,
    // Range start r0 (px).
    range_start: f32,
}

// log2(1 + u) for u >= 0. Near zero a series: 1 + u loses u's low bits in
// f32, and log2 near 1 is only accurate to an absolute 2^-21 on some GPUs.
fn log2_1p(u: f32) -> f32 {
    if (u < 0.0625) {
        // u - u^2/2 + u^3/3 - u^4/4 + u^5/5, over ln 2.
        let s = u * (1.0 - u * (0.5 - u * (0.33333334 - u * (0.25 - u * 0.2))));
        return s * 1.442695;
    }
    return log2(1.0 + u);
}

fn map(v: f32, p: Params) -> f32 {
    let t = sign(v) * log2_1p(abs(v) * p.inv_c);
    return p.range_start + (t - p.lo) * p.k;
}
