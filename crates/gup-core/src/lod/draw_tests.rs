// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-715: a forced level-of-detail tier draws each chunk's
//! representatives through the indexed-rows glue, and the GPU pick, the
//! brushed bits and the validity bits read the data rows they stand for.

use crate::geom::Point;
use crate::prelude::*;
use crate::selection::Layer as _;
use crate::{Context, RowId};
use gup_visual_regression::Rgba8;
use std::collections::BTreeSet;

const W: u32 = 480;
const H: u32 = 360;
const TIER: u32 = 4;
const RADIUS: f32 = 3.0;

#[derive(Clone, Debug)]
struct P {
    x: f64,
    y: f64,
}

/// `n` points spread over `0..10` in both axes, deterministically.
fn points(n: usize, seed: u64) -> Vec<P> {
    let mut state = seed;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    (0..n)
        .map(|_| P {
            x: next() * 10.0,
            y: next() * 10.0,
        })
        .collect()
}

/// A plot of `rows` as circles over fixed domains, in chunks of 2,048
/// rows, drawing tier `tier` (or every row).
fn scatter(rows: Vec<P>, tier: Option<u32>) -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>) {
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, 10.0));
    let y = plot.y(Linear::new().domain(0.0, 10.0));
    let mut sel = Selection::<P, Circle>::new(rows);
    sel.attr(Circle::X, x.encode(|r: &P| r.x))
        .attr(Circle::Y, y.encode(|r: &P| r.y))
        .attr(Circle::RADIUS, Px(RADIUS))
        .max_chunk_rows(2048)
        .force_lod_tier(tier);
    plot.add(sel);
    (plot, x, y)
}

/// The data rows of every chunk's tier-`TIER` prefix, from the CPU's
/// tiers.
fn representatives(plot: &mut Plot) -> Vec<usize> {
    let store = plot.layer_mut::<P, Circle>().unwrap().store().unwrap();
    let mut reps = Vec::new();
    for chunk in store.chunks() {
        let tiers = chunk.tiers().expect("the forced tier built the tiers");
        let n = tiers.prefix(TIER).expect("tier 4 is stored") as usize;
        let base = chunk.row_base() as usize;
        reps.extend(tiers.order[..n].iter().map(|&r| base + r as usize));
    }
    reps
}

fn pixel(x: &ScaleRef<Linear>, y: &ScaleRef<Linear>, p: &P) -> Point {
    Point::new(x.read().eval(p.x) as f32, y.read().eval(p.y) as f32)
}

/// The representatives no other representative's pick area reaches.
fn isolated(centres: &[(usize, Point)]) -> Vec<(usize, Point)> {
    let reach = 2.0 * (RADIUS + crate::pick::PICK_TOLERANCE.0) + 1.0;
    centres
        .iter()
        .filter(|(r, p)| {
            centres
                .iter()
                .all(|(q, c)| q == r || (c.x - p.x).hypot(c.y - p.y) > reach)
        })
        .copied()
        .collect()
}

fn colour(image: &image::RgbaImage, at: Point) -> Rgba8 {
    let [r, g, b, a] = image.get_pixel(at.x.round() as u32, at.y.round() as u32).0;
    Rgba8::new(r, g, b, a)
}

/// AC3.1: every GPU pick under the forced tier returns a representative,
/// and a representative's own pixel returns its data row; a row that is
/// not a representative is not drawn.
#[test]
fn picks_return_the_representatives_data_rows() {
    let cx = Context::new_blocking().unwrap();
    let rows = points(5_000, 0x2545_f491_4f6c_dd1d);
    let (mut plot, x, y) = scatter(rows.clone(), Some(TIER));
    plot.resolve(&cx, W as f32, H as f32).unwrap();
    let reps = representatives(&mut plot);
    let gpu = &plot.layer_mut::<P, Circle>().unwrap().glue_source();
    assert!(!gpu.indexed, "the layer's own glue stays plain");
    assert!(reps.len() < 3 * 256, "{} representatives", reps.len());
    assert!(reps.len() > 200, "{} representatives", reps.len());
    let rep_set: BTreeSet<usize> = reps.iter().copied().collect();
    let centres: Vec<(usize, Point)> = reps.iter().map(|&r| (r, pixel(&x, &y, &rows[r]))).collect();
    let alone = isolated(&centres);
    assert!(alone.len() > 50, "{} isolated representatives", alone.len());
    for &(_, at) in &centres {
        let got = plot
            .pick_row_gpu(&cx, at)
            .unwrap()
            .expect("a circle is there");
        assert_eq!(got.layer, 0);
        assert!(
            rep_set.contains(&got.row),
            "row {} picked at {at:?}",
            got.row
        );
    }
    for &(r, at) in &alone {
        assert_eq!(
            plot.pick_row_gpu(&cx, at).unwrap(),
            Some(RowId { layer: 0, row: r }),
            "representative {r} at {at:?}"
        );
    }
    // Rows away from every representative are not drawn.
    let reach = 2.0 * (RADIUS + crate::pick::PICK_TOLERANCE.0) + 1.0;
    let mut hidden = 0;
    for (r, p) in rows.iter().enumerate() {
        let at = pixel(&x, &y, p);
        if rep_set.contains(&r)
            || centres
                .iter()
                .any(|(_, c)| (c.x - at.x).hypot(c.y - at.y) <= reach)
        {
            continue;
        }
        hidden += 1;
        if hidden <= 200 {
            assert_eq!(plot.pick_row_gpu(&cx, at).unwrap(), None, "row {r}");
        }
    }
    assert!(hidden > 200, "{hidden}");
    // Back to every row: the GPU state is rebuilt without the indexed
    // program, and the hidden rows draw again.
    plot.layer_mut::<P, Circle>().unwrap().force_lod_tier(None);
    plot.resolve(&cx, W as f32, H as f32).unwrap();
    for (r, p) in rows
        .iter()
        .enumerate()
        .filter(|(r, _)| !rep_set.contains(r))
        .take(50)
    {
        let at = pixel(&x, &y, p);
        assert!(plot.pick_row_gpu(&cx, at).unwrap().is_some(), "row {r}");
    }

    // The forced-tier image beside the full draw, for reading by eye.
    if let Some(dir) = std::env::var_os("GUP_LOD_PNG_DIR") {
        let dir = std::path::Path::new(&dir);
        plot.render(&cx, W, H)
            .unwrap()
            .save(dir.join("lod-tier4-5000.png"))
            .unwrap();
        let (mut full, _, _) = scatter(rows, None);
        full.render(&cx, W, H)
            .unwrap()
            .save(dir.join("lod-full-5000.png"))
            .unwrap();
    }
}

