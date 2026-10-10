// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A categorical scatter (RFC-001 S4b): the golden scatter's countries,
//! coloured by continent through a dictionary column, with some continents
//! missing (drawn in the null colour). Domains are explicit, so plots of
//! subsets of the rows (some positions made null, or removed) map values
//! to the same pixels.

#![allow(dead_code)] // each includer uses a different subset

use super::scatter::countries_n;
use gup_core::ScaleRef;
use gup_core::prelude::*;

/// Image width.
pub const WIDTH: u32 = 720;
/// Image height.
pub const HEIGHT: u32 = 450;
/// Mark radius.
pub const RADIUS: f32 = 4.5;
/// The continents, in no particular order.
pub const CONTINENTS: [&str; 5] = ["Europe", "Asia", "Africa", "Americas", "Oceania"];

/// One country.
#[derive(Clone, Debug)]
pub struct Place {
    pub gdp_per_capita: f64,
    pub population: f64,
    pub continent: Option<String>,
}

/// 120 countries; every 13th (from row 6) has no continent.
pub fn places() -> Vec<Place> {
    countries_n(120)
        .into_iter()
        .enumerate()
        .map(|(i, c)| Place {
            gdp_per_capita: c.gdp_per_capita,
            population: c.population,
            continent: (i % 13 != 6).then(|| CONTINENTS[(i * 3 + i / 7) % 5].to_string()),
        })
        .collect()
}

/// The plot over `rows`, with its x and y scale handles.
pub fn plot(rows: Vec<Place>) -> (Plot, ScaleRef<Linear>, ScaleRef<Log>) {
    let mut plot = Plot::new();
    let (x, y) = (
        plot.x(Linear::new().domain(0.0, 60_000.0)),
        plot.y(Log::new().domain(1e5, 1e9)),
    );
    plot.title("Countries by continent")
        .add(Selection::<Place, Circle>::new(rows))
        .attr(Circle::X, x.encode(|p: &Place| p.gdp_per_capita))
        .attr(Circle::Y, y.encode(|p: &Place| p.population))
        .attr(
            Circle::FILL,
            Categorical::okabe_ito().encode_nullable_key(|p: &Place| p.continent.as_deref()),
        )
        .attr(Circle::RADIUS, Px(RADIUS));
    (plot, x, y)
}

/// Each row's dictionary code, computed independently of gup-core: keys
/// numbered in first-seen order, `None` for a missing key.
pub fn codes(rows: &[Place]) -> Vec<Option<u32>> {
    let mut seen: Vec<&str> = Vec::new();
    rows.iter()
        .map(|p| {
            let key = p.continent.as_deref()?;
            Some(match seen.iter().position(|k| *k == key) {
                Some(code) => code as u32,
                None => {
                    seen.push(key);
                    seen.len() as u32 - 1
                }
            })
        })
        .collect()
}
