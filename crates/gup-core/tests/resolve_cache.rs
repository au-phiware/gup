// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The resolve cache and wheel input (GUP-444): an unchanged plot resolves
//! for free, and every change the plot can see is a miss.

use gup_core::geom::Point;
use gup_core::prelude::*;
use gup_core::{Button, Input, Resolved, Response};

const W: f32 = 320.0;
const H: f32 = 200.0;

type Row = (f64, f64, f64);

struct Fixture {
    plot: Plot,
    x: ScaleRef<Linear>,
    y: ScaleRef<Linear>,
    colour: ScaleRef<Sequential>,
}

fn fixture() -> Fixture {
    fixture_with(Linear::new(), Linear::new())
}

/// y is fractional, so its ticks stay fractional (whole-number data tick in
/// whole steps) and the labels change width as the plot zooms.
fn fixture_with(xs: Linear, ys: Linear) -> Fixture {
    let mut plot = Plot::new();
    let (x, y) = (plot.x(xs), plot.y(ys));
    let colour = ScaleRef::new(Sequential::viridis());
    plot.add(Selection::<Row, Circle>::new(
        (0..50)
            .map(|i| {
                (
                    f64::from(i * 10),
                    f64::from(i * i % 9 * 10) + 0.5,
                    f64::from(i % 5),
                )
            })
            .collect::<Vec<Row>>(),
    ))
    .attr(Circle::X, x.encode(|r: &Row| r.0))
    .attr(Circle::Y, y.encode(|r: &Row| r.1))
    .attr(Circle::FILL, colour.encode(|r: &Row| r.2))
    .attr(Circle::RADIUS, Px(4.0));
    Fixture { plot, x, y, colour }
}

fn cx() -> Context {
    Context::new_blocking().expect("headless context")
}

fn same(a: &Resolved, b: &Resolved) -> bool {
    format!("{a:?}") == format!("{b:?}")
}

#[test]
fn unchanged_plot_resolves_for_free() {
    let cx = cx();
    let mut plot = fixture().plot;
    let first: *const Resolved = plot.resolve(&cx, W, H).unwrap();
    let generation = plot.resolve_generation();
    let stats = cx.upload_stats();
    let second: *const Resolved = plot.resolve(&cx, W, H).unwrap();
    assert!(std::ptr::eq(first, second));
    assert_eq!(plot.resolve_generation(), generation);
    assert_eq!(cx.upload_stats(), stats);
    assert!(plot.layout().is_some());
}

/// Resolve, apply `change`, resolve again: one more full resolve, and the
/// result differs when `reflected`.
fn misses(reflected: bool, change: impl FnOnce(&mut Fixture)) {
    let cx = cx();
    let mut f = fixture();
    let before = f.plot.resolve(&cx, W, H).unwrap().clone();
    let generation = f.plot.resolve_generation();
    change(&mut f);
    let after = f.plot.resolve(&cx, W, H).unwrap().clone();
    assert_eq!(f.plot.resolve_generation(), generation + 1);
    if reflected {
        assert!(!same(&before, &after), "the second resolve is the same");
    }
}

#[test]
fn every_plot_change_misses() {
    misses(true, |f| {
        f.plot.title("t");
    });
    misses(true, |f| {
        f.plot.x_title("x");
    });
    misses(true, |f| {
        f.plot.y_title("y");
    });
    misses(true, |f| {
        f.plot.theme(Theme {
            tick_len: Px(14.0),
            ..Theme::default()
        });
    });
    misses(true, |f| {
        f.plot.legends(false);
    });
    misses(true, |f| {
        f.plot
            .add(Selection::<Row, Circle>::new(vec![(100.0, 3.0, 1.0)]));
    });
    misses(true, |f| {
        f.plot
            .layer_mut::<Row, Circle>()
            .unwrap()
            .attr(Circle::RADIUS, Px(12.0));
    });
    misses(true, |f| {
        f.plot.zoom(Point::new(100.0, 100.0), 0.5).unwrap();
    });
    misses(true, |f| {
        let _ = f.plot.handle(Input::Wheel {
            at: Point::new(100.0, 100.0),
            lines: 1.0,
        });
    });
    misses(true, |f| f.x.write().set_domain(0.0, 10.0).unwrap());
    // A colour scale that only the layer's FILL encoding holds.
    misses(false, |f| {
        *f.colour.write() = Sequential::viridis().domain(0.0, 2.0);
    });
}

