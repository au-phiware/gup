// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-619: keyboard focus over rows, from synthetic key input.

mod common;

use common::vr::{harness, layout_metadata};
use gup_core::prelude::*;
use gup_core::scene::{ItemKind, Z_FOCUS, Z_TOOLTIP};
use gup_core::{BoxPlotChart, Button, EventInfo, ImageTarget, Input, Key, Modifiers, RowId};
use gup_visual_regression::RgbaImage;
use std::sync::{Arc, Mutex};

const W: f32 = 480.0;
const H: f32 = 320.0;

type Log = Arc<Mutex<Vec<String>>>;

fn row(layer: usize, row: usize) -> RowId {
    RowId { layer, row }
}

fn key(plot: &mut Plot, k: Key) -> bool {
    plot.handle(Input::key(k)).redraw
}

/// Five points whose rows are not in x order.
const XS: [f64; 5] = [30.0, 10.0, 50.0, 20.0, 40.0];
const YS: [f64; 5] = [3.0, 7.0, 4.0, 8.0, 2.0];

fn scatter() -> (Context, Plot, Log) {
    let cx = Context::new_blocking().expect("headless GPU context");
    let log: Log = Log::default();
    let mut plot = Plot::new();
    let rows: Vec<usize> = (0..5).collect();
    let l = Arc::clone(&log);
    let handler = move |tag: &'static str| {
        let l = Arc::clone(&l);
        move |info: &EventInfo, r: &usize| {
            l.lock().unwrap().push(format!("{tag}({r})"));
            assert_eq!(info.row.row, *r);
        }
    };
    plot.circles(rows)
        .x(|r| XS[*r])
        .y(|r| YS[*r])
        .radius(Px(6.0))
        .tooltip(|r| format!("row {r}"))
        .on(Event::Enter, handler("Enter"))
        .on(Event::Leave, handler("Leave"))
        .on(Event::Click, handler("Click"));
    plot.resolve(&cx, W, H).unwrap();
    (cx, plot, log)
}

fn take(l: &Log) -> Vec<String> {
    std::mem::take(&mut *l.lock().unwrap())
}

#[test]
fn right_visits_rows_in_x_order_and_stops_at_the_end() {
    let (_cx, mut plot, log) = scatter();
    let mut seen = Vec::new();
    for _ in 0..5 {
        assert!(key(&mut plot, Key::Right));
        seen.push(plot.focused().unwrap().row);
        assert_eq!(plot.hovered(), plot.focused());
    }
    assert_eq!(seen, [1, 3, 0, 4, 2]);
    // At the end: no wrap, nothing changes.
    assert!(!key(&mut plot, Key::Right));
    assert_eq!(plot.focused(), Some(row(0, 2)));
    assert_eq!(
        take(&log),
        [
            "Enter(1)", "Leave(1)", "Enter(3)", "Leave(3)", "Enter(0)", "Leave(0)", "Enter(4)",
            "Leave(4)", "Enter(2)"
        ]
    );
    for want in [4, 0, 3, 1] {
        assert!(key(&mut plot, Key::Left));
        assert_eq!(plot.focused(), Some(row(0, want)));
    }
    assert!(!key(&mut plot, Key::Left));
    assert!(key(&mut plot, Key::End));
    assert_eq!(plot.focused(), Some(row(0, 2)));
    assert!(key(&mut plot, Key::Home));
    assert_eq!(plot.focused(), Some(row(0, 1)));
    // With one layer, Up and Down stay.
    assert!(!key(&mut plot, Key::Up));
    assert!(!key(&mut plot, Key::Down));
}

#[test]
fn with_no_focus_the_keys_take_the_first_row() {
    for k in [Key::Right, Key::Left, Key::Home, Key::Up, Key::Down] {
        let (_cx, mut plot, _) = scatter();
        assert!(key(&mut plot, k), "{k:?}");
        assert_eq!(plot.focused(), Some(row(0, 1)), "{k:?}");
    }
}

