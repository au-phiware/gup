// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S8b (GUP-445): CPU picking. Each mark kind's hit shape, rows
//! across chunks, nulls and retention policies, layer order, zoom, and the
//! cost of a pick on 100,000 points.

mod common;

use common::vr::rgba8;
use gup_core::geom::Point as Pt;
use gup_core::prelude::*;
use gup_core::{ImageTarget, Input, RowId};

const W: u32 = 480;
const H: u32 = 320;

#[derive(Clone, Debug)]
struct P {
    x: f64,
    y: f64,
    v: f64,
}

fn p(x: f64, y: f64, v: f64) -> P {
    P { x, y, v }
}

/// A plot with x and y over `0..10` and the handles to read pixels back.
fn base() -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>) {
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, 10.0));
    let y = plot.y(Linear::new().domain(0.0, 10.0));
    (plot, x, y)
}

fn at(x: &ScaleRef<Linear>, y: &ScaleRef<Linear>, p: &P) -> Pt {
    Pt::new(x.read().eval(p.x) as f32, y.read().eval(p.y) as f32)
}

fn off(p: Pt, dx: f32, dy: f32) -> Pt {
    Pt::new(p.x + dx, p.y + dy)
}

fn row(layer: usize, row: usize) -> Option<RowId> {
    Some(RowId { layer, row })
}

fn circles(rows: Vec<P>, x: &ScaleRef<Linear>, y: &ScaleRef<Linear>) -> Selection<P, Circle> {
    let mut sel = Selection::<P, Circle>::new(rows);
    sel.attr(Circle::X, x.encode(|r: &P| r.x))
        .attr(Circle::Y, y.encode(|r: &P| r.y))
        .attr(Circle::RADIUS, Px(6.0));
    sel
}

fn resolve(cx: &Context, plot: &mut Plot) {
    plot.resolve(cx, W as f32, H as f32).unwrap();
}

#[test]
fn circle_hits_follow_the_drawn_discs() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    let rows = vec![p(2.0, 3.0, 0.0), p(5.0, 5.0, 1.0), p(8.0, 7.0, 2.0)];
    let heat = Sequential::viridis().domain(0.0, 2.0);
    let mut sel = circles(rows.clone(), &x, &y);
    sel.attr(Circle::FILL, heat.encode(|r: &P| r.v));
    assert_eq!(
        plot.pick_row(Pt::new(100.0, 100.0)),
        None,
        "before a resolve"
    );
    plot.add(sel);
    let resolved = plot.resolve(&cx, W as f32, H as f32).unwrap();
    let image = ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let r = 6.0;
    for (i, d) in rows.iter().enumerate() {
        let c = at(&x, &y, d);
        assert_eq!(plot.pick_row(c), row(0, i), "centre of row {i}");
        assert_eq!(plot.pick_row(off(c, r - 0.5, 0.0)), row(0, i));
        assert_eq!(plot.pick_row(off(c, 0.0, -(r + 2.5))), row(0, i));
        assert_eq!(plot.pick_row(off(c, r + 3.5, 0.0)), None);
        // The drawn pixel at the anchor has the row's fill.
        let a = plot.pick_anchor(RowId { layer: 0, row: i }).unwrap();
        assert!(
            (a.x - c.x).abs() < 0.01 && (a.y - c.y).abs() < 0.01,
            "{a:?} {c:?}"
        );
        let got = image.get_pixel(a.x as u32, a.y as u32).0;
        let want = rgba8(heat.eval(d.v));
        let de = want.delta_e(gup_visual_regression::Rgba8::rgb(got[0], got[1], got[2]));
        assert!(de < 2.0, "row {i}: {got:?} vs {want:?}, dE {de}");
    }
    let outside = Pt::new(2.0, 2.0);
    assert_eq!(plot.pick_row(outside), None, "outside the plot rect");
    assert_eq!(plot.pick::<P>(at(&x, &y, &rows[1])).unwrap().v, 1.0);
    assert!(plot.pick::<String>(at(&x, &y, &rows[1])).is_none());
}