#[test]
fn a_different_size_or_context_misses() {
    let (cx, other) = (cx(), cx());
    let mut plot = fixture().plot;
    let a = plot.resolve(&cx, W, H).unwrap().clone();
    let g = plot.resolve_generation();
    let b = plot.resolve(&cx, W + 10.0, H).unwrap().clone();
    assert_eq!(plot.resolve_generation(), g + 1);
    assert_eq!(b.layout.width, W + 10.0);
    assert!(!same(&a, &b));
    plot.resolve(&other, W + 10.0, H).unwrap();
    assert_eq!(plot.resolve_generation(), g + 2);
}

#[test]
fn a_failed_resolve_clears_the_cache() {
    let cx = cx();
    let mut plot = fixture().plot;
    plot.resolve(&cx, W, H).unwrap();
    let g = plot.resolve_generation();
    plot.layer_mut::<Row, Circle>().unwrap().z(150);
    assert!(plot.resolve(&cx, W, H).is_err());
    assert!(plot.layout().is_none());
    plot.layer_mut::<Row, Circle>().unwrap().z(1);
    plot.resolve(&cx, W, H).unwrap();
    assert_eq!(plot.resolve_generation(), g + 1);
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(a.abs()).max(1.0)
}

/// `steps` wheel turns of two lines, each resolved: both domains shrink to
/// 0.85² and the values under the pointer stay put, from the first turn on.
/// The rows keep the y tick labels the same width: the plot rect follows
/// the label width, and a rect that moves a pixel moves the mapping with it.
fn wheel_steps(mut f: Fixture, steps: usize) -> Fixture {
    let cx = cx();
    let rect = f.plot.resolve(&cx, W, H).unwrap().layout.plot;
    let at = Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    let domain = |f: &Fixture| {
        (
            f.x.read().current_domain().unwrap(),
            f.y.read().current_domain().unwrap(),
        )
    };
    let under = |f: &Fixture| {
        (
            f.x.read().invert(f64::from(at.x)),
            f.y.read().invert(f64::from(at.y)),
        )
    };
    for step in 0..steps {
        let (dx, dy) = domain(&f);
        let (ux, uy) = under(&f);
        assert_eq!(
            f.plot.handle(Input::Wheel { at, lines: 2.0 }),
            Response { redraw: true }
        );
        f.plot.resolve(&cx, W, H).unwrap();
        let (ex, ey) = domain(&f);
        assert!(
            close(ex.1 - ex.0, (dx.1 - dx.0) * 0.85 * 0.85),
            "{step}: {dx:?} {ex:?}"
        );
        assert!(
            close(ey.1 - ey.0, (dy.1 - dy.0) * 0.85 * 0.85),
            "{step}: {dy:?} {ey:?}"
        );
        let (vx, vy) = under(&f);
        assert!(
            close(vx, ux) && close(vy, uy),
            "{step}: {ux} {vx} {uy} {vy}"
        );
    }
    f
}

fn fixed() -> Fixture {
    fixture_with(
        Linear::new().domain(0.0, 490.0),
        Linear::new().domain(0.0, 80.0),
    )
}

#[test]
fn first_wheel_step_does_not_jump() {
    wheel_steps(fixture(), 1);
    wheel_steps(fixed(), 1);
}

#[test]
fn wheel_keeps_the_value_under_the_pointer() {
    wheel_steps(fixture(), 3);
    wheel_steps(fixed(), 3);
}

#[test]
fn a_domain_set_after_a_zoom_gets_the_fixed_policy() {
    let cx = cx();
    let mut f = wheel_steps(fixture(), 1);
    let rect = f.plot.layout().unwrap().plot;
    f.x.write().set_domain(0.0, 490.0).unwrap();
    f.plot.resolve(&cx, W, H).unwrap();
    // A fixed domain ends `inset` short of the plot rect's right edge.
    assert!(f.x.read().eval(490.0) < f64::from(rect.right()) - 1.0);
}

