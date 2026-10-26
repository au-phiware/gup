// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-546: hover, tooltips and brushing on areas. The CPU pick and the GPU
//! pick (the layer's rows released) agree; a tooltip shows the row under the
//! pointer; a brush selects rows by their top point.

use gup_core::geom::Point as Pt;
use gup_core::prelude::*;
use gup_core::scene::{ItemKind, Z_TOOLTIP};
use gup_core::{Button, Input, RowId};

const W: f32 = 480.0;
const H: f32 = 320.0;

/// A row: x, top, baseline (data units over 0..10).
type Row = (f64, f64, f64);

fn base() -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>) {
    let mut plot = Plot::new();
    plot.legends(false);
    let x = plot.x(Linear::new().domain(0.0, 10.0));
    let y = plot.y(Linear::new().domain(0.0, 10.0));
    (plot, x, y)
}

fn area<'a>(
    plot: &'a mut Plot,
    x: &ScaleRef<Linear>,
    y: &ScaleRef<Linear>,
    rows: Vec<Row>,
) -> &'a mut Selection<Row, Area> {
    let sel = plot.area(rows);
    sel.attr(Area::X, x.encode(|r: &Row| r.0))
        .attr(Area::Y, y.encode(|r: &Row| r.1))
        .attr(Area::Y_END, y.encode(|r: &Row| r.2));
    sel
}

fn px(x: &ScaleRef<Linear>, y: &ScaleRef<Linear>, a: f64, b: f64) -> Pt {
    Pt::new(x.read().eval(a) as f32, y.read().eval(b) as f32)
}

fn id(layer: usize, row: usize) -> Option<RowId> {
    Some(RowId { layer, row })
}

/// A ridge over x = 1, 3, 5, 7, 9; segment 2 runs from (5, 7) to (7, 6).
const RIDGE: [Row; 5] = [
    (1.0, 5.0, 1.0),
    (3.0, 6.0, 1.0),
    (5.0, 7.0, 1.0),
    (7.0, 6.0, 1.0),
    (9.0, 5.0, 1.0),
];

/// The first segment's top edge crosses its baseline at (2, 5); the rest
/// (past x = 3) is a plain ridge.
const CROSSING: [Row; 5] = [
    (1.0, 8.0, 4.0),
    (3.0, 2.0, 6.0),
    (5.0, 7.0, 6.0),
    (7.0, 7.0, 6.0),
    (9.0, 7.0, 6.0),
];

/// The ridge's points every pick must agree on, and the row each picks.
fn ridge_points(x: &ScaleRef<Linear>, y: &ScaleRef<Linear>) -> Vec<(Pt, Option<usize>)> {
    vec![
        // Inside segment 2, left of its middle (x = 6), then right of it.
        (px(x, y, 5.5, 3.0), Some(2)),
        (px(x, y, 6.5, 3.0), Some(3)),
        (px(x, y, 1.2, 3.0), Some(0)),
        (px(x, y, 8.8, 3.0), Some(4)),
        // Above the ridge and below the baseline, well outside.
        (px(x, y, 5.5, 8.5), None),
        (px(x, y, 5.5, 0.4), None),
        // Beyond the last row.
        (px(x, y, 9.5, 3.0), None),
    ]
}

fn crossing_points(x: &ScaleRef<Linear>, y: &ScaleRef<Linear>) -> Vec<(Pt, Option<usize>)> {
    vec![
        // Inside (a, a0, c), then inside (b, b0, c).
        (px(x, y, 1.5, 5.5), Some(0)),
        (px(x, y, 2.5, 4.5), Some(1)),
        // In the quadrilateral's hull, between the top edge and the
        // baseline's ends, but outside both triangles: above and below the
        // crossing.
        (px(x, y, 2.0, 6.2), None),
        (px(x, y, 2.0, 3.8), None),
    ]
}

#[test]
fn a_point_inside_a_segment_picks_the_nearer_row_in_x() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    area(&mut plot, &x, &y, RIDGE.to_vec());
    plot.resolve(&cx, W, H).unwrap();
    for (at, want) in ridge_points(&x, &y) {
        assert_eq!(plot.pick_row(at), want.and_then(|r| id(0, r)), "{at:?}");
    }
    // The anchor is the picked row's top point.
    assert_eq!(
        plot.pick_anchor(RowId { layer: 0, row: 2 }),
        Some(px(&x, &y, 5.0, 7.0))
    );
}

#[test]
fn a_crossing_segment_fills_two_triangles_only() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    area(&mut plot, &x, &y, CROSSING.to_vec());
    plot.resolve(&cx, W, H).unwrap();
    for (at, want) in crossing_points(&x, &y) {
        assert_eq!(plot.pick_row(at), want.and_then(|r| id(0, r)), "{at:?}");
    }
}

#[test]
fn the_upper_band_of_two_stacked_areas_picks_the_upper_layer() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    let rows = |lo: f64, hi: f64| -> Vec<Row> {
        (0..5).map(|i| (1.0 + 2.0 * f64::from(i), hi, lo)).collect()
    };
    area(&mut plot, &x, &y, rows(0.0, 4.0));
    area(&mut plot, &x, &y, rows(4.0, 8.0));
    plot.resolve(&cx, W, H).unwrap();
    assert_eq!(plot.pick_row(px(&x, &y, 3.5, 6.0)), id(1, 1));
    assert_eq!(plot.pick_row(px(&x, &y, 2.5, 6.0)), id(1, 1));
    assert_eq!(plot.pick_row(px(&x, &y, 2.5, 2.0)), id(0, 1));
    assert_eq!(plot.pick_row(px(&x, &y, 2.5, 9.0)), None);
}