#[test]
fn focus_shows_the_tooltip_and_the_ring_at_the_anchor() {
    let (cx, mut plot, _) = scatter();
    assert!(key(&mut plot, Key::Right));
    let a = plot.pick_anchor(row(0, 1)).unwrap();
    let resolved = plot.resolve(&cx, W, H).unwrap();
    let tip = resolved.layout.tooltip.expect("a tooltip");
    assert_eq!(tip.left(), (a.x + 8.0).round());
    let ring = resolved.layout.focus.expect("a ring");
    assert_eq!((ring.width, ring.height), (18.0, 18.0));
    assert_eq!(ring.left() + 9.0, a.x.round());
    assert_eq!(ring.top() + 9.0, a.y.round());
    let z: Vec<i32> = resolved.scene.items.iter().map(|i| i.z).collect();
    let ring_at = z.iter().position(|&z| z == Z_FOCUS).expect("a ring item");
    assert!(z.contains(&Z_TOOLTIP) && Z_FOCUS < Z_TOOLTIP);
    let ItemKind::Rects(rects) = &resolved.scene.items[ring_at].kind else {
        panic!("the ring is rectangles");
    };
    assert_eq!(rects.len(), 8);
    assert!(rects.iter().all(|r| r.rect.x.fract() == 0.0));
    // Escape clears it, and the hover.
    assert!(key(&mut plot, Key::Escape));
    assert_eq!(plot.focused(), None);
    assert_eq!(plot.hovered(), None);
    let resolved = plot.resolve(&cx, W, H).unwrap();
    assert!(resolved.layout.focus.is_none() && resolved.layout.tooltip.is_none());
}

#[test]
fn a_pointer_hover_draws_no_ring_and_the_pointer_ends_focus() {
    let (cx, mut plot, log) = scatter();
    let a2 = plot.pick_anchor(row(0, 2)).unwrap();
    assert!(plot.handle(Input::pointer_moved(a2)).redraw);
    assert_eq!(plot.hovered(), Some(row(0, 2)));
    assert_eq!(plot.focused(), None);
    assert!(plot.resolve(&cx, W, H).unwrap().layout.focus.is_none());
    take(&log);
    // The keyboard takes over a row the pointer holds: no events, a ring.
    assert!(key(&mut plot, Key::End));
    assert_eq!(plot.focused(), Some(row(0, 2)));
    assert!(take(&log).is_empty());
    assert!(plot.resolve(&cx, W, H).unwrap().layout.focus.is_some());
    // A pointer move within the same row leaves focus alone.
    let _ = plot.handle(Input::pointer_moved((a2.x + 1.0, a2.y)));
    assert_eq!(plot.focused(), Some(row(0, 2)));
    // A move onto another row ends it.
    let a0 = plot.pick_anchor(row(0, 0)).unwrap();
    assert!(plot.handle(Input::pointer_moved(a0)).redraw);
    assert_eq!(plot.focused(), None);
    assert_eq!(plot.hovered(), Some(row(0, 0)));
    // So does a move off the rows.
    key(&mut plot, Key::Home);
    assert!(plot.focused().is_some());
    let _ = plot.handle(Input::pointer_moved((1.0, 1.0)));
    assert_eq!(plot.focused(), None);
    assert_eq!(plot.hovered(), None);
}

#[test]
fn enter_clicks_and_escape_clears() {
    let (_cx, mut plot, log) = scatter();
    assert!(!key(&mut plot, Key::Enter));
    assert!(!key(&mut plot, Key::Escape));
    assert!(take(&log).is_empty());
    key(&mut plot, Key::Right);
    take(&log);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s = Arc::clone(&seen);
    plot.on_row(move |info| s.lock().unwrap().push((info.event, info.row, info.at)));
    let _ = key(&mut plot, Key::Enter);
    assert_eq!(take(&log), ["Click(1)"]);
    let a = plot.pick_anchor(row(0, 1)).unwrap();
    assert_eq!(*seen.lock().unwrap(), [(Event::Click, row(0, 1), a)]);
    assert_eq!(plot.focused(), Some(row(0, 1)));
    assert!(key(&mut plot, Key::Escape));
    assert_eq!(take(&log), ["Leave(1)"]);
    assert!(!key(&mut plot, Key::Escape));
}

#[test]
fn keys_do_nothing_before_resolve_without_rows_or_with_modifiers_or_mid_drag() {
    let mut plot = Plot::new();
    plot.circles(vec![0usize, 1])
        .x(|r| *r as f64)
        .y(|r| *r as f64);
    for k in [Key::Right, Key::Home, Key::Enter, Key::Escape] {
        assert!(!key(&mut plot, k));
    }
    assert_eq!(plot.focused(), None);

    // Nothing focusable.
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    plot.circles(vec![0usize, 1])
        .x(|r| *r as f64)
        .y(|r| *r as f64)
        .focusable(false);
    plot.resolve(&cx, W, H).unwrap();
    assert!(!key(&mut plot, Key::Right));
    assert_eq!(plot.focused(), None);

    // Modifiers: Shift-arrows are not ours.
    let (_cx, mut plot, _) = scatter();
    let shifted = Input::key_with(Key::Right, Modifiers::SHIFT);
    assert!(!plot.handle(shifted).redraw);
    assert_eq!(plot.focused(), None);
    // Zoom keys do nothing yet.
    assert!(!key(&mut plot, Key::ZoomIn));

    // A drag in progress ignores keys.
    let (_cx, mut plot, _) = scatter();
    let _ = plot.handle(Input::pointer_down((100.0, 100.0), Button::Primary));
    let _ = plot.handle(Input::pointer_moved((140.0, 120.0)));
    assert!(!key(&mut plot, Key::Right));
    assert_eq!(plot.focused(), None);
}

