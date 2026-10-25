// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S12a (GUP-480): public `append`, `Appender` and
//! `Chart::set_waker`, and what the landed systems do after appends:
//! chunking, draw order, culling, brush bits, picks and SVG.

use gup_core::geom::Point;
use gup_core::prelude::*;
use gup_core::{Appender, ChartExt, Closed, Input, SvgTarget, VectorTarget};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Wake, Waker};

const W: u32 = 400;
const H: u32 = 300;

#[derive(Clone, Debug, PartialEq)]
struct R {
    x: f64,
    y: f64,
    s: f64,
}

fn rows(range: std::ops::Range<u32>) -> Vec<R> {
    range
        .map(|i| {
            let i = f64::from(i);
            R {
                x: (i * 37.0) % 200.0 / 10.0,
                y: (i * 0.37) % 9.0,
                s: 1.0 + (i * 7.0) % 13.0,
            }
        })
        .collect()
}

fn scatter_of(rows: Vec<R>) -> gup_core::ScatterChart<R> {
    gup_core::scatter(rows)
        .x(|r| r.x)
        .y(|r| r.y)
        .fill(Color::hex(0xd62728))
        .radius(3.0)
}

fn fixed(chart: gup_core::ScatterChart<R>) -> gup_core::ScatterChart<R> {
    chart
        .x_scale(Linear::new().domain(0.0, 10.0))
        .y_scale(Linear::new().domain(0.0, 10.0))
}

/// Push `batches` of rows from a thread and wait for it.
fn push_from_thread(feed: &Appender<R>, all: Vec<R>, batches: usize) {
    let feed = feed.clone();
    std::thread::spawn(move || {
        for batch in all.chunks(all.len().div_ceil(batches)) {
            feed.extend(batch.to_vec()).unwrap();
        }
    })
    .join()
    .unwrap();
}

#[test]
fn appender_is_clone_send_sync() {
    fn check<T: Clone + Send + Sync>() {}
    check::<Appender<R>>();
    check::<Appender<String>>();
    assert_eq!(Closed.to_string(), "the chart was dropped");
}

/// AC2: a thread pushes 1,000 rows in 10 `extend` calls into a layer of
/// 64-row chunks; one resolve draws them all, writing exactly their bytes,
/// and the image is that of a chart built from every row at once.
#[test]
fn a_thread_feeds_a_chunked_chart() {
    let cx = Context::new_blocking().unwrap();
    let chunked = |n| {
        let mut chart = scatter_of(rows(0..n));
        chart.layer_mut().max_chunk_rows(64);
        chart
    };
    let mut chart = chunked(100);
    let feed = chart.appender();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    assert!(!chart.needs_frame());
    let start = cx.upload_stats();

    push_from_thread(&feed, rows(100..1100), 10);
    assert!(chart.needs_frame());
    let image = chart.render(&cx, W, H).unwrap();
    let written = cx.upload_stats() - start;
    // Two f32 columns.
    assert_eq!(written.columns.bytes, 1000 * 8, "{written:?}");
    assert!(!chart.needs_frame());

    let expected = chunked(1100).render(&cx, W, H).unwrap();
    assert!(image == expected, "appending differs from building at once");
}

/// The size draw order (GUP-431) merges the appended batch.
#[test]
fn appends_keep_the_size_draw_order() {
    let cx = Context::new_blocking().unwrap();
    let sized = |n| {
        gup_core::scatter(rows(0..n))
            .x(|r| r.x)
            .y(|r| r.y)
            .size(|r| r.s)
            .fill(Color::hex(0x1f77b4))
    };
    let mut chart = sized(60);
    let feed = chart.appender();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    push_from_thread(&feed, rows(60..100), 1);
    let image = chart.render(&cx, W, H).unwrap();
    let expected = sized(100).render(&cx, W, H).unwrap();
    assert!(image == expected, "draw order differs after an append");
}

/// Culling (GUP-457) sees the appended chunks: with a fixed domain most of
/// the rows are outside, and the image is the fresh one.
#[test]
fn appends_are_culled_like_built_rows() {
    let cx = Context::new_blocking().unwrap();
    let culled = |n| {
        let mut chart = fixed(scatter_of(rows(0..n)));
        chart.layer_mut().max_chunk_rows(16);
        chart
    };
    let mut chart = culled(64);
    let feed = chart.appender();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    push_from_thread(&feed, rows(64..200), 4);
    let image = chart.render(&cx, W, H).unwrap();
    let expected = culled(200).render(&cx, W, H).unwrap();
    assert!(image == expected, "culling differs after appends");
}

fn pixel_of(chart: &gup_core::ScatterChart<R>, x: f64, y: f64) -> Point {
    let l = chart.layout().expect("resolved");
    let ticks = |t: &[(f64, f32)], v: f64| {
        let (a, b) = (t[0], *t.last().unwrap());
        (f64::from(a.1) + (v - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)) as f32
    };
    Point::new(ticks(&l.x_ticks, x), ticks(&l.y_ticks, y))
}