#[test]
fn inputs_that_do_nothing() {
    let cx = cx();
    let mut f = fixture();
    let at = Point::new(130.0, 70.0);
    // Before the first resolve, a wheel does nothing.
    assert_eq!(
        f.plot.handle(Input::Wheel { at, lines: 1.0 }),
        Response::default()
    );
    let rect = f.plot.resolve(&cx, W, H).unwrap().layout.plot;
    let at = Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    let domain = |f: &Fixture| {
        (
            f.x.read().current_domain().unwrap(),
            f.y.read().current_domain().unwrap(),
        )
    };
    let (ex, ey) = domain(&f);
    // Nothing to do for these.
    let generation = f.plot.resolve_generation();
    for lines in [0.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            f.plot.handle(Input::Wheel { at, lines }),
            Response::default(),
            "{lines}"
        );
    }
    for input in [
        Input::PointerMoved { at },
        Input::PointerLeft,
        Input::PointerDown {
            at,
            button: Button::Primary,
        },
        Input::PointerUp {
            at,
            button: Button::Primary,
        },
    ] {
        assert_eq!(f.plot.handle(input), Response::default(), "{input:?}");
    }
    f.plot.resolve(&cx, W, H).unwrap();
    assert_eq!(f.plot.resolve_generation(), generation);
    assert_eq!(domain(&f), (ex, ey));
}

// ---- GUP-452: a stable plot rect during interaction ----

use gup_core::geom::Rect;
use gup_core::scene::{HAlign, TextRole};

/// The widest y tick label's ink width, and the leftmost ink edge of any.
fn y_labels(plot: &Plot) -> (f32, f32) {
    let layout = plot.layout().unwrap();
    let ys = layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::TickLabel && t.run.anchor.h == HAlign::End);
    ys.fold((0.0, f32::MAX), |(w, l), t| {
        (w.max(t.bounds.width), l.min(t.bounds.x))
    })
}

/// Explicit domains: `fixed` data with x in `(0, 100)` and y in `(0, 8)`.
fn held_fixture() -> Fixture {
    fixture_with(
        Linear::new().domain(0.0, 100.0),
        Linear::new().domain(0.0, 8.0),
    )
}

const ANCHOR: Point = Point::new(150.0, 120.0);

const SIZE: (f32, f32) = (400.0, 300.0);

fn resolve_at(f: &mut Fixture, cx: &Context, size: (f32, f32)) -> Rect {
    f.plot.resolve(cx, size.0, size.1).unwrap().layout.plot
}

fn wheel(f: &mut Fixture, lines: f32) {
    assert_eq!(
        f.plot.handle(Input::Wheel { at: ANCHOR, lines }),
        Response { redraw: true }
    );
}

/// A fresh plot with the domains `f` has now, resolved at `size`.
fn fresh_like(f: &Fixture, cx: &Context, size: (f32, f32)) -> Rect {
    let (dx, dy) = (
        f.x.read().current_domain().unwrap(),
        f.y.read().current_domain().unwrap(),
    );
    let mut g = fixture_with(
        Linear::new().domain(dx.0, dx.1),
        Linear::new().domain(dy.0, dy.1),
    );
    resolve_at(&mut g, cx, size)
}

fn bits(r: Rect) -> [u32; 4] {
    [r.x, r.y, r.width, r.height].map(f32::to_bits)
}

/// Wheel in six steps, then out six (anchor fixed): the value under the
/// pointer holds within 0.01 px and the plot rect never moves, though the y
/// labels change width. Without the hold (control) the rect moves.
#[test]
fn the_value_under_the_pointer_holds_across_label_width_changes() {
    let cx = cx();
    let theme_pad = Theme::default().pad.0;
    for control in [false, true] {
        let mut f = held_fixture();
        let first = resolve_at(&mut f, &cx, SIZE);
        let mut prev = first;
        let (mut widened, mut narrowed, mut moved) = (false, false, false);
        for lines in [1.0f32; 6].into_iter().chain([-1.0f32; 6]) {
            let (w0, _) = y_labels(&f.plot);
            let v = f.plot.invert(ANCHOR).unwrap();
            wheel(&mut f, lines);
            if control {
                assert_eq!(f.plot.handle(Input::PointerLeft), Response { redraw: true });
            }
            let rect = resolve_at(&mut f, &cx, SIZE);
            let (w1, left) = y_labels(&f.plot);
            widened |= w1 > w0;
            narrowed |= w1 < w0;
            assert!(w1 - w0 <= theme_pad, "{w0} {w1}");
            assert!(left >= 0.0, "{left}");
            if control {
                moved |= bits(rect) != bits(prev);
                prev = rect;
                continue;
            }
            assert_eq!(bits(rect), bits(first), "the rect moved");
            let u = f.plot.invert(ANCHOR).unwrap();
            let (dx, dy) = (
                f.x.read().current_domain().unwrap(),
                f.y.read().current_domain().unwrap(),
            );
            let ppu = (
                f64::from(rect.width) / (dx.1 - dx.0),
                f64::from(rect.height) / (dy.1 - dy.0),
            );
            assert!((u.0 - v.0).abs() * ppu.0 <= 0.01, "{v:?} {u:?}");
            assert!((u.1 - v.1).abs() * ppu.1 <= 0.01, "{v:?} {u:?}");
        }
        assert!(widened && narrowed, "the data must change label widths");
        assert!(!control || moved, "without the hold the rect should move");
    }
}

