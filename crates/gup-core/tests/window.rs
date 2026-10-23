// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S12c (GUP-482): `Window::LastRows` eviction keeps draw order,
//! line bridges, picks, brush bits, culling and `Retain` consistent. Each
//! windowed run is compared with a plot built directly from the rows the
//! layer still holds.

use gup_core::geom::Point;
use gup_core::prelude::*;
use gup_core::{Appender, ChartExt, Input, RowId};
use std::sync::{Arc, Mutex};

const W: u32 = 400;
const H: u32 = 300;
/// The window, the chunk size and the batches of AC2.
const N: usize = 200;
const CHUNK: u32 = 64;
const BATCHES: [usize; 6] = [1, 27, 36, 100, 300, 5];
const TOTAL: usize = 2_000;

#[derive(Clone, Debug, PartialEq)]
struct R {
    x: f64,
    y: f64,
    s: f64,
}

/// AC2's rows: x is the row index, so the auto x domain shows which rows
/// are held. The size grows with the row, so each batch is stored newest
/// first and a chunk boundary splits a batch's sorted rows (the case the
/// window must not evict early). Every value is exact in `f32`, whatever
/// the chunk's origin.
fn row(i: usize) -> R {
    R {
        x: i as f64,
        y: ((i * 7) % 50) as f64,
        s: 1.0 + (i / 100) as f64,
    }
}

