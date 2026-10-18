// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GPU pick against the CPU pick (the oracle), on layers the CPU
//! cannot read, per hit shape, and its cost.

use crate::geom::Point;
use crate::prelude::*;
use crate::{Context, RowId};

const W: f32 = 400.0;
const H: f32 = 300.0;

#[derive(Clone, Debug)]
struct P {
    x: f64,
    y: f64,
    /// A size, a second x, or the y a null row would have had.
    v: f64,
    w: f64,
}

/// A deterministic generator of floats in `0..1`.
fn rng(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn base() -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>) {
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, 10.0));
    let y = plot.y(Linear::new().domain(0.0, 10.0));
    (plot, x, y)
}

fn px(x: &ScaleRef<Linear>, y: &ScaleRef<Linear>, a: f64, b: f64) -> Point {
    Point::new(x.read().eval(a) as f32, y.read().eval(b) as f32)
}

/// Outside the hole around (5, 5) that the null row's neighbourhood keeps
/// free of other marks.
fn clear_of_hole(x: f64, y: f64) -> bool {
    (x - 5.0).abs() > 1.0 || (y - 5.0).abs() > 1.0
}

/// The oracle's scene: overlapping circles (sized, so drawn largest
/// first) and segments at z 0, rects at z 1, and a line with a null row at
/// z 2. Returns the plot, its scales and the null row's would-be point.
fn oracle_scene(cx: &Context) -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>, (f64, f64)) {
    let mut next = rng(0x2545_f491_4f6c_dd1d);
    let (mut plot, x, y) = base();
    let points = |n: usize, next: &mut dyn FnMut() -> f64| {
        let mut rows = Vec::with_capacity(n);
        while rows.len() < n {
            let (a, b) = (next() * 10.0, next() * 10.0);
            if clear_of_hole(a, b) {
                rows.push(P {
                    x: a,
                    y: b,
                    v: next() * 10.0,
                    w: next(),
                });
            }
        }
        rows
    };
    let circles = points(800, &mut next);
    let size = ScaleRef::new(Pow::sqrt().domain(0.0, 10.0).range(Px(1.5), Px(7.0)));
    plot.add(Selection::<P, Circle>::new(circles))
        .attr(Circle::X, x.encode(|r: &P| r.x))
        .attr(Circle::Y, y.encode(|r: &P| r.y))
        .attr(Circle::RADIUS, size.encode(|r: &P| r.v));
    let segments: Vec<P> = points(400, &mut next)
        .into_iter()
        .map(|mut r| {
            // A short stroke from (x, y) towards (x ± 0.4, y ± 0.4).
            r.v = (r.x + (r.w - 0.5) * 0.8).clamp(0.0, 10.0);
            r.w = (r.y + (r.v.fract() - 0.5) * 0.8).clamp(0.0, 10.0);
            r
        })
        .filter(|r| clear_of_hole(r.v, r.w))
        .collect();
    plot.add(Selection::<P, Segment>::new(segments))
        .attr(Segment::X, x.encode(|r: &P| r.x))
        .attr(Segment::Y, y.encode(|r: &P| r.y))
        .attr(Segment::X_END, x.encode(|r: &P| r.v))
        .attr(Segment::Y_END, y.encode(|r: &P| r.w))
        .attr(Segment::WIDTH, Px(2.0));
    let rects: Vec<P> = points(400, &mut next)
        .into_iter()
        .map(|mut r| {
            r.v = (r.x + 0.1 + r.w * 0.3).min(10.0);
            r.w = (r.y + 0.1 + r.v.fract() * 0.3).min(10.0);
            r
        })
        .filter(|r| clear_of_hole(r.v, r.w) && clear_of_hole(r.v, r.y) && clear_of_hole(r.x, r.w))
        .collect();
    let mut sel = Selection::<P, crate::marks::Rect>::new(rects);
    sel.attr(crate::marks::Rect::X, x.encode(|r: &P| r.x))
        .attr(crate::marks::Rect::X_END, x.encode(|r: &P| r.v))
        .attr(crate::marks::Rect::Y, y.encode(|r: &P| r.y))
        .attr(crate::marks::Rect::Y_END, y.encode(|r: &P| r.w))
        .z(1);
    plot.add(sel);
    // A line left to right, with a gap of 0.5 either side of x = 5 but
    // for the null row there.
    let mut xs: Vec<f64> = (0..400)
        .map(|_| next() * 10.0)
        .filter(|v| (v - 5.0).abs() > 0.5)
        .collect();
    xs.push(5.0);
    xs.sort_by(f64::total_cmp);
    let null = (5.0, 5.0);
    let line: Vec<P> = xs
        .iter()
        .map(|&v| P {
            x: v,
            y: if v == 5.0 { f64::NAN } else { next() * 10.0 },
            v: 0.0,
            w: 0.0,
        })
        .collect();
    let mut sel = Selection::<P, Line>::new(line);
    sel.attr(Line::X, x.encode(|r: &P| r.x))
        .attr(Line::Y, y.encode(|r: &P| r.y))
        .attr(Line::WIDTH, Px(1.5))
        .z(2);
    plot.add(sel);
    plot.resolve(cx, W, H).unwrap();
    (plot, x, y, null)
}

