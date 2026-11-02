// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A ranking as horizontal bars, longest first (illustrative data).

use gup::builder::{BarChart, Horizontal};
use gup::prelude::*;

pub struct Language {
    name: &'static str,
    speakers: f64,
}

/// Ten languages, sorted so that the largest is at the top.
pub fn chart() -> BarChart<Language, Horizontal> {
    let mut languages = [
        ("English", 1_450.0),
        ("Mandarin", 1_140.0),
        ("Hindi", 610.0),
        ("Spanish", 560.0),
        ("French", 320.0),
        ("Arabic", 280.0),
        ("Bengali", 270.0),
        ("Portuguese", 260.0),
        ("Russian", 255.0),
        ("Urdu", 230.0),
    ]
    .map(|(name, speakers)| Language { name, speakers });
    languages.sort_by(|a, b| a.speakers.total_cmp(&b.speakers));

    gup::barh(languages)
        .y(|l| l.name)
        .x(|l| l.speakers)
        .value_labels()
        .title("Most spoken languages")
        .subtitle("Total speakers, millions (illustrative data)")
        .x_title("Speakers (millions)")
        .grid(Axes::X)
}

fn main() -> gup::Result<()> {
    gup::show(chart())
}
