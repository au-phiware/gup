// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S8c (GUP-446): hover, click and tooltips, from synthetic input.

mod common;

use common::vr::{harness, layout_metadata};
use gup_core::geom::Point as Pt;
use gup_core::prelude::*;
use gup_core::scene::{ItemKind, TextRole, Z_TOOLTIP};
use gup_core::{Button, EventInfo, ImageTarget, Input, RowId};
use gup_visual_regression::RgbaImage;
use std::sync::{Arc, Mutex};

const W: f32 = 480.0;
const H: f32 = 320.0;

#[derive(Clone, Debug)]
struct Row {
    name: String,
    x: f64,
    y: f64,
    mass: f64,
}

fn rows(n: usize) -> Vec<Row> {
    const NAMES: [&str; 5] = ["Tui", "Weka", "Kiwi", "Kea", "Kaka"];
    (0..n)
        .map(|i| Row {
            name: NAMES[i % 5].into(),
            x: i as f64 * 10.0,
            y: [3.0, 7.0, 4.0, 8.0, 2.0][i % 5],
            mass: [0.1, 1.2, 2.4, 0.9, 0.5][i % 5],
        })
        .collect()
}

type Log = Arc<Mutex<Vec<String>>>;

fn log(log: &Log, tag: &'static str) -> impl FnMut(&EventInfo, &Row) + Send + Sync + 'static {
    let log = Arc::clone(log);
    move |info, row| {
        let mut log = log.lock().unwrap();
        log.push(format!("{tag}({}) {}", info.row.row, row.name));
        assert_eq!(info.row.layer, 0);
    }
}

fn take(l: &Log) -> Vec<String> {
    std::mem::take(&mut *l.lock().unwrap())
}

/// Three circles with all three handlers logging, resolved at 480x320.
fn fixture(retain: Retain) -> (Context, Plot, Log) {
    let cx = Context::new_blocking().expect("headless GPU context");
    let l: Log = Log::default();
    let mut plot = Plot::new();
    plot.circles(rows(3))
        .x(|r| r.x)
        .y(|r| r.y)
        .radius(Px(8.0))
        .retain(retain)
        .on(Event::Enter, log(&l, "Enter"))
        .on(Event::Leave, log(&l, "Leave"))
        .on(Event::Click, log(&l, "Click"));
    plot.resolve(&cx, W, H).unwrap();
    (cx, plot, l)
}

fn anchor(plot: &mut Plot, row: usize) -> Pt {
    plot.pick_anchor(RowId { layer: 0, row }).unwrap()
}

fn moved(plot: &mut Plot, at: Pt) -> bool {
    plot.handle(Input::PointerMoved { at }).redraw
}

fn off(p: Pt, dx: f32, dy: f32) -> Pt {
    Pt::new(p.x + dx, p.y + dy)
}

fn empty_space(plot: &Plot) -> Pt {
    let r = plot.layout().unwrap().plot;
    Pt::new(r.left() + 3.0, r.top() + 3.0)
}

#[test]
fn enter_and_leave_follow_the_pointer_in_order() {
    let (_cx, mut plot, l) = fixture(Retain::Auto);
    let (a0, a1) = (anchor(&mut plot, 0), anchor(&mut plot, 1));
    let space = empty_space(&plot);
    assert_eq!(plot.hovered(), None);

    moved(&mut plot, a0);
    assert_eq!(plot.hovered(), Some(RowId { layer: 0, row: 0 }));
    moved(&mut plot, off(a0, 2.0, 1.0));
    moved(&mut plot, a1);
    assert_eq!(plot.hovered(), Some(RowId { layer: 0, row: 1 }));
    // Within a disc: nothing.
    moved(&mut plot, off(a1, 3.0, -2.0));
    moved(&mut plot, space);
    assert_eq!(plot.hovered(), None);
    moved(&mut plot, a1);
    let _ = plot.handle(Input::PointerLeft);
    assert_eq!(plot.hovered(), None);
    let _ = plot.handle(Input::PointerLeft);

    assert_eq!(
        take(&l),
        [
            "Enter(0) Tui",
            "Leave(0) Tui",
            "Enter(1) Weka",
            "Leave(1) Weka",
            "Enter(1) Weka",
            "Leave(1) Weka",
        ]
    );
}

#[test]
fn event_info_carries_the_pointer_and_the_row() {
    let (_cx, mut plot, _) = fixture(Retain::Auto);
    let seen = Arc::new(Mutex::new(Vec::<EventInfo>::new()));
    let s = Arc::clone(&seen);
    plot.layer_mut::<Row, Circle>()
        .unwrap()
        .on(Event::Enter, move |info, _| s.lock().unwrap().push(*info));
    let at = off(anchor(&mut plot, 2), 1.0, 1.0);
    moved(&mut plot, at);
    let seen = seen.lock().unwrap();
    assert_eq!(
        *seen,
        [EventInfo {
            event: Event::Enter,
            at,
            row: RowId { layer: 0, row: 2 }
        }]
    );
}

#[test]
fn a_click_needs_a_press_and_release_within_four_pixels() {
    let (_cx, mut plot, l) = fixture(Retain::Auto);
    let a2 = anchor(&mut plot, 2);
    let down = |button, at| Input::PointerDown { at, button };
    let up = |button, at| Input::PointerUp { at, button };

    let _ = plot.handle(down(Button::Primary, a2));
    let _ = plot.handle(up(Button::Primary, off(a2, 3.0, 0.0)));
    assert_eq!(take(&l), ["Click(2) Kiwi"]);

    // Moved 5 px between down and up (still on the disc): no click.
    let _ = plot.handle(down(Button::Primary, a2));
    let _ = plot.handle(up(Button::Primary, off(a2, 5.0, 0.0)));
    assert_eq!(take(&l), Vec::<String>::new());

    // An up without a down.
    let _ = plot.handle(up(Button::Primary, a2));
    for b in [Button::Secondary, Button::Middle] {
        let _ = plot.handle(down(b, a2));
        let _ = plot.handle(up(b, a2));
    }
    assert_eq!(take(&l), Vec::<String>::new());

    // A press and release over empty space clicks nothing.
    let space = empty_space(&plot);
    let _ = plot.handle(down(Button::Primary, space));
    let _ = plot.handle(up(Button::Primary, space));
    assert_eq!(take(&l), Vec::<String>::new());
}

#[test]
fn handlers_receive_the_row_data() {
    let (_cx, mut plot, _) = fixture(Retain::Auto);
    let names = Arc::new(Mutex::new(Vec::<String>::new()));
    let n = Arc::clone(&names);
    plot.layer_mut::<Row, Circle>()
        .unwrap()
        .on(Event::Click, move |_, r: &Row| {
            n.lock().unwrap().push(r.name.clone());
        });
    let a = anchor(&mut plot, 1);
    let _ = plot.handle(Input::PointerDown {
        at: a,
        button: Button::Primary,
    });
    let _ = plot.handle(Input::PointerUp {
        at: a,
        button: Button::Primary,
    });
    assert_eq!(*names.lock().unwrap(), ["Weka"]);
}

#[test]
fn columns_only_layers_hover_but_run_no_handler() {
    let (_cx, mut plot, l) = fixture(Retain::Columns);
    let a = anchor(&mut plot, 1);
    moved(&mut plot, a);
    assert_eq!(plot.hovered(), Some(RowId { layer: 0, row: 1 }));
    let _ = plot.handle(Input::PointerDown {
        at: a,
        button: Button::Primary,
    });
    let _ = plot.handle(Input::PointerUp {
        at: a,
        button: Button::Primary,
    });
    let space = empty_space(&plot);
    moved(&mut plot, space);
    assert_eq!(plot.hovered(), None);
    assert_eq!(take(&l), Vec::<String>::new());
}

#[test]
fn a_zoom_leaves_the_hovered_row() {
    let (_cx, mut plot, l) = fixture(Retain::Auto);
    let a0 = anchor(&mut plot, 0);
    moved(&mut plot, a0);
    assert!(plot.handle(Input::Wheel { at: a0, lines: 1.0 }).redraw);
    assert_eq!(plot.hovered(), None);
    assert_eq!(take(&l), ["Enter(0) Tui", "Leave(0) Tui"]);
}

fn with_tooltip(n: usize, retain: Retain) -> (Context, Plot) {
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut plot = Plot::new();
    plot.circles(rows(n))
        .x(|r| r.x)
        .y(|r| r.y)
        .radius(Px(6.0))
        .retain(retain)
        .tooltip(|r| format!("{}\nmass {:.1} kg", r.name, r.mass));
    plot.resolve(&cx, W, H).unwrap();
    (cx, plot)
}

#[test]
fn redraw_only_when_the_tooltip_changes() {
    let (_cx, mut plot) = with_tooltip(3, Retain::Auto);
    let (a0, a1) = (anchor(&mut plot, 0), anchor(&mut plot, 1));
    let space = empty_space(&plot);
    assert!(!moved(&mut plot, space));
    assert!(moved(&mut plot, a0), "enter");
    assert!(!moved(&mut plot, off(a0, 1.0, 1.0)), "within the row");
    assert!(moved(&mut plot, a1), "change");
    assert!(moved(&mut plot, space), "leave");
    assert!(moved(&mut plot, a1));
    assert!(plot.handle(Input::PointerLeft).redraw);

    // A layer without a tooltip never asks for a redraw.
    let (_cx, mut plot, _) = fixture(Retain::Auto);
    let a = anchor(&mut plot, 0);
    assert!(!moved(&mut plot, a));
    let space = empty_space(&plot);
    assert!(!moved(&mut plot, space));
}

#[test]
fn hovering_composes_a_tooltip_without_resolving_again() {
    let (cx, mut plot) = with_tooltip(3, Retain::Auto);
    let a1 = anchor(&mut plot, 1);
    let generation = plot.resolve_generation();
    let stats = cx.upload_stats();
    assert!(moved(&mut plot, a1));

    let resolved = plot.resolve(&cx, W, H).unwrap();
    let at_tooltip: Vec<_> = resolved
        .scene
        .items
        .iter()
        .filter(|i| i.z == Z_TOOLTIP)
        .collect();
    assert_eq!(at_tooltip.len(), 2);
    assert!(matches!(at_tooltip[0].kind, ItemKind::Rects(_)));
    let ItemKind::Text(runs) = &at_tooltip[1].kind else {
        panic!("second tooltip item is not text");
    };
    assert_eq!(runs.len(), 2);
    assert!(runs.iter().all(|r| r.role == TextRole::Tooltip));
    assert_eq!(&*runs[0].text, "Weka");
    assert_eq!(&*runs[1].text, "mass 1.2 kg");
    assert!(resolved.layout.tooltip.is_some());
    assert!(
        resolved
            .layout
            .texts
            .iter()
            .all(|t| t.run.role != TextRole::Tooltip)
    );

    assert_eq!(plot.resolve_generation(), generation);
    let d = cx.upload_stats() - stats;
    assert_eq!(d.columns.bytes, 0);
    assert_eq!(d.validity.bytes, 0);
    assert_eq!(d.uniforms.bytes, 0);
    // Resolving again is free.
    let stats = cx.upload_stats();
    plot.resolve(&cx, W, H).unwrap();
    assert_eq!(cx.upload_stats(), stats);

    let _ = plot.handle(Input::PointerLeft);
    let resolved = plot.resolve(&cx, W, H).unwrap();
    assert!(resolved.scene.items.iter().all(|i| i.z != Z_TOOLTIP));
    assert_eq!(resolved.layout.tooltip, None);
    assert_eq!(plot.layout().unwrap().tooltip, None);
    assert_eq!(plot.resolve_generation(), generation);
}

#[test]
fn a_resize_while_hovering_moves_the_tooltip_to_the_new_anchor() {
    let (cx, mut plot) = with_tooltip(3, Retain::Auto);
    let a1 = anchor(&mut plot, 0);
    moved(&mut plot, a1);
    let before = plot.resolve(&cx, W, H).unwrap().layout.tooltip.unwrap();
    let generation = plot.resolve_generation();

    let layout = plot.resolve(&cx, 640.0, 400.0).unwrap().layout.clone();
    assert_eq!(plot.resolve_generation(), generation + 1);
    let a1b = anchor(&mut plot, 0);
    assert_ne!(a1, a1b);
    let tip = layout.tooltip.expect("still hovering");
    assert_ne!(tip, before);
    // Up and right of the new anchor.
    assert_eq!(tip.left(), (a1b.x + 8.0).round());
    assert_eq!(tip.bottom(), (a1b.y - 8.0).round());
    assert_eq!(plot.hovered(), Some(RowId { layer: 0, row: 0 }));
}

#[test]
fn a_tooltip_near_the_corner_flips_and_a_long_one_clamps() {
    let (cx, mut plot) = with_tooltip(5, Retain::Auto);
    // The right-most row (x = 40) is at the plot's right edge.
    let a = anchor(&mut plot, 4);
    let plot_rect = plot.layout().unwrap().plot;
    assert!(plot_rect.right() - a.x < 12.0, "{a:?} {plot_rect:?}");
    moved(&mut plot, a);
    let tip = plot.resolve(&cx, W, H).unwrap().layout.tooltip.unwrap();
    assert!(tip.right() <= a.x - 8.0 + 0.01, "{tip:?} {a:?}");
    assert!(tip.left() >= 1.0 && tip.right() <= W - 1.0);
    assert!(tip.top() >= 1.0 && tip.bottom() <= H - 1.0);
    for v in [tip.left(), tip.top(), tip.right(), tip.bottom()] {
        assert_eq!(v, v.round());
    }

    // Text wider than the canvas sits at x = 1.
    let mut long = Plot::new();
    long.circles(rows(3))
        .x(|r| r.x)
        .y(|r| r.y)
        .tooltip(|r| format!("{} {}", r.name, "very long ".repeat(30)));
    long.resolve(&cx, W, H).unwrap();
    let a = anchor(&mut long, 1);
    moved(&mut long, a);
    let tip = long.resolve(&cx, W, H).unwrap().layout.tooltip.unwrap();
    assert_eq!(tip.left(), 1.0);
}

#[test]
fn an_empty_tooltip_draws_nothing() {
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut plot = Plot::new();
    plot.circles(rows(3))
        .x(|r| r.x)
        .y(|r| r.y)
        .tooltip(|_| String::new());
    plot.resolve(&cx, W, H).unwrap();
    let a = anchor(&mut plot, 1);
    assert!(!moved(&mut plot, a));
    assert_eq!(plot.hovered(), Some(RowId { layer: 0, row: 1 }));
    let resolved = plot.resolve(&cx, W, H).unwrap();
    assert_eq!(resolved.layout.tooltip, None);
    assert!(resolved.scene.items.iter().all(|i| i.z != Z_TOOLTIP));
}

#[test]
fn tooltip_golden() {
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut plot = Plot::new();
    plot.circles(rows(5))
        .x(|r| r.x)
        .y(|r| r.y)
        .radius(Px(5.0))
        .tooltip(|r| format!("{}\nmass {:.1} kg", r.name, r.mass));
    plot.resolve(&cx, W, H).unwrap();
    let a = anchor(&mut plot, 2);
    moved(&mut plot, a);
    let resolved = plot.resolve(&cx, W, H).unwrap();
    let layout = resolved.layout.clone();
    let tip = layout.tooltip.unwrap();
    assert_eq!(tip.left(), (a.x + 8.0).round());
    assert_eq!(tip.bottom(), (a.y - 8.0).round());
    let image = ImageTarget::new(&cx, W as u32, H as u32)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(W as u32, H as u32, image.into_raw()).unwrap();
    harness()
        .run("gup_core/tooltip", Ok((vr, layout_metadata(&layout))))
        .assert_ok();
}

// ---- Brush and drag-zoom (GUP-454) ------------------------------------

fn feed(plot: &mut Plot, input: Input) {
    let _ = plot.handle(input);
}

/// Press at `from`, move past the click slop, move to `to`, release.
fn drag(plot: &mut Plot, from: Pt, to: Pt) {
    let button = Button::Primary;
    feed(plot, Input::PointerDown { at: from, button });
    let mid = Pt::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    feed(plot, Input::PointerMoved { at: mid });
    feed(plot, Input::PointerMoved { at: to });
    feed(plot, Input::PointerUp { at: to, button });
}

/// The pixel of `value` along an axis, from its tick pixels.
fn px(ticks: &[(f64, f32)], value: f64) -> f32 {
    let (a, b) = (ticks[0], *ticks.last().unwrap());
    (f64::from(a.1) + (value - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)) as f32
}

/// Four layers at different `z`; positions (data units) are listed in
/// `MIXED_*`.
const CIRCLES: [(f64, f64); 3] = [(2.0, 2.0), (4.0, 6.0), (8.0, 8.0)];
const RECTS: [(f64, f64, f64, f64); 2] = [(1.0, 1.0, 5.0, 3.0), (6.0, 6.0, 9.0, 9.0)];
const SEGMENTS: [(f64, f64, f64, f64); 2] = [(1.0, 8.0, 3.0, 6.0), (7.0, 1.0, 9.0, 3.0)];
const LINE: [(f64, f64); 5] = [(1.0, 5.0), (3.0, 4.0), (5.0, 5.0), (7.0, 3.0), (9.0, 5.0)];

fn mixed() -> (Context, Plot, Log) {
    let clicks = Log::default();
    let l = clicks.clone();
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut plot = Plot::new();
    plot.circles(CIRCLES)
        .x(|r| r.0)
        .y(|r| r.1)
        .radius(Px(4.0))
        .on(Event::Click, move |info, _| {
            l.lock().unwrap().push(format!("Click {}", info.row.row));
        })
        .z(3);
    plot.rects(RECTS)
        .x(|r| r.0)
        .y(|r| r.1)
        .x_end(|r| r.2)
        .y_end(|r| r.3)
        .z(1);
    plot.segments(SEGMENTS)
        .x(|r| r.0)
        .y(|r| r.1)
        .x_end(|r| r.2)
        .y_end(|r| r.3)
        .width(Px(2.0))
        .z(2);
    plot.line(LINE).x(|r| r.0).y(|r| r.1).z(4);
    plot.drag(Drag::Brush);
    plot.resolve(&cx, W, H).unwrap();
    (cx, plot, clicks)
}

/// The pixel rectangle of the data box `x0..x1`, `y0..y1`.
fn data_box(plot: &Plot, x: (f64, f64), y: (f64, f64)) -> (Pt, Pt) {
    let l = plot.layout().unwrap();
    (
        Pt::new(px(&l.x_ticks, x.0), px(&l.y_ticks, y.1)),
        Pt::new(px(&l.x_ticks, x.1), px(&l.y_ticks, y.0)),
    )
}

fn id(layer: usize, row: usize) -> RowId {
    RowId { layer, row }
}

#[test]
fn a_brush_selects_rows_by_mark_position_for_all_four_marks() {
    let (_cx, mut plot, _) = mixed();
    let (a, b) = data_box(&plot, (1.9, 5.1), (1.9, 7.1));
    drag(&mut plot, a, b);
    let brush = plot.brush().expect("a brush");
    // Circles (2,2), (4,6); rect centre (3,2); segment midpoint (2,7); line
    // vertices (3,4) and (5,5).
    assert_eq!(
        brush.rows,
        [id(0, 0), id(0, 1), id(1, 0), id(2, 0), id(3, 1), id(3, 2)]
    );
    assert!((brush.x.0 - 1.9).abs() < 0.05 && (brush.x.1 - 5.1).abs() < 0.05);
    assert!((brush.y.0 - 1.9).abs() < 0.05 && (brush.y.1 - 7.1).abs() < 0.05);
}

#[test]
fn a_big_mark_overlapping_but_not_centred_inside_is_not_brushed() {
    let (_cx, mut plot, _) = mixed();
    // Covers most of rect 0 (1..5 × 1..3) but not its centre (3, 2).
    let (a, b) = data_box(&plot, (3.5, 5.1), (0.0, 2.6));
    drag(&mut plot, a, b);
    assert!(plot.brush().unwrap().rows.iter().all(|r| r.layer != 1));
}

#[test]
fn handlers_run_once_on_release_and_a_click_clears() {
    let (_cx, mut plot, clicks) = mixed();
    let calls: Arc<Mutex<Vec<Option<usize>>>> = Arc::default();
    let c = calls.clone();
    plot.on_brush(move |b| c.lock().unwrap().push(b.map(|b| b.rows.len())));
    let button = Button::Primary;
    let (a, b) = data_box(&plot, (1.9, 5.1), (1.9, 7.1));
    feed(&mut plot, Input::PointerDown { at: a, button });
    assert!(
        plot.handle(Input::PointerMoved {
            at: off(a, 20.0, -20.0)
        })
        .redraw
    );
    feed(&mut plot, Input::PointerMoved { at: b });
    assert!(calls.lock().unwrap().is_empty(), "moves call no handler");
    assert!(plot.brush().is_none());
    feed(&mut plot, Input::PointerUp { at: b, button });
    assert_eq!(*calls.lock().unwrap(), [Some(6)]);

    // A click on a row clears the brush and is still delivered.
    let row_at = plot.pick_anchor(id(0, 2)).unwrap();
    feed(&mut plot, Input::PointerDown { at: row_at, button });
    feed(
        &mut plot,
        Input::PointerUp {
            at: off(row_at, 1.0, 1.0),
            button,
        },
    );
    assert!(plot.brush().is_none());
    assert_eq!(*calls.lock().unwrap(), [Some(6), None]);
    assert_eq!(take(&clicks), ["Click 2"]);

    // clear_brush with no brush calls nothing.
    plot.clear_brush();
    assert_eq!(calls.lock().unwrap().len(), 2);
}

#[test]
fn a_brush_from_outside_is_clamped_and_an_empty_one_clears() {
    let (_cx, mut plot, _) = mixed();
    let r = plot.layout().unwrap().plot;
    let (a, b) = data_box(&plot, (7.0, 9.0), (7.0, 9.0));
    // From beyond the bottom-right corner into the upper left.
    drag(&mut plot, Pt::new(r.right() + 40.0, r.bottom() + 40.0), a);
    let rect = plot.brush_rect().unwrap();
    assert!((rect.right() - r.right()).abs() < 0.01);
    assert!((rect.bottom() - r.bottom()).abs() < 0.01);
    // Everything right of and below `a` that is drawn: circle 2 and rect 1.
    let rows = &plot.brush().unwrap().rows;
    assert!(rows.contains(&id(0, 2)) && rows.contains(&id(1, 1)));
    // Dragging along the plot's edge from outside is zero-width: it clears.
    let calls: Arc<Mutex<Vec<bool>>> = Arc::default();
    let c = calls.clone();
    plot.on_brush(move |b| c.lock().unwrap().push(b.is_some()));
    drag(
        &mut plot,
        Pt::new(r.right() + 30.0, r.top() + 30.0),
        Pt::new(r.right() + 60.0, r.top() + 90.0),
    );
    assert!(plot.brush().is_none());
    assert_eq!(*calls.lock().unwrap(), [false]);
    let _ = b;
}

#[test]
fn a_brush_survives_a_wheel_step_and_a_resize_and_follows_the_data() {
    let (cx, mut plot, _) = mixed();
    let (a, b) = data_box(&plot, (1.9, 5.1), (1.9, 7.1));
    drag(&mut plot, a, b);
    let before = plot.brush().cloned().unwrap();
    let generation = plot.resolve_generation();
    // The overlay is composed on the cached resolve.
    plot.resolve(&cx, W, H).unwrap();
    assert_eq!(plot.resolve_generation(), generation);
    let at = Pt::new(W / 2.0, H / 2.0);
    for (w, h, lines) in [(W, H, -1.0), (W + 40.0, H + 20.0, 0.0)] {
        if lines != 0.0 {
            feed(&mut plot, Input::Wheel { at, lines });
        }
        plot.resolve(&cx, w, h).unwrap();
        assert_eq!(plot.brush(), Some(&before));
        let l = plot.layout().unwrap().clone();
        let rect = plot.brush_rect().unwrap();
        let want = [
            px(&l.x_ticks, before.x.0),
            px(&l.y_ticks, before.y.1),
            px(&l.x_ticks, before.x.1),
            px(&l.y_ticks, before.y.0),
        ];
        let got = [rect.left(), rect.top(), rect.right(), rect.bottom()];
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 0.01, "{got:?} vs {want:?}");
        }
    }
}

