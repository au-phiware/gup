// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The slice of OpenType layout `gup-text` needs, read straight from a
//! font's GSUB and GPOS tables: GPOS `kern` pair adjustments and GSUB `tnum`
//! single substitutions. It is checked against a full shaping engine on the
//! bundled faces (`tests/shaping_oracle.rs`) and costs a few KB, where the
//! engine costs hundreds.

use std::collections::HashMap;
use std::sync::Mutex;
use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
use ttf_parser::gsub::{SingleSubstitution, SubstitutionSubtable};
use ttf_parser::opentype_layout::Coverage;
use ttf_parser::{Face, GlyphId, Tag};

/// The pair cache is cleared before it passes this many entries.
const MAX_CACHED_PAIRS: usize = 65_536;

/// A face's `kern` and `tnum` data, read once.
pub(crate) struct OtLayout {
    units_per_em: f32,
    /// GSUB `tnum` single substitutions, flattened: glyph to tabular glyph.
    tnum: HashMap<u16, u16>,
    /// Lookup indices of every GPOS feature tagged `kern`, deduplicated,
    /// ascending.
    kern_lookups: Vec<u16>,
    /// Pair results so far, in font units (0 = the pair does not kern).
    pairs: Mutex<HashMap<(u16, u16), i16>>,
}

impl OtLayout {
    pub(crate) fn parse(face: &Face<'_>) -> Self {
        let mut tnum = HashMap::new();
        if let Some(gsub) = face.tables().gsub {
            let wanted = Tag::from_bytes(b"tnum");
            for feature in gsub.features {
                if feature.tag != wanted {
                    continue;
                }
                for index in feature.lookup_indices {
                    let Some(lookup) = gsub.lookups.get(index) else {
                        continue;
                    };
                    for subtable in lookup.subtables.into_iter::<SubstitutionSubtable>() {
                        if let SubstitutionSubtable::Single(single) = subtable {
                            collect_single(single, &mut tnum);
                        }
                    }
                }
            }
        }
        let mut kern_lookups = Vec::new();
        if let Some(gpos) = face.tables().gpos {
            let wanted = Tag::from_bytes(b"kern");
            for feature in gpos.features {
                if feature.tag == wanted {
                    kern_lookups.extend(feature.lookup_indices);
                }
            }
            kern_lookups.sort_unstable();
            kern_lookups.dedup();
        }
        Self {
            units_per_em: f32::from(face.units_per_em()),
            tnum,
            kern_lookups,
            pairs: Mutex::new(HashMap::new()),
        }
    }

    /// The tabular form of `glyph`, or `glyph` itself when it has none.
    pub(crate) fn tabular(&self, glyph: u16) -> u16 {
        self.tnum.get(&glyph).copied().unwrap_or(glyph)
    }

    /// Whether the face has GPOS `kern` lookups (otherwise its legacy `kern`
    /// table, if any, applies).
    pub(crate) fn has_gpos_kern(&self) -> bool {
        !self.kern_lookups.is_empty()
    }

    /// Kerning between `a` and `b` in pixels at `px`. `data` is the font
    /// file; it is parsed again on a cache miss.
    pub(crate) fn kern(&self, data: &[u8], a: u16, b: u16, px: f32) -> f32 {
        let cached = {
            let pairs = self.pairs.lock().unwrap_or_else(|e| e.into_inner());
            pairs.get(&(a, b)).copied()
        };
        let units = cached.unwrap_or_else(|| {
            let units = Face::parse(data, 0)
                .map(|face| self.pair_units(&face, a, b))
                .unwrap_or(0);
            let mut pairs = self.pairs.lock().unwrap_or_else(|e| e.into_inner());
            if pairs.len() >= MAX_CACHED_PAIRS {
                pairs.clear();
            }
            pairs.insert((a, b), units);
            units
        });
        f32::from(units) * px / self.units_per_em
    }

    /// The summed adjustment of the `kern` lookups, in font units: each
    /// lookup contributes the first pair record that matches, and the lookups
    /// add up in index order.
    fn pair_units(&self, face: &Face<'_>, a: u16, b: u16) -> i16 {
        let Some(gpos) = face.tables().gpos else {
            return 0;
        };
        let (a, b) = (GlyphId(a), GlyphId(b));
        let mut total = 0i32;
        for &index in &self.kern_lookups {
            let Some(lookup) = gpos.lookups.get(index) else {
                continue;
            };
            for subtable in lookup.subtables.into_iter::<PositioningSubtable>() {
                let PositioningSubtable::Pair(pair) = subtable else {
                    continue;
                };
                let Some(ci) = pair.coverage().get(a) else {
                    continue;
                };
                let value = match pair {
                    PairAdjustment::Format1 { sets, .. } => sets
                        .get(ci)
                        .and_then(|set| set.get(b))
                        .map(|(first, _)| first),
                    PairAdjustment::Format2 {
                        classes, matrix, ..
                    } => matrix
                        .get((classes.0.get(a), classes.1.get(b)))
                        .map(|(first, _)| first),
                };
                if let Some(first) = value {
                    total += i32::from(first.x_advance);
                    break;
                }
            }
        }
        total.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
    }
}

/// Add the glyph pairs of one GSUB single substitution to `map`.
fn collect_single(single: SingleSubstitution<'_>, map: &mut HashMap<u16, u16>) {
    let (coverage, delta, substitutes) = match single {
        SingleSubstitution::Format1 { coverage, delta } => (coverage, delta, None),
        SingleSubstitution::Format2 {
            coverage,
            substitutes,
        } => (coverage, 0, Some(substitutes)),
    };
    let mut add = |glyph: u16, index: u16| {
        let to = match &substitutes {
            None => Some((i32::from(glyph) + i32::from(delta)) as u16),
            Some(list) => list.get(index).map(|g| g.0),
        };
        if let Some(to) = to {
            map.entry(glyph).or_insert(to);
        }
    };
    match coverage {
        Coverage::Format1 { glyphs } => {
            for (index, glyph) in glyphs.into_iter().enumerate() {
                add(glyph.0, index as u16);
            }
        }
        Coverage::Format2 { records } => {
            for record in records {
                for glyph in record.start.0..=record.end.0 {
                    add(glyph, record.value + (glyph - record.start.0));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::INTER_REGULAR;

    fn glyph(face: &Face<'_>, c: char) -> u16 {
        face.glyph_index(c).expect("in the subset").0
    }

    #[test]
    fn the_regular_subset_has_tnum_and_kern() {
        let face = Face::parse(INTER_REGULAR, 0).unwrap();
        let ot = OtLayout::parse(&face);
        for c in '0'..='9' {
            let d = glyph(&face, c);
            assert_ne!(ot.tabular(d), d, "{c:?} has a tabular form");
        }
        assert_eq!(ot.tabular(glyph(&face, 'A')), glyph(&face, 'A'));
        assert_eq!(ot.kern_lookups, [0, 1]);
        assert!(ot.has_gpos_kern());
        let av = ot.kern(INTER_REGULAR, glyph(&face, 'A'), glyph(&face, 'V'), 16.0);
        assert!(av < 0.0, "{av}");
        let space = ot.kern(INTER_REGULAR, glyph(&face, ' '), glyph(&face, ' '), 16.0);
        assert_eq!(space, 0.0);
    }
}
