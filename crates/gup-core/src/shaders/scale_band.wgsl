// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Ordinal position scales (RFC-001 S5b), band and point: dictionary code
// `code` maps to the centre of its slot, `offset + step * code`, both in
// pixels (computed in f64 on the CPU). The glue hides a row whose code is
// the reserved null code, so this function never sees one drawn.
#define_import_path gup::scale::band

struct Params {
    // The centre of code 0's slot.
    offset: f32,
    // The distance between adjacent slots (negative on a reversed range).
    step: f32,
    // Uniform `Params` structs span a multiple of 16 bytes (GUP-401).
    pad_a: u32,
    pad_b: u32,
}

fn map(code: u32, p: Params) -> f32 {
    return p.offset + p.step * f32(code);
}