#[test]
fn drag_zoom_sets_the_domains_to_the_rectangle_and_reset_view_undoes_it() {
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.add(Selection::<(f64, f64), Circle>::new(CIRCLES.to_vec()))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Circle::RADIUS, Px(4.0));
    plot.drag(Drag::Zoom);
    let r = plot.resolve(&cx, W, H).unwrap().layout.plot;
    let first = (x.read().current_domain(), y.read().current_domain());
    let (a, b) = (
        Pt::new(r.left() + 60.0, r.top() + 40.0),
        Pt::new(r.left() + 200.0, r.top() + 150.0),
    );
    let (va, vb) = (plot.invert(a).unwrap(), plot.invert(b).unwrap());
    drag(&mut plot, a, b);
    assert!(plot.brush().is_none());
    plot.resolve(&cx, W, H).unwrap();
    let close = |got: f64, want: f64| (got - want).abs() <= 1e-9 * want.abs().max(1.0);
    let (dx, dy) = (
        x.read().current_domain().unwrap(),
        y.read().current_domain().unwrap(),
    );
    assert!(
        close(dx.0, va.0) && close(dx.1, vb.0),
        "{dx:?} {va:?} {vb:?}"
    );
    assert!(
        close(dy.0, vb.1) && close(dy.1, va.1),
        "{dy:?} {va:?} {vb:?}"
    );

    // Too narrow on one axis changes nothing.
    drag(&mut plot, a, off(a, 3.0, 80.0));
    assert_eq!(x.read().current_domain().unwrap(), dx);
    plot.reset_view();
    plot.resolve(&cx, W, H).unwrap();
    assert_eq!(
        (x.read().current_domain(), y.read().current_domain()),
        first
    );
}

