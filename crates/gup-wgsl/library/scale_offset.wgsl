// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A pixel offset added to the previous link's output (RFC-001 S10b): the
// slot of a grouped bar within its band. The offset is read on the CPU each
// time the plot resolves.
#define_import_path gup::scale::offset

struct Params {
    offset: f32,
    // Uniform `Params` structs span a multiple of 16 bytes (GUP-401).
    pad_a: u32,
    pad_b: u32,
    pad_c: u32,
}

fn map(v: f32, p: Params) -> f32 {
    return v + p.offset;
}