#[test]
fn the_hold_grows_for_a_label_that_cannot_fit() {
    let cx = cx();
    let mut f = held_fixture();
    let first = resolve_at(&mut f, &cx, SIZE);
    wheel(&mut f, 1.0);
    let held = resolve_at(&mut f, &cx, SIZE);
    assert_eq!(bits(held), bits(first));
    // Twenty lines at once: the labels grow past the padding.
    wheel(&mut f, 20.0);
    let grown = resolve_at(&mut f, &cx, SIZE);
    let (_, left) = y_labels(&f.plot);
    assert!(grown.x > held.x && left >= 0.0, "{grown:?} {left}");
    // Exactly what a fresh layout of those labels needs.
    assert_eq!(grown.x, fresh_like(&f, &cx, SIZE).x);
    // Grown margins stay grown (the hold only grows).
    wheel(&mut f, -1.0);
    assert!(resolve_at(&mut f, &cx, SIZE).x >= grown.x);
}

#[test]
fn the_hold_is_released_on_leave_and_resize() {
    let cx = cx();
    let mut f = held_fixture();
    resolve_at(&mut f, &cx, SIZE);
    for _ in 0..6 {
        wheel(&mut f, 1.0);
        resolve_at(&mut f, &cx, SIZE);
    }
    assert_ne!(
        bits(f.plot.layout().unwrap().plot),
        bits(fresh_like(&f, &cx, SIZE))
    );
    assert_eq!(f.plot.handle(Input::PointerLeft), Response { redraw: true });
    let released = resolve_at(&mut f, &cx, SIZE);
    assert_eq!(bits(released), bits(fresh_like(&f, &cx, SIZE)));
    // Nothing is held now.
    assert_eq!(f.plot.handle(Input::PointerLeft), Response::default());

    // Resize.
    wheel(&mut f, 1.0);
    resolve_at(&mut f, &cx, SIZE);
    let other = (360.0, 280.0);
    let resized = resolve_at(&mut f, &cx, other);
    assert_eq!(bits(resized), bits(fresh_like(&f, &cx, other)));
}

#[test]
fn zoom_called_directly_does_not_hold() {
    let cx = cx();
    let mut f = held_fixture();
    resolve_at(&mut f, &cx, SIZE);
    for _ in 0..6 {
        f.plot.zoom(ANCHOR, 0.85).unwrap();
        let rect = resolve_at(&mut f, &cx, SIZE);
        assert_eq!(bits(rect), bits(fresh_like(&f, &cx, SIZE)));
    }
}

#[test]
fn invert_is_none_before_resolve_and_outside_the_plot() {
    let cx = cx();
    let mut f = held_fixture();
    assert_eq!(f.plot.invert(ANCHOR), None);
    let r = resolve_at(&mut f, &cx, SIZE);
    assert!(f.plot.invert(ANCHOR).is_some());
    assert!(f.plot.invert(Point::new(r.left(), r.top())).is_some());
    assert!(f.plot.invert(Point::new(r.right(), r.bottom())).is_some());
    assert_eq!(f.plot.invert(Point::new(r.left() - 1.0, ANCHOR.y)), None);
    assert_eq!(f.plot.invert(Point::new(ANCHOR.x, r.bottom() + 1.0)), None);
}

// ---- GUP-453: drag pan, view axes and reset ----

use gup_core::{Axes, Drag, RowId};
use std::sync::{Arc, Mutex};

type Events = Arc<Mutex<Vec<&'static str>>>;

const PS: (f32, f32) = (400.0, 300.0);