#[test]
fn brush_golden() {
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut plot = Plot::new();
    plot.circles(rows(5))
        .x(|r| r.x)
        .y(|r| r.y)
        .radius(Px(5.0))
        .tooltip(|r| format!("{}\nmass {:.1} kg", r.name, r.mass));
    plot.drag(Drag::Brush);
    plot.resolve(&cx, W, H).unwrap();
    // Brush the dots of rows 1 and 2 (x 10 and 20; y 7 and 4).
    let (p1, p2) = (anchor(&mut plot, 1), anchor(&mut plot, 2));
    let from = Pt::new(p1.x - 20.0, p1.y - 30.0);
    let to = Pt::new(p2.x + 20.0, p2.y + 30.0);
    drag(&mut plot, from, to);
    assert_eq!(plot.brush().unwrap().rows, [id(0, 1), id(0, 2)]);
    let resolved = plot.resolve(&cx, W, H).unwrap();
    let layout = resolved.layout.clone();
    let image = ImageTarget::new(&cx, W as u32, H as u32)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(W as u32, H as u32, image.into_raw()).unwrap();
    // A background pixel inside the brush, away from the dots, is tinted
    // blue; the border is the stroke colour.
    let rect = plot.brush_rect().unwrap();
    let at = |x: f32, y: f32| {
        let i = (y as usize * W as usize + x as usize) * 4;
        let d = vr.data();
        [d[i], d[i + 1], d[i + 2]]
    };
    let (l, t) = (rect.left().round(), rect.top().round());
    let inside = at(l + 4.0, t + 4.0);
    assert!(
        inside[0] >= 200 && i32::from(inside[2]) - i32::from(inside[0]) >= 10,
        "{inside:?}"
    );
    let border = at(l + 30.0, t);
    for (g, w) in border.iter().zip([0x44u8, 0x77, 0xaa]) {
        assert!(g.abs_diff(w) <= 6, "border {border:?}");
    }
    harness()
        .run("gup_core/brush", Ok((vr, layout_metadata(&layout))))
        .assert_ok();
}

