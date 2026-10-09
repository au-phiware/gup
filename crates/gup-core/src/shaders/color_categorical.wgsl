// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Categorical colour (RFC-001 S4b; a stand-in for S5's full scale): a
// dictionary code picks a palette entry, cycling past the palette's end, and
// the reserved null code picks the null colour.
#define_import_path gup::color::categorical

struct Params {
    // The palette, sRGB-encoded with straight alpha; the first `count`
    // entries are used.
    colors: array<vec4<f32>, 8>,
    // The colour of a missing key.
    null_color: vec4<f32>,
    count: u32,
    // Uniform `Params` structs span a multiple of 16 bytes (GUP-401).
    pad_a: u32,
    pad_b: u32,
    pad_c: u32,
}

// The code of a missing key (`gup_core::column::NULL_CODE`).
const NULL_CODE: u32 = 0xffffffffu;

fn map(code: u32, p: Params) -> vec4<f32> {
    // Indexed through a local copy: a run-time index into an array value
    // needs a variable on some backends.
    var colors = p.colors;
    let color = colors[code % p.count];
    return select(color, p.null_color, code == NULL_CODE);
}