/// Whether a disagreement at `at` is a near tie: the CPU's best distance
/// is within 0.01 px of the tolerance or of 0, or the two answers'
/// distances (or a line's two ends) differ by less than 0.01 px.
fn near_tie(plot: &mut Plot, at: Point, cpu: Option<RowId>, gpu: Option<RowId>) -> String {
    let hits = plot.pick_index().unwrap().hits_at(at, 3.0);
    let best = |row: Option<RowId>| {
        hits.iter()
            .filter(|h| Some(h.1) == row)
            .map(|h| h.0)
            .fold(f32::INFINITY, f32::min)
    };
    let (dc, dg) = (best(cpu), best(gpu));
    let tol = 0.01;
    let mut why = Vec::new();
    if [dc, dg]
        .iter()
        .any(|d| (d - 3.0).abs() < tol || d.abs() < tol)
    {
        why.push(format!("edge (cpu d {dc}, gpu d {dg})"));
    }
    if (dc - dg).abs() < tol {
        why.push(format!("equal distances ({dc}, {dg})"));
    }
    if hits.iter().any(|h| h.2.is_some_and(|g| g < tol)) {
        why.push("a line's ends are equally near".into());
    }
    why.join("; ")
}

#[test]
fn agrees_with_cpu_pick() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y, null) = oracle_scene(&cx);
    let rect = plot.layout().unwrap().plot;
    let mut next = rng(0x9e37_79b9_7f4a_7c15);
    let n = 10_000;
    let (mut agree, mut hits) = (0, 0);
    for _ in 0..n {
        let at = Point::new(
            rect.x + next() as f32 * rect.width,
            rect.y + next() as f32 * rect.height,
        );
        let cpu = plot.pick_row(at);
        let gpu = plot.pick_row_gpu(&cx, at).unwrap();
        hits += usize::from(cpu.is_some());
        if cpu == gpu {
            agree += 1;
            continue;
        }
        let why = near_tie(&mut plot, at, cpu, gpu);
        println!("disagreement at {at:?}: cpu {cpu:?}, gpu {gpu:?}: {why}");
        assert!(
            !why.is_empty(),
            "not a near tie at {at:?}: cpu {cpu:?}, gpu {gpu:?}; within 6 px: {:?}",
            plot.pick_index().unwrap().hits_at(at, 6.0)
        );
    }
    let rate = agree as f64 / f64::from(n);
    println!(
        "agreement {agree}/{n} ({:.4}%), {hits} CPU hits",
        rate * 100.0
    );
    assert!(rate >= 0.999, "{rate}");
    assert!(hits > n as usize / 2, "the scene should be dense: {hits}");

    // Around the null row: nothing on either path, away from other marks.
    let c = px(&x, &y, null.0, null.1);
    let mut checked = 0;
    for i in 0..400 {
        let (r, t) = (3.0 * next() as f32, i as f32 * 0.7);
        let at = Point::new(c.x + r * t.cos(), c.y + r * t.sin());
        if !plot.pick_index().unwrap().hits_at(at, 3.5).is_empty() {
            continue;
        }
        checked += 1;
        assert_eq!(plot.pick_row(at), None, "{at:?}");
        assert_eq!(plot.pick_row_gpu(&cx, at).unwrap(), None, "{at:?}");
    }
    assert!(checked > 300, "{checked}");

    // 1 px outside the plot rect: nothing, and no GPU work.
    let submissions = cx.submissions();
    for at in [
        Point::new(rect.x - 1.0, rect.y + 10.0),
        Point::new(rect.x + rect.width + 1.0, rect.y + 10.0),
        Point::new(rect.x + 10.0, rect.y - 1.0),
        Point::new(rect.x + 10.0, rect.y + rect.height + 1.0),
    ] {
        assert_eq!(plot.pick_row(at), None);
        assert_eq!(plot.pick_row_gpu(&cx, at).unwrap(), None);
    }
    assert_eq!(cx.submissions(), submissions);
}