// ---- GPU-picked hover and click (GUP-456) ------------------------------

type RowLog = Arc<Mutex<Vec<(Event, usize)>>>;

/// 12 circles, `GpuOnly` in chunks of 4 (so the CPU cannot read them), with
/// `on_row` logging; or the same rows kept (`Retain::Auto`) for the CPU
/// path.
fn gpu_fixture(retain: Retain) -> (Context, Plot, RowLog) {
    let (cx, plot, seen, _) = gpu_fixture_scales(retain);
    (cx, plot, seen)
}

fn gpu_fixture_scales(retain: Retain) -> (Context, Plot, RowLog, [ScaleRef<Linear>; 2]) {
    let cx = Context::new_blocking().expect("headless GPU context");
    let seen = RowLog::default();
    let mut plot = Plot::new();
    let sink = Arc::clone(&seen);
    plot.on_row(move |info| sink.lock().unwrap().push((info.event, info.row.row)));
    let mut sel = Selection::<Row, Circle>::new(rows(12));
    sel.max_chunk_rows(4).retain(retain);
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.add(sel)
        .attr(Circle::X, x.encode(|r: &Row| r.x))
        .attr(Circle::Y, y.encode(|r: &Row| r.y))
        .attr(Circle::RADIUS, Px(8.0));
    plot.resolve(&cx, W, H).unwrap();
    (cx, plot, seen, [x, y])
}