#[test]
fn the_row_drawn_last_wins_an_overlap() {
    let cx = Context::new_blocking().unwrap();
    // Constant radius: the later row.
    let (mut plot, x, y) = base();
    plot.add(circles(vec![p(4.0, 5.0, 0.0), p(4.2, 5.0, 0.0)], &x, &y));
    resolve(&cx, &mut plot);
    let (a, b) = (at(&x, &y, &p(4.0, 5.0, 0.0)), at(&x, &y, &p(4.2, 5.0, 0.0)));
    let mid = Pt::new((a.x + b.x) / 2.0, a.y);
    assert_eq!(plot.pick_row(mid), row(0, 1));
    assert_eq!(plot.pick_row(off(a, -3.0, 0.0)), row(0, 0));

    // Radius from a column: the smaller disc draws last.
    let (mut plot, x, y) = base();
    let size = ScaleRef::new(Pow::sqrt().domain(0.0, 10.0).range(Px(4.0), Px(12.0)));
    let rows = vec![p(4.0, 5.0, 10.0), p(4.3, 5.0, 0.0)];
    let mut sel = circles(rows.clone(), &x, &y);
    sel.attr(Circle::RADIUS, size.encode(|r: &P| r.v));
    plot.add(sel);
    resolve(&cx, &mut plot);
    let (big, small) = (at(&x, &y, &rows[0]), at(&x, &y, &rows[1]));
    assert_eq!(plot.pick_row(small), row(0, 1), "the small disc is on top");
    // Inside the big disc, 2 px outside the small one's edge: the big one.
    let rs = size.read().eval(0.0) as f32;
    let q = Pt::new(small.x - (rs + 2.0), small.y);
    assert!(((q.x - big.x).powi(2) + (q.y - big.y).powi(2)).sqrt() < size.read().eval(10.0) as f32);
    assert_eq!(plot.pick_row(q), row(0, 0));
}

#[test]
fn rects_pick_inside_and_not_4px_outside() {
    let cx = Context::new_blocking().unwrap();
    // Centre-and-extent in x (a band), edges in y (a derived baseline).
    let mut plot = Plot::new();
    let bx = plot.x(Band::new());
    let y = plot.y(Linear::new().domain(0.0, 60.0));
    let baseline = y.clone();
    struct Sale {
        month: &'static str,
        total: f64,
    }
    let months = ["Jan", "Feb", "Mar"];
    let sales: Vec<Sale> = months
        .iter()
        .zip([32.0, 45.0, 28.0])
        .map(|(&month, total)| Sale { month, total })
        .collect();
    plot.add(Selection::<Sale, Rect>::new(sales))
        .attr(Rect::X, bx.encode_key(|s: &Sale| s.month).domain(months))
        .attr(Rect::WIDTH, bx.band_width_px())
        .attr(Rect::Y, y.encode(|s: &Sale| s.total))
        .attr(
            Rect::Y_END,
            Derived::new(move || Px(baseline.read().eval(0.0) as f32)),
        );
    resolve(&cx, &mut plot);
    let base_y = y.read().eval(0.0) as f32;
    let bw = bx.read().band_width().abs() as f32;
    for (i, (m, total)) in months.iter().zip([32.0, 45.0, 28.0]).enumerate() {
        let cxp = bx.read().eval(i as f64) as f32;
        let top = y.read().eval(total) as f32;
        let mid = Pt::new(cxp, (top + base_y) / 2.0);
        assert_eq!(plot.pick_row(mid), row(0, i), "{m}");
        assert_eq!(plot.pick_row(Pt::new(cxp, top - 4.0)), None, "{m} above");
        assert_eq!(
            plot.pick_row(Pt::new(cxp + bw / 2.0 - 0.5, mid.y)),
            row(0, i)
        );
        assert_eq!(
            plot.pick_row(Pt::new(cxp, base_y + 4.0)).map(|r| r.layer),
            None,
            "{m} below"
        );
        let a = plot.pick_anchor(RowId { layer: 0, row: i }).unwrap();
        assert!(
            (a.x - cxp).abs() < 0.01 && (a.y - top).abs() < 0.01,
            "{a:?}"
        );
    }
}