#[test]
fn gpu_only_layers_are_picked_on_the_gpu() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<P> = (0..12)
        .map(|i| P {
            x: 0.5 + f64::from(i) * 0.8,
            y: 2.0 + f64::from(i % 5),
            v: 0.0,
            w: 0.0,
        })
        .collect();
    let (mut plot, x, y) = base();
    let mut sel = Selection::<P, Circle>::new(rows.clone());
    sel.attr(Circle::X, x.encode(|r: &P| r.x))
        .attr(Circle::Y, y.encode(|r: &P| r.y))
        .attr(Circle::RADIUS, Px(5.0))
        .max_chunk_rows(4)
        .retain(Retain::GpuOnly);
    plot.add(sel);
    plot.resolve(&cx, W, H).unwrap();
    assert!(plot.layer_mut::<P, Circle>().unwrap().rows_released());
    for (i, r) in rows.iter().enumerate() {
        let at = px(&x, &y, r.x, r.y);
        assert_eq!(plot.pick_row(at), None, "row {i}");
        assert_eq!(
            plot.pick_row_gpu(&cx, at).unwrap(),
            Some(RowId { layer: 0, row: i }),
            "row {i}"
        );
    }

    // A line: 1 px off each segment, nearer one end, picks that end.
    let (mut plot, x, y) = base();
    let mut sel = Selection::<P, Line>::new(rows.clone());
    sel.attr(Line::X, x.encode(|r: &P| r.x))
        .attr(Line::Y, y.encode(|r: &P| r.y))
        .attr(Line::WIDTH, Px(2.0))
        .max_chunk_rows(4)
        .retain(Retain::GpuOnly);
    plot.add(sel);
    plot.resolve(&cx, W, H).unwrap();
    for i in 0..rows.len() - 1 {
        let (a, b) = (
            px(&x, &y, rows[i].x, rows[i].y),
            px(&x, &y, rows[i + 1].x, rows[i + 1].y),
        );
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = dx.hypot(dy);
        // 2 px from the centre line (1 px off the 1 px half-width).
        let normal = (-dy / len * 2.0, dx / len * 2.0);
        for (t, want) in [(0.3, i), (0.7, i + 1)] {
            let at = Point::new(a.x + dx * t + normal.0, a.y + dy * t + normal.1);
            assert_eq!(plot.pick_row(at), None, "segment {i}, t {t}");
            assert_eq!(
                plot.pick_row_gpu(&cx, at).unwrap(),
                Some(RowId {
                    layer: 0,
                    row: want
                }),
                "segment {i}, t {t}"
            );
        }
    }
}

/// One mark's WGSL `hit_distance`, read back from the pick's depth, against
/// its CPU twin on 1,000 random points around it.
fn shape_case(name: &str, add: impl FnOnce(&mut Plot, &ScaleRef<Linear>, &ScaleRef<Linear>)) {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    add(&mut plot, &x, &y);
    plot.resolve(&cx, W, H).unwrap();
    let rect = plot.layout().unwrap().plot;
    let centre = px(&x, &y, 5.0, 5.0);
    let mut next = rng(0x1234_5678_9abc_def1);
    let mut worst = 0.0f32;
    for i in 0..1000 {
        let at = Point::new(
            centre.x + (next() as f32 - 0.5) * 60.0,
            centre.y + (next() as f32 - 0.5) * 60.0,
        );
        assert!(
            at.x > rect.x
                && at.x < rect.x + rect.width
                && at.y > rect.y
                && at.y < rect.y + rect.height,
            "{at:?}"
        );
        let hits = plot.pick_index().unwrap().hits_at(at, f32::INFINITY);
        let d = hits.iter().map(|h| h.0).fold(f32::INFINITY, f32::min);
        let gpu = plot.pick_row_gpu(&cx, at).unwrap();
        let picker = plot.gpu_pick.as_ref().unwrap();
        let depth = picker.depths.last().unwrap().1;
        let tol = 1e-4;
        if d > 3.0 + tol {
            assert_eq!(gpu, None, "{name} #{i} at {at:?}: d {d}");
        } else if d < 3.0 - tol {
            assert!(gpu.is_some(), "{name} #{i} at {at:?}: d {d}");
            if d > tol {
                let gd = (depth - 0.5) * 6.0;
                worst = worst.max((gd - d).abs());
                assert!(
                    (gd - d).abs() <= tol,
                    "{name} #{i} at {at:?}: cpu {d}, gpu {gd}"
                );
            } else if d < -tol {
                assert_eq!(depth, 0.0, "{name} #{i} at {at:?}: d {d}");
            }
        }
        if !hits.iter().any(|h| h.2.is_some_and(|g| g < tol)) && d < 3.0 - tol {
            assert_eq!(gpu, plot.pick_row(at), "{name} #{i} at {at:?}");
        }
    }
    println!("{name}: worst |gpu - cpu| distance {worst:e} px");
}

