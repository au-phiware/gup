// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Softplus, ln(1 + exp(k v)) / k: a smooth ramp from 0, for
// `#[wgsl_function(file = "…")]` (GUP-420). It borrows the symmetric log
// scale's accurate log2(1 + u) from Gup's library.
#import gup::scale::symlog as symlog

struct Params {
    k: f32,
}

fn softplus(v: f32, p: Params) -> f32 {
    return symlog::log2_1p(exp(p.k * v)) * 0.6931472 / p.k;
}