#[test]
fn rect_edges_in_either_order_pick_inside_only() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    plot.add(Selection::<P, Rect>::new(vec![p(7.0, 8.0, 5.0)]))
        .attr(Rect::X, x.encode(|r: &P| r.x))
        .attr(Rect::X_END, x.encode(|r: &P| r.v))
        .attr(Rect::Y, y.encode(|r: &P| r.y))
        .attr(Rect::Y_END, y.encode(|r: &P| r.y - 2.0));
    resolve(&cx, &mut plot);
    let (x0, x1) = (x.read().eval(5.0) as f32, x.read().eval(7.0) as f32);
    let (y0, y1) = (y.read().eval(8.0) as f32, y.read().eval(6.0) as f32);
    let (xm, ym) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (left, right) = (x0.min(x1), x0.max(x1));
    let (top, bottom) = (y0.min(y1), y0.max(y1));
    assert_eq!(plot.pick_row(Pt::new(xm, ym)), row(0, 0));
    assert_eq!(plot.pick_row(Pt::new(left + 0.5, ym)), row(0, 0));
    assert_eq!(plot.pick_row(Pt::new(left - 4.0, ym)), None);
    assert_eq!(plot.pick_row(Pt::new(right + 4.0, ym)), None);
    assert_eq!(plot.pick_row(Pt::new(xm, top - 4.0)), None);
    assert_eq!(plot.pick_row(Pt::new(xm, bottom + 4.0)), None);
    let a = plot.pick_anchor(RowId { layer: 0, row: 0 }).unwrap();
    assert!((a.x - xm).abs() < 0.01 && (a.y - top).abs() < 0.01, "{a:?}");
}

struct Seg {
    x0: f64,
    x1: f64,
    y: f64,
}

#[test]
fn segments_have_square_caps_and_a_tolerance() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    plot.add(Selection::<Seg, Segment>::new(vec![Seg {
        x0: 2.0,
        x1: 6.0,
        y: 5.0,
    }]))
    .attr(Segment::X, x.encode(|s: &Seg| s.x0))
    .attr(Segment::X_END, x.encode(|s: &Seg| s.x1))
    .attr(Segment::Y, y.encode(|s: &Seg| s.y))
    .attr(Segment::Y_END, y.encode(|s: &Seg| s.y))
    .attr(Segment::WIDTH, Px(4.0));
    resolve(&cx, &mut plot);
    let a = Pt::new(x.read().eval(2.0) as f32, y.read().eval(5.0) as f32);
    let b = Pt::new(x.read().eval(6.0) as f32, a.y);
    let mid = Pt::new((a.x + b.x) / 2.0, a.y);
    assert_eq!(plot.pick_row(mid), row(0, 0));
    assert_eq!(plot.pick_row(off(b, 1.5, 0.0)), row(0, 0), "square cap");
    assert_eq!(plot.pick_row(off(b, 4.5, 0.0)), row(0, 0), "tolerance");
    assert_eq!(plot.pick_row(off(b, 6.0, 0.0)), None);
    assert_eq!(plot.pick_row(off(mid, 0.0, 5.5)), None);
    assert_eq!(plot.pick_row(off(mid, 0.0, 4.5)), row(0, 0));
    assert_eq!(plot.pick_anchor(RowId { layer: 0, row: 0 }), Some(mid));
}