/// A scatter with a tooltip and all three handlers, logging to `events`.
fn pan_fixture(xs: Linear, ys: Linear) -> (Fixture, Events) {
    let events = Events::default();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(xs), plot.y(ys));
    let colour = ScaleRef::new(Sequential::viridis());
    let sel = plot.add(Selection::<Row, Circle>::new(
        (0..50)
            .map(|i| {
                (
                    f64::from(i * 10 + 1),
                    f64::from(i * i % 9 * 10 + 1),
                    f64::from(i % 5),
                )
            })
            .collect::<Vec<Row>>(),
    ));
    sel.attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(Circle::FILL, colour.encode(|r: &Row| r.2))
        .attr(Circle::RADIUS, Px(6.0))
        .tooltip(|r| format!("{}", r.0));
    for (event, tag) in [
        (Event::Enter, "Enter"),
        (Event::Leave, "Leave"),
        (Event::Click, "Click"),
    ] {
        let events = Arc::clone(&events);
        sel.on(event, move |_, _| events.lock().unwrap().push(tag));
    }
    (Fixture { plot, x, y, colour }, events)
}

fn auto_pan() -> (Fixture, Events) {
    pan_fixture(Linear::new(), Linear::new())
}

fn explicit_pan() -> (Fixture, Events) {
    pan_fixture(
        Linear::new().domain(0.0, 500.0),
        Linear::new().domain(0.0, 100.0),
    )
}

fn take(e: &Events) -> Vec<&'static str> {
    std::mem::take(&mut *e.lock().unwrap())
}

fn press(f: &mut Fixture, at: Point) -> Response {
    f.plot.handle(Input::PointerDown {
        at,
        button: Button::Primary,
    })
}

fn release(f: &mut Fixture, at: Point) -> Response {
    f.plot.handle(Input::PointerUp {
        at,
        button: Button::Primary,
    })
}

fn move_to(f: &mut Fixture, at: Point) -> Response {
    f.plot.handle(Input::PointerMoved { at })
}

fn domains(f: &Fixture) -> [(f64, f64); 2] {
    [
        f.x.read().current_domain().unwrap(),
        f.y.read().current_domain().unwrap(),
    ]
}

/// The pixel of row 3's centre, and a point to press at (on it).
fn row_point(f: &mut Fixture) -> Point {
    f.plot.pick_anchor(RowId { layer: 0, row: 3 }).unwrap()
}

/// Drag from a row through `+(2, 0)`, `+(30, 20)` and `+(60, -10)`,
/// resolving after each move: the value under the pointer at the press
/// stays under it within 0.01 px, and the plot rect never moves.
fn drag_holds_the_value(
    f: &mut Fixture,
    events: &Events,
    y_domain: impl Fn() -> (f64, f64),
    y_eval: impl Fn(f64) -> f64,
) {
    let cx = cx();
    let rect = f.plot.resolve(&cx, PS.0, PS.1).unwrap().layout.plot;
    let p = row_point(f);
    let _ = move_to(f, p);
    assert_eq!(f.plot.hovered(), Some(RowId { layer: 0, row: 3 }));
    take(events);
    let v = f.plot.invert(p).unwrap();
    let before = (f.x.read().current_domain(), y_domain());
    assert_eq!(press(f, p), Response::default());
    // Within the slop: nothing moves, the row stays hovered.
    assert_eq!(move_to(f, Point::new(p.x + 2.0, p.y)), Response::default());
    assert_eq!((f.x.read().current_domain(), y_domain()), before);
    assert_eq!(f.plot.hovered(), Some(RowId { layer: 0, row: 3 }));
    for d in [(30.0, 20.0), (60.0, -10.0)] {
        let at = Point::new(p.x + d.0, p.y + d.1);
        assert_eq!(move_to(f, at), Response { redraw: true });
        assert_eq!(f.plot.hovered(), None);
        let r = f.plot.resolve(&cx, PS.0, PS.1).unwrap().layout.plot;
        assert_eq!(bits(r), bits(rect), "the plot rect moved");
        let (x_px, y_px) = (f.x.read().eval(v.0), y_eval(v.1));
        assert!((x_px - f64::from(at.x)).abs() <= 0.01, "x {x_px} {at:?}");
        assert!((y_px - f64::from(at.y)).abs() <= 0.01, "y {y_px} {at:?}");
    }
    assert_eq!(take(events), ["Leave"]);
    let end = Point::new(p.x + 60.0, p.y - 10.0);
    assert_eq!(release(f, end), Response::default());
    assert!(take(events).is_empty(), "the release clicked");
    // After the release, a move hovers again.
    let _ = move_to(f, end);
    assert!(f.plot.hovered().is_some());
}

