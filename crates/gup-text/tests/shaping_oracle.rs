// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `gup-text` lays text out with its own small walk over GPOS `kern` and
//! GSUB `tnum` (`src/opentype.rs`). This test holds it to a full shaping
//! engine: for both bundled faces, the width of every two-character string of
//! printable ASCII, and of a few real labels, equals the sum of the engine's
//! advances exactly.

use gup_text::{Features, Font, INTER_REGULAR, INTER_SEMIBOLD};

fn shaped_width(face: &rustybuzz::Face<'_>, text: &str, tnum: bool) -> f32 {
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(rustybuzz::Direction::LeftToRight);
    buffer.set_script(rustybuzz::script::LATIN);
    let features: Vec<rustybuzz::Feature> = if tnum {
        vec![rustybuzz::Feature::new(
            rustybuzz::ttf_parser::Tag::from_bytes(b"tnum"),
            1,
            ..,
        )]
    } else {
        Vec::new()
    };
    let glyphs = rustybuzz::shape(face, &features, buffer);
    glyphs
        .glyph_positions()
        .iter()
        .map(|p| p.x_advance as f32)
        .sum()
}

fn strings(font: &Font) -> Vec<String> {
    let ascii: Vec<char> = (0x20u8..=0x7E).map(char::from).collect();
    let mut all = Vec::new();
    for &a in &ascii {
        for &b in &ascii {
            all.push(format!("{a}{b}"));
        }
    }
    assert_eq!(all.len(), 9025);
    for s in [
        "\u{2212}42 \u{b0}C",
        "2024\u{2013}Q1",
        "Z\u{fc}rich",
        "Revenue (USD)",
        "AVAST Today",
    ] {
        // Characters the face lacks fall back to Regular, which the engine
        // (one face) does not do.
        if s.chars().all(|c| font.has_own_glyph(c)) {
            all.push(s.to_owned());
        }
    }
    all
}

fn check(name: &str, font: &Font, bytes: &[u8]) {
    let face = rustybuzz::Face::from_slice(bytes, 0).expect("a face");
    let upem = f32::from(face.units_per_em() as u16);
    let mut checked = 0;
    for text in strings(font) {
        for (features, tnum) in [
            (Features::KERN, false),
            (Features::KERN | Features::TNUM, true),
        ] {
            let ours = font.measure_with(&text, upem, features).width;
            let theirs = shaped_width(&face, &text, tnum);
            assert_eq!(ours, theirs, "{name} {text:?} tnum={tnum}");
            checked += 1;
        }
    }
    assert!(checked >= 2 * 9025, "{name}: {checked}");
}

#[test]
fn regular_widths_equal_the_shaping_engines() {
    check("Regular", &Font::inter(), INTER_REGULAR);
}

#[test]
fn semibold_widths_equal_the_shaping_engines() {
    check("SemiBold", &Font::inter_semibold(), INTER_SEMIBOLD);
}
