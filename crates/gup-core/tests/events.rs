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