#[test]
fn lines_pick_the_nearer_end_and_break_at_nulls() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    let rows = vec![
        p(1.0, 2.0, 0.0),
        p(3.0, 6.0, 0.0),
        p(5.0, f64::NAN, 0.0),
        p(7.0, 4.0, 0.0),
        p(9.0, 8.0, 0.0),
    ];
    plot.add(Selection::<P, Line>::new(rows.clone()))
        .attr(Line::X, x.encode(|r: &P| r.x))
        .attr(Line::Y, y.encode(|r: &P| r.y))
        .attr(Line::WIDTH, Px(2.0));
    resolve(&cx, &mut plot);
    let pt = |i: usize| at(&x, &y, &rows[i]);
    let (p0, p1, p3, p4) = (pt(0), pt(1), pt(3), pt(4));
    let lerp = |a: Pt, b: Pt, t: f32| Pt::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
    assert_eq!(plot.pick_row(lerp(p0, p1, 0.7)), row(0, 1));
    assert_eq!(plot.pick_row(lerp(p0, p1, 0.3)), row(0, 0));
    assert_eq!(plot.pick_row(p1), row(0, 1));
    // Where segments 1-2 and 2-3 would run, if row 2 had y = 5.
    let p2 = Pt::new(x.read().eval(5.0) as f32, y.read().eval(5.0) as f32);
    assert_eq!(plot.pick_row(lerp(p1, p2, 0.5)), None);
    assert_eq!(plot.pick_row(lerp(p2, p3, 0.5)), None);
    assert_eq!(plot.pick_row(off(p4, 1.0, 1.0)), row(0, 4));
    assert_eq!(plot.pick_row(lerp(p3, p4, 0.4)), row(0, 3));
}

#[test]
fn chunks_and_nulls_do_not_change_picks() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<P> = (0..10)
        .map(|i| p(0.5 + f64::from(i), 5.0 + f64::from(i % 3), 0.0))
        .collect();
    for chunk in [None, Some(4)] {
        let (mut plot, x, y) = base();
        let mut sel = circles(rows.clone(), &x, &y);
        if let Some(c) = chunk {
            sel.max_chunk_rows(c);
        }
        plot.add(sel);
        resolve(&cx, &mut plot);
        for (i, d) in rows.iter().enumerate() {
            assert_eq!(plot.pick_row(at(&x, &y, d)), row(0, i), "{chunk:?} row {i}");
        }
    }
    // A NaN y is never picked.
    let (mut plot, x, y) = base();
    let mut with_null = rows.clone();
    with_null[4].y = f64::NAN;
    let mut sel = circles(with_null, &x, &y);
    sel.max_chunk_rows(4);
    plot.add(sel);
    resolve(&cx, &mut plot);
    let l = plot.layout().unwrap().plot;
    let col = at(&x, &y, &rows[4]).x;
    let mut yy = l.y;
    while yy < l.y + l.height {
        assert_ne!(plot.pick_row(Pt::new(col, yy)), row(0, 4), "y {yy}");
        yy += 2.0;
    }
}

#[test]
fn retention_decides_what_a_pick_can_return() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<P> = (0..10)
        .map(|i| p(0.5 + f64::from(i), 5.0, f64::from(i)))
        .collect();
    for (policy, pick_row, pick_data) in [
        (Retain::Rows, true, true),
        (Retain::Columns, true, false),
        (Retain::GpuOnly, false, false),
    ] {
        let (mut plot, x, y) = base();
        let mut sel = circles(rows.clone(), &x, &y);
        sel.max_chunk_rows(4).retain(policy);
        plot.add(sel);
        // A second layer at another height, with the default policy.
        let mut other = circles(rows.clone(), &x, &y);
        other.attr(Circle::Y, y.encode(|r: &P| r.y + 3.0));
        plot.add(other);
        resolve(&cx, &mut plot);
        for (i, d) in rows.iter().enumerate() {
            let c = at(&x, &y, d);
            let want = if pick_row { row(0, i) } else { None };
            assert_eq!(plot.pick_row(c), want, "{policy:?} row {i}");
            assert_eq!(
                plot.pick::<P>(c).map(|r| r.v),
                pick_data.then_some(d.v),
                "{policy:?} row {i}"
            );
            let upper = at(&x, &y, &p(d.x, d.y + 3.0, 0.0));
            assert_eq!(plot.pick_row(upper), row(1, i), "second layer, {policy:?}");
        }
    }
}

#[test]
fn the_higher_z_wins_then_the_later_layer() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    let mut first = circles(vec![p(5.0, 5.0, 0.0)], &x, &y);
    first.z(5);
    plot.add(first);
    plot.add(circles(vec![p(5.0, 5.0, 0.0)], &x, &y));
    plot.add(circles(vec![p(5.0, 5.0, 0.0)], &x, &y));
    resolve(&cx, &mut plot);
    let c = at(&x, &y, &p(5.0, 5.0, 0.0));
    assert_eq!(plot.pick_row(c), row(0, 0), "z 5 over later layers");
    let (mut plot, x, y) = base();
    plot.add(circles(vec![p(5.0, 5.0, 0.0)], &x, &y));
    plot.add(circles(vec![p(5.0, 5.0, 0.0)], &x, &y));
    resolve(&cx, &mut plot);
    assert_eq!(plot.pick_row(c), row(1, 0), "equal z: the later layer");
}

