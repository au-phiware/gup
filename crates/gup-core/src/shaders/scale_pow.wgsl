// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Power position scale (square root at exponent 0.5) over an absolute
// column. The transform keeps the sign, f(x) = sign(x) * |x|^e, so domains
// that cross zero map monotonically; the exponent is positive.
#define_import_path gup::scale::pow

struct Params {
    // The exponent e (> 0).
    exponent: f32,
    // f(d0), the transformed domain start.
    lo: f32,
    // Pixels per transformed unit: (r1 - r0) / (f(d1) - f(d0)).
    k: f32,
    // Range start r0 (px).
    range_start: f32,
}

fn map(v: f32, p: Params) -> f32 {
    // pow(0, e) is exp2(e * log2(0)): keep zero exact rather than trust it.
    let t = select(sign(v) * pow(abs(v), p.exponent), 0.0, v == 0.0);
    return p.range_start + (t - p.lo) * p.k;
}