/// AC3's rows: a 20 × 20 grid, so the at most 328 held rows never share a
/// cell and each row's centre picks it.
fn cell(i: usize) -> R {
    R {
        x: (i % 20) as f64,
        y: ((i / 20) % 20) as f64,
        s: 1.0 + (i / 100) as f64,
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Kind {
    Plain,
    Sized,
    Line,
    /// Sized, on the grid, with fixed domains (AC3).
    Grid,
}

fn data(kind: Kind) -> fn(usize) -> R {
    if kind == Kind::Grid { cell } else { row }
}

/// A plot of `rows` drawn as `kind`, in 64-row chunks.
fn plot_of(kind: Kind, rows: Vec<R>, window: Window, retain: Retain) -> Plot {
    let scatter = || gup_core::scatter(rows.clone()).x(|r| r.x).y(|r| r.y);
    let mut plot = match kind {
        Kind::Plain | Kind::Sized | Kind::Grid => {
            let chart = scatter().fill(Color::hex(0xd62728)).window(window);
            let mut chart = match kind {
                Kind::Plain => chart.radius(3.0),
                Kind::Sized => chart.size(|r| r.s),
                _ => chart
                    .size(|r| r.s)
                    .size_scale(Pow::sqrt().range(Px(2.0), Px(5.0)))
                    .x_scale(Linear::new().domain(-1.0, 20.0))
                    .y_scale(Linear::new().domain(-1.0, 20.0)),
            };
            chart.layer_mut().max_chunk_rows(CHUNK).retain(retain);
            chart.into_plot()
        }
        Kind::Line => {
            let mut chart = gup_core::line(rows.clone())
                .x(|r| r.x)
                .y(|r| r.y)
                .stroke(Color::hex(0x1f77b4))
                .width(2.0)
                .window(window);
            chart.layers_mut()[0].max_chunk_rows(CHUNK).retain(retain);
            chart.into_plot()
        }
    };
    plot.chunk_culling(true);
    plot
}

fn appender(plot: &mut Plot, kind: Kind) -> Appender<R> {
    match kind {
        Kind::Line => plot.layer_mut::<R, Line>().unwrap().appender(),
        _ => plot.layer_mut::<R, Circle>().unwrap().appender(),
    }
}

fn held(plot: &mut Plot, kind: Kind) -> usize {
    match kind {
        Kind::Line => plot.layer_mut::<R, Line>().unwrap().len(),
        _ => plot.layer_mut::<R, Circle>().unwrap().len(),
    }
}

fn id(row: usize) -> RowId {
    RowId { layer: 0, row }
}

/// A windowed run: AC2's batches to 2,000 rows, through an appender,
/// one resolve per batch.
struct Run {
    plot: Plot,
    feed: Appender<R>,
    kind: Kind,
    total: usize,
    /// Bytes written per kind of upload since before the first resolve.
    start: gup_core::UploadStats,
    /// Rows uploaded: each resolve uploads the appended rows it still
    /// holds (rows a batch pushed past the window are never uploaded).
    uploaded: usize,
}

impl Run {
    fn new(cx: &Context, kind: Kind, retain: Retain) -> Self {
        let mut plot = plot_of(kind, vec![data(kind)(0)], Window::LastRows(N), retain);
        let feed = appender(&mut plot, kind);
        let start = cx.upload_stats();
        plot.resolve(cx, W as f32, H as f32).unwrap();
        Self {
            plot,
            feed,
            kind,
            total: 1,
            start,
            uploaded: 1,
        }
    }

    /// The first row id still held.
    fn first(&mut self) -> usize {
        self.total - held(&mut self.plot, self.kind)
    }

    /// Append `n` rows and resolve.
    fn feed(&mut self, cx: &Context, n: usize) {
        let f = data(self.kind);
        self.feed
            .extend((self.total..self.total + n).map(f))
            .unwrap();
        let before = self.total;
        self.total += n;
        self.plot.resolve(cx, W as f32, H as f32).unwrap();
        self.uploaded += self.total - self.first().max(before);
    }

    /// AC2's batches after the first row, to 2,000 rows, checking the
    /// window after each resolve (`check` also gets the run).
    fn run_to_end(&mut self, cx: &Context, mut check: impl FnMut(&mut Self)) {
        for &n in BATCHES.iter().cycle().skip(1) {
            let n = n.min(TOTAL - self.total);
            if n == 0 {
                break;
            }
            self.feed(cx, n);
            let held = held(&mut self.plot, self.kind);
            assert!(
                held >= N.min(self.total) && held <= N + 2 * CHUNK as usize,
                "{:?}: {held} rows held of {}",
                self.kind,
                self.total
            );
            check(self);
        }
        assert_eq!(self.total, TOTAL);
    }

    /// A fresh, unwindowed plot of exactly the rows still held.
    fn fresh(&mut self) -> Plot {
        let first = self.first();
        let rows = (first..self.total).map(data(self.kind)).collect();
        plot_of(self.kind, rows, Window::All, Retain::Auto)
    }
}

/// Every one of the newest `N` rows is drawn: the pick index locates it.
fn newest_are_drawn(run: &mut Run) {
    for r in run.total.saturating_sub(N)..run.total {
        assert!(
            run.plot.pick_anchor(id(r)).is_some(),
            "{:?}: row {r} of {} not drawn",
            run.kind,
            run.total
        );
    }
}

/// Column bytes per row: x and y, and the size.
fn stride(kind: Kind) -> u64 {
    match kind {
        Kind::Plain | Kind::Line => 8,
        Kind::Sized | Kind::Grid => 12,
    }
}

fn vr(image: &image::RgbaImage) -> gup_visual_regression::RgbaImage {
    gup_visual_regression::RgbaImage::new(image.width(), image.height(), image.as_raw().clone())
        .unwrap()
}

/// AC2 for one layer kind under one policy.
fn window_run(cx: &Context, kind: Kind, retain: Retain) -> Run {
    let mut run = Run::new(cx, kind, retain);
    let cpu = retain != Retain::GpuOnly;
    run.run_to_end(cx, |run| {
        if cpu {
            newest_are_drawn(run);
        }
    });
    let image = run.plot.render(cx, W, H).unwrap();
    let written = cx.upload_stats() - run.start;
    // Eviction writes no column bytes; a size order never compacted here.
    assert!(run.uploaded < TOTAL, "some rows left before their upload");
    assert_eq!(
        written.columns.bytes,
        run.uploaded as u64 * stride(kind),
        "{kind:?} {retain:?}: {written:?}"
    );
    eprintln!("{kind:?} {retain:?}: {written:?}");
    let expected = run.fresh().render(cx, W, H).unwrap();
    // The draw order is a fresh plot's, row for row (the `DrawOrder` unit
    // tests assert it, ties included). The pixels are not always byte-
    // equal: the windowed layer's chunks start at different rows than the
    // fresh plot's, so a mark's position is summed from a different chunk
    // origin and a software rasteriser (lavapipe) can round an antialiased
    // edge pixel the other way. So compare within the harness tolerance.
    let tol = gup_visual_regression::DiffTolerance::default();
    let stats =
        gup_visual_regression::diff::perceptual_diff(&vr(&expected), &vr(&image), &tol).unwrap();
    eprintln!("{kind:?} {retain:?}: vs fresh: {stats}");
    if !stats.within(&tol) {
        let dir = std::env::temp_dir();
        image
            .save(dir.join(format!("gup482_{kind:?}_{retain:?}.png")))
            .unwrap();
        expected
            .save(dir.join(format!("gup482_{kind:?}_{retain:?}_fresh.png")))
            .unwrap();
        panic!("{kind:?} {retain:?}: the windowed image differs from a fresh plot: {stats}");
    }
    if std::env::var_os("GUP482_SAVE").is_some() {
        image
            .save(std::env::temp_dir().join(format!("gup482_{kind:?}.png")))
            .unwrap();
    }
    run
}

/// AC2: the auto x domain follows the window.
fn x_domain(plot: &mut Plot) -> (f64, f64) {
    let rect = plot.layout().expect("resolved").plot;
    let mid = rect.top() + rect.height / 2.0;
    let lo = plot.invert(Point::new(rect.left(), mid)).unwrap().0;
    let hi = plot.invert(Point::new(rect.right(), mid)).unwrap().0;
    (lo, hi)
}

#[test]
fn a_windowed_scatter_equals_a_fresh_plot_of_its_rows() {
    let mut run = window_run(&Context::new_blocking().unwrap(), Kind::Plain, Retain::Auto);
    let first = run.first();
    let (lo, hi) = x_domain(&mut run.plot);
    assert!(
        lo > 1_000.0 && lo <= first as f64 && hi >= (TOTAL - 1) as f64,
        "x domain {lo}..{hi}, rows {first}.."
    );
}

#[test]
fn a_windowed_size_order_equals_a_fresh_plot_of_its_rows() {
    window_run(&Context::new_blocking().unwrap(), Kind::Sized, Retain::Auto);
}

/// A line starts at its first kept row: the bridge into the first kept
/// chunk left with the evicted chunk.
#[test]
fn a_windowed_line_equals_a_fresh_plot_of_its_rows() {
    let mut run = window_run(&Context::new_blocking().unwrap(), Kind::Line, Retain::Auto);
    let (lo, _) = x_domain(&mut run.plot);
    assert!(lo > 1_000.0, "x domain from {lo}");
}

/// AC3: under each policy the run keeps its window, writes only the
/// appended rows and draws what a fresh plot draws; picks work as far as
/// the policy keeps data (GPU picks only under `GpuOnly`).
#[test]
fn windows_work_under_every_retain_policy() {
    let cx = Context::new_blocking().unwrap();
    for retain in [Retain::Rows, Retain::Columns, Retain::GpuOnly] {
        let mut run = window_run(&cx, Kind::Grid, retain);
        let first = run.first();
        for r in (first..TOTAL).step_by(23) {
            let at = grid_point(&mut run.plot, r);
            let gpu = run.plot.pick_row_gpu(&cx, at).unwrap();
            assert_eq!(gpu, Some(id(r)), "{retain:?}: GPU pick of row {r}");
            if retain == Retain::GpuOnly {
                continue;
            }
            assert_eq!(run.plot.pick_row(at), Some(id(r)), "{retain:?}: row {r}");
            let row = run.plot.pick::<R>(at).cloned();
            assert_eq!(
                row,
                (retain == Retain::Rows).then(|| cell(r)),
                "{retain:?}: row {r}"
            );
        }
    }
}

/// Where grid row `r` is drawn (fixed domains, so it never moves).
fn grid_point(plot: &mut Plot, r: usize) -> Point {
    let l = plot.layout().expect("resolved");
    let ticks = |t: &[(f64, f32)], v: f64| {
        let (a, b) = (t[0], *t.last().unwrap());
        (f64::from(a.1) + (v - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)) as f32
    };
    let c = cell(r);
    Point::new(ticks(&l.x_ticks, c.x), ticks(&l.y_ticks, c.y))
}

/// The rows of the grid run held, and an evicted row whose cell no held
/// row covers.
fn grid_run(cx: &Context) -> Run {
    let mut run = Run::new(cx, Kind::Grid, Retain::Auto);
    run.run_to_end(cx, newest_are_drawn);
    run
}

/// AC3: CPU picks return each held row's id (counted from the first row
/// ever) and `&T`; an evicted row is gone from picks and `Plot::row`.
#[test]
fn picks_report_held_rows_after_evictions() {
    let cx = Context::new_blocking().unwrap();
    let mut run = grid_run(&cx);
    let first = run.first();
    assert!(first > 1_500, "{first} rows evicted");
    for r in first..TOTAL {
        let at = run.plot.pick_anchor(id(r)).expect("held");
        assert!((at.x - grid_point(&mut run.plot, r).x).abs() < 0.5);
        assert_eq!(run.plot.pick_row(at), Some(id(r)));
        assert_eq!(run.plot.pick::<R>(at), Some(&cell(r)));
        assert_eq!(run.plot.row::<R>(id(r)), Some(&cell(r)));
    }
    // Evicted rows whose cell no held row covers: nothing there.
    let covered: Vec<usize> = (first..TOTAL).map(|r| r % 400).collect();
    let mut gone = 0;
    for r in first - 400..first {
        assert_eq!(run.plot.row::<R>(id(r)), None, "evicted row {r}");
        assert_eq!(run.plot.pick_anchor(id(r)), None, "evicted row {r}");
        if !covered.contains(&(r % 400)) {
            let at = grid_point(&mut run.plot, r);
            assert_eq!(run.plot.pick_row(at), None, "evicted row {r}");
            gone += 1;
        }
    }
    assert!(gone > 50, "{gone} empty cells checked");
}

/// AC3: GPU picks agree with CPU picks on held rows, and a pick drawn
/// before an eviction and polled after it reports no row.
#[test]
fn gpu_picks_agree_and_do_not_cross_an_eviction() {
    let cx = Context::new_blocking().unwrap();
    let mut run = grid_run(&cx);
    let first = run.first();
    let held = TOTAL - first;
    for r in (0..10).map(|k| first + k * held / 10) {
        let at = grid_point(&mut run.plot, r);
        let cpu = run.plot.pick_row(at);
        assert_eq!(cpu, Some(id(r)));
        assert_eq!(run.plot.pick_row_gpu(&cx, at).unwrap(), cpu, "row {r}");
    }
    // The newest row stays held; the oldest go.
    let at = grid_point(&mut run.plot, TOTAL - 1);
    let ticket = run.plot.request_pick(&cx, at).unwrap();
    run.feed(&cx, 100);
    assert!(run.first() > first, "the batch evicted");
    // Picked again (waiting for the device), it finds the row; the first
    // pick, read back by then, reports none.
    assert_eq!(run.plot.pick_row_gpu(&cx, at).unwrap(), Some(id(TOTAL - 1)));
    let picks = run.plot.poll_picks(&cx);
    let pick = picks.iter().find(|p| p.ticket == ticket).expect("polled");
    assert_eq!(pick.row, None, "a pick across an eviction is dropped");
}

fn green_at(image: &image::RgbaImage, at: Point) -> u8 {
    image.get_pixel(at.x.round() as u32, at.y.round() as u32).0[1]
}

fn drag(plot: &mut Plot, from: Point, to: Point) {
    let button = gup_core::Button::Primary;
    let _ = plot.handle(Input::PointerDown { at: from, button });
    let _ = plot.handle(Input::PointerMoved { at: to });
    let _ = plot.handle(Input::PointerUp { at: to, button });
}

/// Brushed rows are lit (red, low green); every other held row is dimmed
/// (faded toward the background, high green).
fn check_brush(cx: &Context, run: &mut Run) {
    let image = run.plot.render(cx, W, H).unwrap();
    let brushed: Vec<usize> = run
        .plot
        .brush()
        .expect("brushed")
        .rows
        .iter()
        .map(|r| r.row)
        .collect();
    let first = run.first();
    let mut lit = 0;
    for r in first..run.total {
        let g = green_at(&image, grid_point(&mut run.plot, r));
        if brushed.contains(&r) {
            assert!(g < 90, "brushed row {r}: green {g}");
            lit += 1;
        } else {
            assert!(g > 120, "unbrushed row {r}: green {g}");
        }
    }
    assert!(lit > 10, "{lit} lit rows");
}

/// AC3: a brush over held rows dims exactly the others, before and after a
/// further eviction.
#[test]
fn brush_bits_follow_evictions() {
    let cx = Context::new_blocking().unwrap();
    let mut run = grid_run(&cx);
    run.plot.drag(Drag::Brush);
    run.plot.resolve(&cx, W as f32, H as f32).unwrap();
    // The lower half of the grid's columns 2..12.
    let first = run.first();
    let a = grid_point(&mut run.plot, 2 + 20 * 19);
    let b = grid_point(&mut run.plot, 11);
    let (a, b) = (
        Point::new(a.x - 6.0, a.y - 6.0),
        Point::new(b.x + 6.0, b.y + 6.0),
    );
    drag(&mut run.plot, a, b);
    let brushed = run.plot.brush().expect("brushed").rows.len();
    assert!(brushed > 50, "{brushed} rows brushed, held from {first}");
    check_brush(&cx, &mut run);
    run.feed(&cx, 100);
    assert!(run.first() > first, "the batch evicted");
    check_brush(&cx, &mut run);
}

/// AC3: culling (S9f) on and off draw the same image after evictions.
#[test]
fn culling_matches_after_evictions() {
    let cx = Context::new_blocking().unwrap();
    let mut run = grid_run(&cx);
    // Only grid rows 0..7 are inside: whole chunks are outside.
    let y = run.plot.y(Linear::new().domain(-1.0, 7.0));
    drop(y);
    let culled = run.plot.render(&cx, W, H).unwrap();
    run.plot.chunk_culling(false);
    let all = run.plot.render(&cx, W, H).unwrap();
    assert!(culled == all, "culling changed the image");
}

/// AC3: events never fire for an evicted row, and a hovered row that is
/// evicted is no longer hovered.
#[test]
fn evicted_rows_fire_no_events() {
    let cx = Context::new_blocking().unwrap();
    let mut run = grid_run(&cx);
    let seen: Arc<Mutex<Vec<(Event, usize)>>> = Arc::default();
    let log = Arc::clone(&seen);
    run.plot.on_row(move |info| {
        log.lock().unwrap().push((info.event, info.row.row));
    });
    let first = run.first();
    let oldest = grid_point(&mut run.plot, first);
    let _ = run.plot.handle(Input::PointerMoved { at: oldest });
    run.plot.resolve(&cx, W as f32, H as f32).unwrap();
    assert_eq!(run.plot.hovered(), Some(id(first)));
    run.feed(&cx, 100);
    let evicted = run.first();
    assert!(evicted > first, "the batch evicted");
    assert_eq!(run.plot.hovered(), None, "the hovered row was evicted");
    seen.lock().unwrap().clear();
    // Over every evicted row's old place, and click it.
    for r in first..evicted {
        let at = grid_point(&mut run.plot, r);
        let _ = run.plot.handle(Input::PointerMoved { at });
        let button = gup_core::Button::Primary;
        let _ = run.plot.handle(Input::PointerDown { at, button });
        let _ = run.plot.handle(Input::PointerUp { at, button });
        run.plot.resolve(&cx, W as f32, H as f32).unwrap();
    }
    let seen = seen.lock().unwrap();
    assert!(
        seen.iter().all(|&(_, r)| r >= evicted),
        "events for evicted rows: {seen:?}"
    );
}