fn row_at(plot: &Plot, i: usize) -> Pt {
    let l = plot.layout().unwrap();
    let r = rows(12);
    Pt::new(px(&l.x_ticks, r[i].x), px(&l.y_ticks, r[i].y))
}

/// Resolve until nothing is in flight, at most 10 times.
fn settle(plot: &mut Plot, cx: &Context) {
    for _ in 0..10 {
        if !Chart::needs_frame(plot) {
            return;
        }
        // A frame's time: the GPU finishes what was submitted.
        std::thread::sleep(std::time::Duration::from_millis(10));
        plot.resolve(cx, W, H).unwrap();
    }
    panic!("a pick was still in flight after 10 resolves");
}

fn events(seen: &RowLog) -> Vec<(Event, usize)> {
    std::mem::take(&mut *seen.lock().unwrap())
}

fn row_id(row: usize) -> Option<RowId> {
    Some(RowId { layer: 0, row })
}

#[test]
fn cpu_readable_plots_keep_the_synchronous_path() {
    let (cx, mut plot, seen) = gpu_fixture(Retain::Auto);
    let at = row_at(&plot, 5);
    let before = cx.submissions();
    moved(&mut plot, at);
    assert!(!Chart::needs_frame(&plot));
    assert_eq!(plot.hovered(), row_id(5));
    assert_eq!(events(&seen), [(Event::Enter, 5)]);
    assert_eq!(cx.submissions(), before);
}