fn drag(chart: &mut gup_core::ScatterChart<R>, from: Point, to: Point) {
    let button = gup_core::Button::Primary;
    let _ = chart.handle(Input::PointerDown { at: from, button });
    let _ = chart.handle(Input::PointerMoved { at: to });
    let _ = chart.handle(Input::PointerUp { at: to, button });
}

fn colour_at(image: &image::RgbaImage, at: Point) -> [u8; 3] {
    let p = image.get_pixel(at.x.round() as u32, at.y.round() as u32).0;
    [p[0], p[1], p[2]]
}

/// Brush bits (GUP-458) grow with the rows: the brushed rows stay lit and
/// an appended row, which the brush never held, is dimmed.
#[test]
fn appended_rows_are_unbrushed_and_old_brushed_rows_stay_lit() {
    let cx = Context::new_blocking().unwrap();
    let at = |x, y| R { x, y, s: 1.0 };
    let mut chart = fixed(
        gup_core::scatter(vec![at(2.0, 2.0), at(2.5, 2.0), at(8.0, 8.0)])
            .x(|r| r.x)
            .y(|r| r.y)
            .fill(Color::hex(0xd62728))
            .radius(6.0),
    );
    chart.plot_mut().drag(Drag::Brush);
    let feed = chart.appender();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    let (a, b) = (pixel_of(&chart, 1.0, 4.0), pixel_of(&chart, 4.0, 1.0));
    drag(&mut chart, a, b);
    assert_eq!(chart.plot_mut().brush().map(|b| b.rows.len()), Some(2));
    chart.render(&cx, W, H).unwrap();

    feed.extend([at(3.0, 3.0), at(9.0, 9.0)]).unwrap();
    let image = chart.render(&cx, W, H).unwrap();
    // (2,2) and (3,3) are both under the brush rectangle's tint; only the
    // brushed row is lit. (8,8) and (9,9) are unbrushed and untinted.
    let lit = colour_at(&image, pixel_of(&chart, 2.0, 2.0));
    let appended = colour_at(&image, pixel_of(&chart, 3.0, 3.0));
    let dim = colour_at(&image, pixel_of(&chart, 8.0, 8.0));
    assert_eq!(colour_at(&image, pixel_of(&chart, 9.0, 9.0)), dim);
    assert!(appended[1] > lit[1] + 40, "{appended:?} vs lit {lit:?}");
    assert!(dim[1] > lit[1] + 40, "{dim:?} vs lit {lit:?}");
    // The brush still holds its two rows.
    assert_eq!(chart.plot_mut().brush().map(|b| b.rows.len()), Some(2));
}

/// Picks (GUP-445) find appended rows, with their data row index.
#[test]
fn picks_find_appended_rows() {
    let cx = Context::new_blocking().unwrap();
    let at = |x, y| R { x, y, s: 1.0 };
    let mut chart = fixed(
        gup_core::scatter(vec![at(2.0, 2.0)])
            .x(|r| r.x)
            .y(|r| r.y)
            .radius(6.0),
    );
    let feed = chart.appender();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    feed.extend([at(5.0, 5.0), at(7.0, 3.0)]).unwrap();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    let p = pixel_of(&chart, 7.0, 3.0);
    let id = chart.pick_row(p).expect("a pick");
    assert_eq!((id.layer, id.row), (0, 2));
    assert_eq!(chart.pick(p), Some(&at(7.0, 3.0)));
    assert_eq!(chart.plot_mut().pick_item_count(), Some(3));
}

/// SVG (GUP-467) draws every row, appended ones included.
#[test]
fn svg_draws_appended_rows() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = fixed(scatter_of(
        rows(0..10)
            .into_iter()
            .map(|r| R { x: r.x / 2.0, ..r })
            .collect(),
    ));
    let feed = chart.appender();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    feed.extend(rows(10..25).into_iter().map(|r| R { x: r.x / 2.0, ..r }))
        .unwrap();
    let scene = chart
        .resolve(&cx, W as f32, H as f32)
        .unwrap()
        .scene
        .clone();
    let mut svg = SvgTarget::new();
    svg.render(&scene).unwrap();
    assert_eq!(svg.into_string().matches("<circle ").count(), 25);
}

/// Appending through `layer_mut` bumps the revision: the resolve is not the
/// cached one and draws the new rows.
#[test]
fn a_public_append_misses_the_cache() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = scatter_of(rows(0..20));
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    let before = chart.render(&cx, W, H).unwrap();
    chart.layer_mut().append(rows(20..40)).unwrap();
    let after = chart.render(&cx, W, H).unwrap();
    assert_ne!(before, after);
    assert!(after == scatter_of(rows(0..40)).render(&cx, W, H).unwrap());
}