#[test]
fn rows_outside_the_view_are_not_visited_and_a_zoom_clears_focus() {
    let (cx, mut plot, _) = scatter();
    key(&mut plot, Key::Home);
    assert!(plot.focused().is_some());
    // Zoom in about the leftmost row: the right-hand rows leave the plot.
    let at = plot.pick_anchor(row(0, 1)).unwrap();
    assert!(plot.handle(Input::wheel(at, 4.0)).redraw);
    assert_eq!(plot.focused(), None);
    plot.resolve(&cx, W, H).unwrap();
    let rect = plot.layout().unwrap().plot;
    let mut visited = Vec::new();
    while key(&mut plot, Key::Right) {
        let id = plot.focused().unwrap();
        let a = plot.pick_anchor(id).unwrap();
        assert!(a.x >= rect.left() && a.x <= rect.right(), "{id:?} {a:?}");
        visited.push(id.row);
    }
    assert!(!visited.is_empty() && visited.len() < 5, "{visited:?}");
}

#[test]
fn a_line_chart_walks_series_with_up_and_down() {
    let cx = Context::new_blocking().unwrap();
    // Series A at x 0, 10, 20, 30; B at 4, 14, 24; C at 0, 10, 20, 30.
    let mut rows: Vec<(&'static str, f64, f64)> = Vec::new();
    for x in [0.0, 10.0, 20.0, 30.0] {
        rows.push(("A", x, 1.0 + x / 10.0));
    }
    for x in [4.0, 14.0, 24.0] {
        rows.push(("B", x, 5.0 - x / 10.0));
    }
    for x in [0.0, 10.0, 20.0, 30.0] {
        rows.push(("C", x, 8.0));
    }
    let mut chart = gup_core::line(rows).x(|r| r.1).y(|r| r.2).color(|r| r.0);
    chart.resolve(&cx, W, H).unwrap();
    assert!(chart.handle(Input::key(Key::Right)).redraw);
    assert!(chart.handle(Input::key(Key::Right)).redraw);
    assert!(chart.handle(Input::key(Key::Right)).redraw);
    assert_eq!(chart.plot_mut().focused(), Some(row(0, 2)));
    // Up: series B, nearest x to 20 is 24.
    assert!(chart.handle(Input::key(Key::Up)).redraw);
    assert_eq!(chart.plot_mut().focused(), Some(row(1, 2)));
    // Up again: series C, nearest to 24 is 20.
    assert!(chart.handle(Input::key(Key::Up)).redraw);
    assert_eq!(chart.plot_mut().focused(), Some(row(2, 2)));
    assert!(!chart.handle(Input::key(Key::Up)).redraw);
    // Every row of a line is reached, the last vertex too.
    assert!(chart.handle(Input::key(Key::End)).redraw);
    assert_eq!(chart.plot_mut().focused(), Some(row(2, 3)));
    assert!(chart.handle(Input::key(Key::Down)).redraw);
    assert_eq!(chart.plot_mut().focused(), Some(row(1, 2)));
    // Nearest to 24 in A is 20.
    assert!(chart.handle(Input::key(Key::Down)).redraw);
    assert_eq!(chart.plot_mut().focused(), Some(row(0, 2)));
    assert!(!chart.handle(Input::key(Key::Down)).redraw);
    let mut seen = Vec::new();
    assert!(chart.handle(Input::key(Key::Home)).redraw);
    seen.push(chart.plot_mut().focused().unwrap().row);
    while chart.handle(Input::key(Key::Right)).redraw {
        seen.push(chart.plot_mut().focused().unwrap().row);
    }
    assert_eq!(seen, [0, 1, 2, 3]);
}

#[test]
fn a_grouped_bar_chart_walks_bands_and_groups() {
    let cx = Context::new_blocking().unwrap();
    let data = [
        ("Q1", "north", 3.0),
        ("Q2", "north", 4.0),
        ("Q3", "north", 2.0),
        ("Q1", "south", 5.0),
        ("Q2", "south", 1.0),
        ("Q3", "south", 6.0),
    ];
    let mut chart = gup_core::bar(data).x(|r| r.0).y(|r| r.2).group(|r| r.1);
    chart.resolve(&cx, W, H).unwrap();
    let mut visited = Vec::new();
    while chart.handle(Input::key(Key::Right)).redraw {
        visited.push(chart.plot_mut().focused().unwrap());
    }
    // Three bars in the first layer, then it stays.
    assert_eq!(visited.len(), 3);
    assert!(visited.iter().all(|r| r.layer == visited[0].layer));
    assert!(chart.handle(Input::key(Key::Up)).redraw);
    let up = chart.plot_mut().focused().unwrap();
    assert_eq!(up.layer, visited[0].layer + 1);
    assert_eq!(up.row, visited[2].row, "the nearest x is the last band's");
}

#[test]
fn a_box_plot_focuses_boxes_and_outliers_only() {
    let cx = Context::new_blocking().unwrap();
    let mut data: Vec<(&'static str, f64)> = Vec::new();
    for c in ["a", "b", "c"] {
        for v in 10..=19 {
            data.push((c, f64::from(v)));
        }
    }
    data.push(("b", 60.0));
    let mut chart = gup_core::boxplot(data).x(|r| r.0).y(|r| r.1);
    chart.resolve(&cx, W, H).unwrap();
    let mut visited = Vec::new();
    let step = |chart: &mut BoxPlotChart<_>, k, visited: &mut Vec<RowId>| {
        let r = chart.handle(Input::key(k)).redraw;
        if r {
            visited.push(chart.plot_mut().focused().unwrap());
        }
        r
    };
    for _ in 0..3 {
        assert!(step(&mut chart, Key::Right, &mut visited));
    }
    assert!(!step(&mut chart, Key::Right, &mut visited));
    let boxes: Vec<usize> = visited.iter().map(|r| r.row).collect();
    assert_eq!(boxes, [0, 1, 2]);
    let box_layer = visited[0].layer;
    assert!(visited.iter().all(|r| r.layer == box_layer));
    // Up skips the whiskers, caps and medians to the outliers.
    assert!(step(&mut chart, Key::Up, &mut visited));
    let outlier = *visited.last().unwrap();
    assert!(outlier.layer > box_layer + 1, "{outlier:?}");
    assert!(!step(&mut chart, Key::Up, &mut visited));
    assert!(step(&mut chart, Key::Down, &mut visited));
    assert_eq!(visited.last().unwrap().layer, box_layer);
}

fn luminance(c: Color) -> f64 {
    let [r, g, b, _] = c.to_rgba8();
    let lin = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn contrast(a: Color, b: Color) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The ring's ink (the title colour) against the page (its edge colour),
/// in the light and the dark theme.
#[test]
fn the_ring_meets_3_to_1_in_both_themes() {
    for theme in [Theme::default(), Theme::dark()] {
        let c = contrast(theme.title, theme.background);
        assert!(c >= 3.0, "{c}");
    }
}

fn render(cx: &Context, plot: &mut Plot) -> (image::RgbaImage, gup_core::Layout) {
    let resolved = plot.resolve(cx, W, H).unwrap();
    let layout = resolved.layout.clone();
    let image = ImageTarget::new(cx, W as u32, H as u32)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap();
    (image, layout)
}

#[test]
fn keyboard_focus_golden() {
    let (cx, mut plot, _) = scatter();
    for _ in 0..3 {
        key(&mut plot, Key::Right);
    }
    // The third in x order is row 0.
    assert_eq!(plot.focused(), Some(row(0, 0)));
    let (image, layout) = render(&cx, &mut plot);
    let vr = RgbaImage::new(W as u32, H as u32, image.into_raw()).unwrap();
    harness()
        .run(
            "gup_core/keyboard_focus",
            Ok((vr, layout_metadata(&layout))),
        )
        .assert_ok();
}

/// The dark theme's ring: its pixels, and a PNG to read by eye when
/// `GUP_KEYBOARD_PNG` names a path.
#[test]
fn the_ring_is_drawn_in_the_dark_theme() {
    let (cx, mut plot, _) = scatter();
    plot.theme(Theme::dark());
    key(&mut plot, Key::Right);
    let (image, layout) = render(&cx, &mut plot);
    let ring = layout.focus.unwrap();
    let theme = Theme::dark();
    let px = |x: f32, y: f32| image.get_pixel(x as u32, y as u32).0;
    let mid = ring.top() + 9.0;
    assert_eq!(px(ring.left() + 1.0, mid), theme.title.to_rgba8());
    assert_eq!(px(ring.left(), mid), theme.background.to_rgba8());
    if let Ok(path) = std::env::var("GUP_KEYBOARD_PNG") {
        image.save(path).unwrap();
    }
}