#[test]
fn a_gpu_only_layer_hovers_after_a_resolve() {
    let (cx, mut plot, seen) = gpu_fixture(Retain::GpuOnly);
    assert!(Chart::needs_frame(&plot) || plot.hovered().is_none());
    let at = row_at(&plot, 5);
    assert!(moved(&mut plot, at));
    assert!(Chart::needs_frame(&plot));
    assert_eq!(plot.hovered(), None);
    settle(&mut plot, &cx);
    assert_eq!(plot.hovered(), row_id(5));
    assert_eq!(events(&seen), [(Event::Enter, 5)]);
    // Leaving works the same way.
    let space = empty_space(&plot);
    assert!(moved(&mut plot, space));
    settle(&mut plot, &cx);
    assert_eq!(plot.hovered(), None);
    assert_eq!(events(&seen), [(Event::Leave, 5)]);
}

#[test]
fn moves_between_resolves_submit_one_pick_for_the_last_position() {
    let (cx, mut plot, seen) = gpu_fixture(Retain::GpuOnly);
    let before = cx.submissions();
    for i in 0..10 {
        let at = row_at(&plot, i);
        assert!(moved(&mut plot, at));
    }
    let last = row_at(&plot, 11);
    moved(&mut plot, last);
    plot.resolve(&cx, W, H).unwrap();
    settle(&mut plot, &cx);
    assert_eq!(cx.submissions() - before, 1);
    assert_eq!(plot.hovered(), row_id(11));
    assert_eq!(events(&seen), [(Event::Enter, 11)]);
}