/// GUP-444: an appender with nothing pushed leaves a clean resolve writing
/// nothing.
#[test]
fn an_idle_appender_keeps_resolve_clean() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = scatter_of(rows(0..20));
    let _feed = chart.appender();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    let start = cx.upload_stats();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    assert_eq!(cx.upload_stats() - start, Default::default());
    assert!(!chart.needs_frame());
}

#[test]
fn push_after_the_chart_drops_is_closed() {
    let mut chart = scatter_of(rows(0..3));
    let feed = chart.appender();
    assert_eq!(feed.push(rows(3..4).remove(0)), Ok(()));
    drop(chart);
    assert_eq!(feed.push(rows(4..5).remove(0)), Err(Closed));
    assert_eq!(feed.extend(rows(5..7)), Err(Closed));
}

struct Count(AtomicUsize);

impl Wake for Count {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// AC3: each `push` and `extend` wakes the host's waker once, whether the
/// appender was made before or after `set_waker`.
#[test]
fn pushes_wake_the_host() {
    let count = Arc::new(Count(AtomicUsize::new(0)));
    let woken = || count.0.load(Ordering::SeqCst);
    let mut chart = scatter_of(rows(0..3));
    let early = chart.appender();
    chart.set_waker(Waker::from(count.clone()));
    let late = chart.appender();
    early.push(rows(3..4).remove(0)).unwrap();
    assert_eq!(woken(), 1);
    late.push(rows(4..5).remove(0)).unwrap();
    assert_eq!(woken(), 2);
    late.extend(rows(5..15)).unwrap();
    assert_eq!(woken(), 3);

    // A keyed line chart's builder feed and a plain line's layer feed too.
    for keyed in [true, false] {
        let count = Arc::new(Count(AtomicUsize::new(0)));
        let line = gup_core::line(vec![(0.0, 1.0, "a")]).x(|p| p.0).y(|p| p.1);
        let mut line = if keyed { line.color(|p| p.2) } else { line };
        let feed = line.appender();
        line.set_waker(Waker::from(count.clone()));
        feed.push((1.0, 2.0, "a")).unwrap();
        assert_eq!(count.0.load(Ordering::SeqCst), 1, "keyed: {keyed}");
        assert!(line.needs_frame());
    }
}

/// A keyed line chart: rows of a known key extend its line, a new key adds
/// a line in the next palette colour with a legend entry, and the picture
/// is that of a chart built from every row.
#[test]
fn a_keyed_line_chart_routes_rows_to_lines() {
    let cx = Context::new_blocking().unwrap();
    type L = (f64, f64, &'static str);
    let make = |rows: Vec<L>| {
        gup_core::line(rows)
            .x(|p: &L| p.0)
            .y(|p: &L| p.1)
            .color(|p: &L| p.2)
            .width(3.0)
    };
    let point = |i: u32, key| (f64::from(i), f64::from(i % 5), key);
    let first: Vec<L> = (0..6)
        .flat_map(|i| [point(i, "A"), point(i, "B")])
        .collect();
    let more: Vec<L> = (6..12)
        .flat_map(|i| [point(i, "A"), point(i, "B"), point(i, "C")])
        .collect();
    let mut chart = make(first.clone());
    let feed = chart.appender();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    feed.extend(more.clone()).unwrap();
    let (image, layout) = chart.render_resolved(&cx, W, H).unwrap();

    let all: Vec<L> = first.iter().chain(&more).copied().collect();
    let mut fresh = make(all);
    // The fresh chart orders rows by first seen key: A, B, C as well. Its
    // rows are interleaved by key, so each line has the same points.
    let (expected, _) = fresh.render_resolved(&cx, W, H).unwrap();
    assert!(image == expected, "keyed appends differ from a fresh chart");
    let legend: Vec<_> = layout
        .texts
        .iter()
        .filter(|t| &*t.run.text == "C")
        .collect();
    assert_eq!(legend.len(), 1, "a legend entry for the new key");
}

/// A keyed line chart over `u32` ids: a row of a known id extends its line,
/// a new id adds one (GUP-537).
#[test]
fn an_integer_keyed_line_chart_adds_a_line_per_new_id() {
    let cx = Context::new_blocking().unwrap();
    type P = (f64, f64, u32);
    let mut chart = gup_core::line(vec![(0.0, 0.0, 7), (1.0, 1.0, 9), (2.0, 2.0, 7)])
        .x(|p: &P| p.0)
        .y(|p: &P| p.1)
        .color(|p| p.2);
    let feed = chart.appender();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    assert_eq!(chart.layers_mut().len(), 2);

    feed.extend(vec![(3.0, 3.0, 7)]).unwrap();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    assert_eq!(chart.layers_mut().len(), 2);

    feed.extend(vec![(4.0, 4.0, 11)]).unwrap();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    assert_eq!(chart.layers_mut().len(), 3);
}