/// The GPU pick of a layer whose rows were released agrees with the CPU pick
/// on the same points.
#[test]
fn the_gpu_pick_agrees_with_the_cpu_pick() {
    let cx = Context::new_blocking().unwrap();
    type Points = fn(&ScaleRef<Linear>, &ScaleRef<Linear>) -> Vec<(Pt, Option<usize>)>;
    let scenes: [(&str, Vec<Row>, Points); 2] = [
        ("ridge", RIDGE.to_vec(), ridge_points),
        ("crossing", CROSSING.to_vec(), crossing_points),
    ];
    for (name, rows, points) in scenes {
        let (mut cpu, x, y) = base();
        area(&mut cpu, &x, &y, rows.clone());
        cpu.resolve(&cx, W, H).unwrap();
        let (mut gpu, x, y) = base();
        area(&mut gpu, &x, &y, rows)
            .max_chunk_rows(4)
            .retain(Retain::GpuOnly);
        gpu.resolve(&cx, W, H).unwrap();
        for (at, want) in points(&x, &y) {
            let want = want.and_then(|r| id(0, r));
            assert_eq!(cpu.pick_row(at), want, "{name} CPU {at:?}");
            assert_eq!(gpu.pick_row(at), None, "{name}: no rows to read");
            assert_eq!(
                gpu.pick_row_gpu(&cx, at).unwrap(),
                want,
                "{name} GPU {at:?}"
            );
        }
    }
}

/// Over a grid of points, a GPU hit is the CPU's hit; the CPU also hits
/// within the pick tolerance outside the region, where the GPU draws no
/// fragment (an area has no pick margin).
#[test]
fn a_gpu_hit_is_the_cpu_hit() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<Row> = vec![
        (0.5, 7.0, 2.0),
        (2.5, 3.0, 5.0),
        (4.0, 8.0, 1.0),
        (6.5, 2.0, 4.0),
        (7.5, 6.0, 6.5),
        (9.5, 4.0, 0.5),
    ];
    let (mut plot, x, y) = base();
    area(&mut plot, &x, &y, rows);
    plot.resolve(&cx, W, H).unwrap();
    let r = plot.layout().unwrap().plot;
    let (mut hits, mut gpu_hits) = (0, 0);
    for i in 0..60 {
        for j in 0..40 {
            let at = Pt::new(
                r.x + (i as f32 + 0.37) * r.width / 60.0,
                r.y + (j as f32 + 0.61) * r.height / 40.0,
            );
            let cpu = plot.pick_row(at);
            let gpu = plot.pick_row_gpu(&cx, at).unwrap();
            if gpu.is_some() {
                assert_eq!(gpu, cpu, "{at:?}");
                gpu_hits += 1;
            }
            hits += usize::from(cpu.is_some());
        }
    }
    assert!(gpu_hits > 300, "{gpu_hits} GPU hits");
    assert!(hits >= gpu_hits);
}

#[test]
fn a_tooltip_shows_the_row_under_the_pointer() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    area(&mut plot, &x, &y, RIDGE.to_vec()).tooltip(|r| format!("x = {}", r.0));
    plot.resolve(&cx, W, H).unwrap();
    assert!(plot.layout().unwrap().tooltip.is_none());
    for (at, text) in [
        (px(&x, &y, 5.5, 3.0), "x = 5"),
        (px(&x, &y, 6.5, 3.0), "x = 7"),
    ] {
        assert!(plot.handle(Input::PointerMoved { at }).redraw);
        let resolved = plot.resolve(&cx, W, H).unwrap();
        assert!(resolved.layout.tooltip.is_some());
        let runs: Vec<String> = resolved
            .scene
            .items
            .iter()
            .filter(|i| i.z == Z_TOOLTIP)
            .filter_map(|i| match &i.kind {
                ItemKind::Text(runs) => Some(runs),
                _ => None,
            })
            .flatten()
            .map(|r| r.text.to_string())
            .collect();
        assert_eq!(runs, [text]);
    }
    // Off the area, the tooltip goes.
    let off = px(&x, &y, 5.5, 9.0);
    assert!(plot.handle(Input::PointerMoved { at: off }).redraw);
    plot.resolve(&cx, W, H).unwrap();
    assert!(plot.layout().unwrap().tooltip.is_none());
}

#[test]
fn a_brush_selects_the_rows_whose_top_point_is_inside() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    area(&mut plot, &x, &y, RIDGE.to_vec());
    plot.drag(Drag::Brush);
    plot.resolve(&cx, W, H).unwrap();
    // Around the top points of rows 1 to 3 (y = 6, 7, 6), not the baseline.
    let (a, b) = (px(&x, &y, 2.5, 7.5), px(&x, &y, 7.5, 5.5));
    let button = Button::Primary;
    let _ = plot.handle(Input::PointerDown { at: a, button });
    let _ = plot.handle(Input::PointerMoved {
        at: Pt::new(a.x + 20.0, a.y + 20.0),
    });
    let _ = plot.handle(Input::PointerMoved { at: b });
    let _ = plot.handle(Input::PointerUp { at: b, button });
    let rows = plot.brush().expect("a brush").rows.clone();
    assert_eq!(
        rows,
        [
            RowId { layer: 0, row: 1 },
            RowId { layer: 0, row: 2 },
            RowId { layer: 0, row: 3 }
        ]
    );
}
