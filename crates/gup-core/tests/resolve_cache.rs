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

fn fixture_with(xs: Linear, ys: Linear) -> Fixture {
    let mut plot = Plot::new();
    let (x, y) = (plot.x(xs), plot.y(ys));
    let colour = ScaleRef::new(Sequential::viridis());
    plot.add(Selection::<Row, Circle>::new(
        (0..50)
            .map(|i| {
                (
                    f64::from(i * 10),
                    f64::from(i * i % 9 * 10),
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