#[test]
fn a_zoom_moves_the_picks_with_the_marks() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    let rows = vec![p(4.0, 5.0, 0.0), p(5.0, 5.0, 0.0), p(6.0, 5.0, 0.0)];
    plot.add(circles(rows.clone(), &x, &y));
    resolve(&cx, &mut plot);
    let centre = at(&x, &y, &rows[1]);
    let before = at(&x, &y, &rows[2]);
    assert_eq!(plot.pick_row(before), row(0, 2));
    assert!(
        plot.handle(Input::Wheel {
            at: centre,
            lines: 2.0
        })
        .redraw
    );
    resolve(&cx, &mut plot);
    let after = at(&x, &y, &rows[2]);
    assert!(after.x > before.x + 10.0, "{before:?} -> {after:?}");
    assert_eq!(plot.pick_row(after), row(0, 2));
    assert_eq!(plot.pick_row(centre), row(0, 1));
}

/// The library circle without a declared hit shape: a user mark.
mod user {
    use gup_core::prelude::{Color, Mark, Px, Unit};

    #[derive(Debug, Mark)]
    #[mark(module = "gup::marks::circle", geometry = Quad)]
    pub struct Circle {
        #[channel(role = X)]
        x: Px,
        #[channel(role = Y)]
        y: Px,
        #[channel(role = Size, default = Px(3.5))]
        radius: Px,
        #[channel(role = Color, default = Color::hex(0x0072b2))]
        fill: Color,
        #[channel(default = Unit(1.0))]
        fill_opacity: Unit,
        #[channel(role = Color, default = Color::TRANSPARENT)]
        stroke: Color,
        #[channel(default = Px(0.0))]
        stroke_width: Px,
    }
}

#[test]
fn a_mark_without_a_hit_shape_is_never_picked() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = base();
    plot.add(Selection::<P, user::Circle>::new(vec![p(5.0, 5.0, 0.0)]))
        .attr(user::Circle::X, x.encode(|r: &P| r.x))
        .attr(user::Circle::Y, y.encode(|r: &P| r.y))
        .attr(user::Circle::RADIUS, Px(6.0));
    resolve(&cx, &mut plot);
    assert_eq!(plot.pick_row(at(&x, &y, &p(5.0, 5.0, 0.0))), None);
}

/// AC4: 100,000 circles. Run with
/// `cargo test -p gup-core --release --test pick -- --ignored --nocapture`.
#[test]
#[ignore = "timing: run in release"]
fn pick_100k_points() {
    use std::time::Instant;
    let cx = Context::new_blocking().unwrap();
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let rows: Vec<P> = (0..100_000)
        .map(|_| p(next() * 10.0, next() * 10.0, 0.0))
        .collect();
    let (mut plot, x, y) = base();
    plot.add(circles(rows, &x, &y));
    plot.resolve(&cx, 800.0, 500.0).unwrap();
    let rect = plot.layout().unwrap().plot;
    let t = Instant::now();
    let _ = plot.pick_row(Pt::new(rect.x + 50.0, rect.y + 50.0));
    let build = t.elapsed();
    let t = Instant::now();
    let mut hits = 0;
    for _ in 0..10_000 {
        let q = Pt::new(
            rect.x + next() as f32 * rect.width,
            rect.y + next() as f32 * rect.height,
        );
        hits += usize::from(plot.pick_row(q).is_some());
    }
    let mean = t.elapsed() / 10_000;
    println!("first pick (index build): {build:?}; mean of 10,000 picks: {mean:?}; {hits} hits");
    assert!(build.as_millis() <= 50, "{build:?}");
    assert!(mean.as_micros() <= 20, "{mean:?}");
}