#[test]
fn hit_distance_matches_the_cpu_per_shape() {
    let one = |a: f64, b: f64, c: f64, d: f64| {
        vec![P {
            x: a,
            y: b,
            v: c,
            w: d,
        }]
    };
    shape_case("circle", |plot, x, y| {
        plot.add(Selection::<P, Circle>::new(one(5.0, 5.0, 0.0, 0.0)))
            .attr(Circle::X, x.encode(|r: &P| r.x))
            .attr(Circle::Y, y.encode(|r: &P| r.y))
            .attr(Circle::RADIUS, Px(9.5));
    });
    shape_case("rect", |plot, x, y| {
        use crate::marks::Rect as R;
        plot.add(Selection::<P, R>::new(one(4.6, 4.7, 5.5, 5.2)))
            .attr(R::X, x.encode(|r: &P| r.x))
            .attr(R::X_END, x.encode(|r: &P| r.v))
            .attr(R::Y, y.encode(|r: &P| r.y))
            .attr(R::Y_END, y.encode(|r: &P| r.w));
    });
    shape_case("segment", |plot, x, y| {
        plot.add(Selection::<P, Segment>::new(one(4.5, 4.6, 5.4, 5.3)))
            .attr(Segment::X, x.encode(|r: &P| r.x))
            .attr(Segment::Y, y.encode(|r: &P| r.y))
            .attr(Segment::X_END, x.encode(|r: &P| r.v))
            .attr(Segment::Y_END, y.encode(|r: &P| r.w))
            .attr(Segment::WIDTH, Px(5.0));
    });
    shape_case("line", |plot, x, y| {
        let rows = vec![
            P {
                x: 4.4,
                y: 4.5,
                v: 0.0,
                w: 0.0,
            },
            P {
                x: 5.6,
                y: 5.4,
                v: 0.0,
                w: 0.0,
            },
        ];
        plot.add(Selection::<P, Line>::new(rows))
            .attr(Line::X, x.encode(|r: &P| r.x))
            .attr(Line::Y, y.encode(|r: &P| r.y))
            .attr(Line::WIDTH, Px(4.0));
    });
}

#[test]
fn requests_and_polls() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    assert!(plot.request_pick(&cx, Point::new(10.0, 10.0)).is_err());
    plot.add(Selection::<P, Circle>::new(vec![P {
        x: 5.0,
        y: 5.0,
        v: 0.0,
        w: 0.0,
    }]))
    .attr(Circle::X, x.encode(|r: &P| r.x))
    .attr(Circle::Y, y.encode(|r: &P| r.y));
    plot.resolve(&cx, W, H).unwrap();
    let c = px(&x, &y, 5.0, 5.0);
    let tickets: Vec<_> = (0..4)
        .map(|i| {
            plot.request_pick(&cx, Point::new(c.x + i as f32 * 5.0, c.y))
                .unwrap()
        })
        .collect();
    let err = plot.request_pick(&cx, c).unwrap_err().to_string();
    assert!(err.contains("in flight"), "{err}");
    cx.wait_idle().unwrap();
    let done = plot.poll_picks(&cx);
    assert_eq!(done.iter().map(|p| p.ticket).collect::<Vec<_>>(), tickets);
    let rows: Vec<_> = done.iter().map(|p| p.row.is_some()).collect();
    assert_eq!(rows, [true, true, false, false], "{done:?}");
    assert!(
        done.iter()
            .all(|p| p.generation == plot.resolve_generation())
    );
    assert!(plot.poll_picks(&cx).is_empty());
}

/// The median of 100 GPU picks on 100,000 circles at 800 × 500.
#[test]
fn pick_100k_circles() {
    use std::time::Instant;
    let cx = Context::new_blocking().unwrap();
    let mut next = rng(0x2545_f491_4f6c_dd1d);
    let rows: Vec<P> = (0..100_000)
        .map(|_| P {
            x: next() * 10.0,
            y: next() * 10.0,
            v: 0.0,
            w: 0.0,
        })
        .collect();
    let (mut plot, x, y) = base();
    plot.add(Selection::<P, Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &P| r.x))
        .attr(Circle::Y, y.encode(|r: &P| r.y))
        .attr(Circle::RADIUS, Px(6.0));
    plot.resolve(&cx, 800.0, 500.0).unwrap();
    let rect = plot.layout().unwrap().plot;
    // The first pick links the pick pipeline.
    plot.pick_row_gpu(&cx, Point::new(rect.x + 50.0, rect.y + 50.0))
        .unwrap();
    let mut times: Vec<_> = (0..100)
        .map(|_| {
            let at = Point::new(
                rect.x + next() as f32 * rect.width,
                rect.y + next() as f32 * rect.height,
            );
            let t = Instant::now();
            plot.pick_row_gpu(&cx, at).unwrap();
            t.elapsed()
        })
        .collect();
    times.sort();
    let median = times[50];
    println!("median of 100 GPU picks on 100,000 circles at 800x500: {median:?}");
    let budget = if cfg!(debug_assertions) { 50 } else { 5 };
    assert!(median.as_millis() < budget, "{median:?}");
}
