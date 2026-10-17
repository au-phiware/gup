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