#[test]
fn a_result_for_an_older_position_is_dropped() {
    let (cx, mut plot, seen) = gpu_fixture(Retain::GpuOnly);
    let at = row_at(&plot, 5);
    moved(&mut plot, at);
    plot.resolve(&cx, W, H).unwrap(); // submitted
    let space = empty_space(&plot);
    moved(&mut plot, space);
    settle(&mut plot, &cx);
    assert_eq!(plot.hovered(), None);
    let seen = events(&seen);
    assert!(
        seen.is_empty() || seen == [(Event::Enter, 5), (Event::Leave, 5)],
        "{seen:?}"
    );
}

/// A pick drawn before a view change must not hover: the view moved under it.
#[test]
fn a_pick_older_than_the_last_resolve_is_dropped() {
    let (cx, mut plot, seen) = gpu_fixture(Retain::GpuOnly);
    let at = row_at(&plot, 5);
    moved(&mut plot, at);
    plot.resolve(&cx, W, H).unwrap(); // submitted at generation g
    let g = plot.resolve_generation();
    // Row 5 and its neighbours: 18 px is nothing's hit.
    plot.pan(18.0, 0.0).unwrap();
    settle(&mut plot, &cx);
    assert!(plot.resolve_generation() > g);
    assert!(
        events(&seen).iter().all(|e| *e != (Event::Enter, 5)),
        "the stale pick hovered row 5"
    );
    assert_ne!(plot.hovered(), row_id(5));
    // The hover is what a fresh pick finds where the pointer is.
    assert_eq!(plot.hovered(), plot.pick_row_gpu(&cx, at).unwrap());
}