#[test]
fn drag_pans_explicit_and_automatic_domains() {
    for (mut f, events) in [explicit_pan(), auto_pan()] {
        let y = f.y.clone();
        let y2 = y.clone();
        drag_holds_the_value(
            &mut f,
            &events,
            move || y.read().current_domain().unwrap(),
            move |v| y2.read().eval(v),
        );
    }
}

#[test]
fn drag_pans_a_log_axis() {
    let (mut f, events) = pan_fixture(Linear::new(), Linear::new());
    // A log y replaces the linear one.
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
    let sel = plot.add(Selection::<Row, Circle>::new(
        (0..50)
            .map(|i| (f64::from(i * 10 + 1), f64::from(i % 9 + 1) * 3.0, 0.0))
            .collect::<Vec<Row>>(),
    ));
    sel.attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(Circle::RADIUS, Px(6.0))
        .tooltip(|r| format!("{}", r.0));
    for (event, tag) in [
        (Event::Enter, "Enter"),
        (Event::Leave, "Leave"),
        (Event::Click, "Click"),
    ] {
        let events = Arc::clone(&events);
        sel.on(event, move |_, _| events.lock().unwrap().push(tag));
    }
    f.plot = plot;
    f.x = x;
    let y2 = y.clone();
    drag_holds_the_value(
        &mut f,
        &events,
        move || y.read().current_domain().unwrap(),
        move |v| y2.read().eval(v),
    );
}

#[test]
fn drag_keeps_the_widened_range_on_automatic_axes() {
    let cx = cx();
    let (mut f, _) = auto_pan();
    let rect = f.plot.resolve(&cx, PS.0, PS.1).unwrap().layout.plot;
    let p = Point::new(rect.x + 100.0, rect.y + 100.0);
    let _ = press(&mut f, p);
    let _ = move_to(&mut f, Point::new(p.x + 30.0, p.y + 20.0));
    let _ = release(&mut f, Point::new(p.x + 30.0, p.y + 20.0));
    let rect2 = f.plot.resolve(&cx, PS.0, PS.1).unwrap().layout.plot;
    assert_eq!(bits(rect), bits(rect2));
    let [dx, dy] = domains(&f);
    let (x, y) = (f.x.read(), f.y.read());
    assert!((x.eval(dx.0) - f64::from(rect.left())).abs() <= 0.01);
    assert!((x.eval(dx.1) - f64::from(rect.right())).abs() <= 0.01);
    assert!((y.eval(dy.0) - f64::from(rect.bottom())).abs() <= 0.01);
    assert!((y.eval(dy.1) - f64::from(rect.top())).abs() <= 0.01);
}

#[test]
fn view_axes_and_drag_choose_what_moves() {
    let cx = cx();
    let wheel_at = |f: &mut Fixture, p: Point| f.plot.handle(Input::Wheel { at: p, lines: 2.0 });
    let drag = |f: &mut Fixture, p: Point| {
        let _ = press(f, p);
        let _ = move_to(f, Point::new(p.x + 30.0, p.y + 20.0));
        let _ = move_to(f, Point::new(p.x + 60.0, p.y - 10.0));
        let _ = release(f, Point::new(p.x + 60.0, p.y - 10.0));
    };
    for (axes, moves) in [
        (Axes::X, [true, false]),
        (Axes::Y, [false, true]),
        (Axes::Neither, [false, false]),
        (Axes::Both, [true, true]),
    ] {
        let (mut f, _) = explicit_pan();
        f.plot.view_axes(axes);
        let rect = f.plot.resolve(&cx, PS.0, PS.1).unwrap().layout.plot;
        let p = Point::new(rect.x + 100.0, rect.y + 100.0);
        let before = domains(&f);
        drag(&mut f, p);
        let r = wheel_at(&mut f, p);
        assert_eq!(r.redraw, moves[0] || moves[1], "{axes:?}");
        let after = domains(&f);
        for i in 0..2 {
            assert_eq!(after[i] != before[i], moves[i], "{axes:?} axis {i}");
        }
        if !moves[1] {
            assert_eq!(after[1].0.to_bits(), before[1].0.to_bits());
            assert_eq!(after[1].1.to_bits(), before[1].1.to_bits());
        }
    }
    let (mut f, events) = explicit_pan();
    f.plot.drag(Drag::Off);
    let rect = f.plot.resolve(&cx, PS.0, PS.1).unwrap().layout.plot;
    let p = Point::new(rect.x + 100.0, rect.y + 100.0);
    let before = domains(&f);
    drag(&mut f, p);
    assert_eq!(domains(&f), before);
    assert!(take(&events).is_empty());
}