/// AC3.2: the brushed bit is read by data row: a brushed representative
/// draws at full alpha, an unbrushed one dimmed.
#[test]
fn the_brush_dims_representatives_by_data_row() {
    let cx = Context::new_blocking().unwrap();
    let rows = points(5_000, 0x9e37_79b9_7f4a_7c15);
    let (mut plot, x, y) = scatter(rows.clone(), Some(TIER));
    plot.drag(Drag::Brush);
    let before = plot.render(&cx, W, H).unwrap();
    let reps = representatives(&mut plot);
    let rep_set: BTreeSet<usize> = reps.iter().copied().collect();
    let centres: Vec<(usize, Point)> = reps.iter().map(|&r| (r, pixel(&x, &y, &rows[r]))).collect();
    let alone = isolated(&centres);
    // Brush every other isolated representative and every row that is not
    // a representative (whose bits no drawn instance reads).
    let brushed: Vec<_> = alone.iter().step_by(2).copied().collect();
    let unbrushed: Vec<_> = alone.iter().skip(1).step_by(2).copied().collect();
    let mut set: Vec<RowId> = (0..rows.len())
        .filter(|r| !rep_set.contains(r))
        .chain(brushed.iter().map(|&(r, _)| r))
        .map(|row| RowId { layer: 0, row })
        .collect();
    set.sort_by_key(|r| r.row);
    plot.set_brush_rows(set);
    let after = plot.render(&cx, W, H).unwrap();
    assert!(brushed.len() > 20 && unbrushed.len() > 20);
    for &(r, at) in &brushed {
        let (a, b) = (colour(&before, at), colour(&after, at));
        assert!(a.delta_e(b) < 1.0, "brushed {r} at {at:?}: {a:?} → {b:?}");
    }
    for &(r, at) in &unbrushed {
        let (a, b) = (colour(&before, at), colour(&after, at));
        assert!(
            a.delta_e(b) >= 10.0,
            "unbrushed {r} at {at:?}: {a:?} → {b:?} (ΔE00 {})",
            a.delta_e(b)
        );
    }
}

/// AC3.3: a row with a null y is never a representative, and a
/// representative's validity bit is its data row's: nulls at instance
/// indices that representatives occupy blank none of them.
#[test]
fn validity_is_read_by_data_row() {
    let cx = Context::new_blocking().unwrap();
    let mut rows = points(5_000, 0x1234_5678_9abc_def1);
    // Nulls among the first rows of every chunk: under a bug that read the
    // bits at the instance index, the first representatives would vanish.
    let nulls: BTreeSet<usize> = (0..3)
        .flat_map(|k| (1..120).step_by(2).map(move |r| k * 2048 + r))
        .collect();
    for &r in &nulls {
        rows[r].y = f64::NAN;
    }
    let (mut plot, x, y) = scatter(rows.clone(), Some(TIER));
    plot.resolve(&cx, W as f32, H as f32).unwrap();
    assert!(
        plot.layer_mut::<P, Circle>()
            .unwrap()
            .glue_source()
            .validity,
        "the layer reads validity bits"
    );
    let reps = representatives(&mut plot);
    assert!(
        reps.iter().all(|r| !nulls.contains(r)),
        "a null represents nothing"
    );
    let centres: Vec<(usize, Point)> = reps.iter().map(|&r| (r, pixel(&x, &y, &rows[r]))).collect();
    let alone = isolated(&centres);
    // Some isolated representatives sit at an instance index that is a
    // null row's chunk-local index.
    let store = plot.layer_mut::<P, Circle>().unwrap().store().unwrap();
    let at_null_index = alone
        .iter()
        .filter(|(r, _)| {
            let k = r / 2048;
            let tiers = store.chunks()[k].tiers().unwrap();
            let i = tiers
                .order
                .iter()
                .position(|&o| o as usize == r % 2048)
                .unwrap();
            nulls.contains(&(k * 2048 + i))
        })
        .count();
    assert!(at_null_index > 3, "{at_null_index}");
    for &(r, at) in &alone {
        assert_eq!(
            plot.pick_row_gpu(&cx, at).unwrap(),
            Some(RowId { layer: 0, row: r }),
            "representative {r} at {at:?}"
        );
    }
}