#[test]
fn a_gpu_click_needs_a_press_and_release_within_four_pixels() {
    let (cx, mut plot, seen) = gpu_fixture(Retain::GpuOnly);
    let at = row_at(&plot, 7);
    let button = Button::Primary;
    feed(&mut plot, Input::PointerDown { at, button });
    feed(
        &mut plot,
        Input::PointerUp {
            at: off(at, 2.0, 0.0),
            button,
        },
    );
    assert!(Chart::needs_frame(&plot));
    settle(&mut plot, &cx);
    assert_eq!(events(&seen), [(Event::Click, 7)]);

    // A pan (beyond 4 px) is no click.
    feed(&mut plot, Input::PointerDown { at, button });
    feed(
        &mut plot,
        Input::PointerMoved {
            at: off(at, 10.0, 0.0),
        },
    );
    feed(
        &mut plot,
        Input::PointerUp {
            at: off(at, 10.0, 0.0),
            button,
        },
    );
    settle(&mut plot, &cx);
    assert!(events(&seen).iter().all(|e| e.0 != Event::Click));
}

#[test]
fn a_drag_cancels_a_pending_hover_pick() {
    let (cx, mut plot, seen) = gpu_fixture(Retain::GpuOnly);
    let at = row_at(&plot, 5);
    let button = Button::Primary;
    feed(&mut plot, Input::PointerDown { at, button });
    feed(
        &mut plot,
        Input::PointerMoved {
            at: off(at, 2.0, 0.0),
        },
    );
    assert!(Chart::needs_frame(&plot));
    feed(
        &mut plot,
        Input::PointerMoved {
            at: off(at, 12.0, 0.0),
        },
    );
    assert!(!Chart::needs_frame(&plot));
    plot.resolve(&cx, W, H).unwrap();
    settle(&mut plot, &cx);
    feed(
        &mut plot,
        Input::PointerUp {
            at: off(at, 12.0, 0.0),
            button,
        },
    );
    settle(&mut plot, &cx);
    assert_eq!(plot.hovered(), None);
    assert!(events(&seen).is_empty());
}

/// CPU and GPU picks hover the same row for the same input sequence.
#[test]
fn cpu_and_gpu_hover_agree() {
    let (cx, mut cpu, _) = gpu_fixture(Retain::Auto);
    let (cx2, mut gpu, _) = gpu_fixture(Retain::GpuOnly);
    let mut points = Vec::new();
    for i in 0..12 {
        let c = row_at(&cpu, i);
        points.extend([c, off(c, 6.0, 5.0), off(c, 14.0, 0.0), off(c, 18.0, 0.0)]);
    }
    points.push(empty_space(&cpu));
    for at in points {
        moved(&mut cpu, at);
        moved(&mut gpu, at);
        settle(&mut cpu, &cx);
        settle(&mut gpu, &cx2);
        assert_eq!(cpu.hovered(), gpu.hovered(), "at {at:?}");
    }
}