#[test]
fn input_leaves_a_band_axis_alone() {
    let cx = cx();
    let mut plot = Plot::new();
    plot.legends(false);
    let (x, y) = (plot.x(Band::new()), plot.y(Linear::new()));
    plot.add(Selection::<(String, f64), Circle>::new(
        (0..12)
            .map(|i| (format!("k{}", i % 4), f64::from(i) * 3.0))
            .collect::<Vec<_>>(),
    ))
    .attr(Circle::X, x.encode_key(|r: &(String, f64)| r.0.as_str()))
    .attr(Circle::Y, y.encode(|r: &(String, f64)| r.1))
    .attr(Circle::RADIUS, Px(5.0));
    let rect = plot.resolve(&cx, PS.0, PS.1).unwrap().layout.plot;
    let p = Point::new(rect.x + 100.0, rect.y + 100.0);
    let (bx, by) = (x.read().current_domain(), y.read().current_domain());
    let r = plot.handle(Input::Wheel { at: p, lines: 2.0 });
    assert_eq!(r, Response { redraw: true });
    let (zy, zx) = (y.read().current_domain(), x.read().current_domain());
    assert_ne!(zy, by);
    assert_eq!(zx, bx);
    let _ = plot.handle(Input::PointerDown {
        at: p,
        button: Button::Primary,
    });
    let r = plot.handle(Input::PointerMoved {
        at: Point::new(p.x + 30.0, p.y + 20.0),
    });
    assert_eq!(r, Response { redraw: true });
    assert_ne!(y.read().current_domain(), zy);
    assert_eq!(x.read().current_domain(), bx);
    // Called directly, a band axis is an error.
    assert!(plot.pan(5.0, 5.0).is_err());
}

#[test]
fn reset_view_returns_to_the_first_view() {
    let cx = cx();
    let (mut f, _) = auto_pan();
    let first = f.plot.resolve(&cx, PS.0, PS.1).unwrap().clone();
    let first_rgba = f.plot.render(&cx, PS.0 as u32, PS.1 as u32).unwrap();
    let p = Point::new(first.layout.plot.x + 120.0, first.layout.plot.y + 90.0);
    for _ in 0..2 {
        let _ = f.plot.handle(Input::Wheel { at: p, lines: 2.0 });
        f.plot.resolve(&cx, PS.0, PS.1).unwrap();
    }
    let _ = press(&mut f, p);
    let _ = move_to(&mut f, Point::new(p.x + 30.0, p.y + 20.0));
    let _ = release(&mut f, Point::new(p.x + 30.0, p.y + 20.0));
    f.plot.resolve(&cx, PS.0, PS.1).unwrap();
    f.plot.reset_view();
    let again = f.plot.resolve(&cx, PS.0, PS.1).unwrap();
    assert_eq!(again.layout, first.layout);
    assert!(f.x.read().is_auto() && f.y.read().is_auto());
    assert_eq!(
        f.plot.render(&cx, PS.0 as u32, PS.1 as u32).unwrap(),
        first_rgba
    );

    // Explicit domains come back bitwise.
    let (mut g, _) = explicit_pan();
    g.plot.resolve(&cx, PS.0, PS.1).unwrap();
    let before = domains(&g);
    let _ = g.plot.handle(Input::Wheel { at: p, lines: 2.0 });
    g.plot.pan(10.0, 10.0).unwrap();
    assert_ne!(domains(&g), before);
    g.plot.reset_view();
    g.plot.resolve(&cx, PS.0, PS.1).unwrap();
    assert_eq!(domains(&g), before);
    assert!(!g.x.read().is_auto());

    // No view change: a no-op that keeps the cache.
    let generation = g.plot.resolve_generation();
    g.plot.reset_view();
    g.plot.resolve(&cx, PS.0, PS.1).unwrap();
    assert_eq!(g.plot.resolve_generation(), generation);
}
