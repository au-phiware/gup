// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The level of detail (GUP-716): `Lod::Auto` draws a bounded number of
//! representatives per visible chunk above `Lod::AUTO_MIN_ROWS` visible
//! rows, and changes nothing at or under it, or for marks that do not take
//! it. Set `GUP_LOD_PNG_DIR` to save the 1M-row scatter under `Auto` and
//! `Off` for reading by eye.

mod common;

use gup_core::prelude::*;
use gup_core::{ImageTarget, Renderer, Scene};

const W: u32 = 400;
const H: u32 = 300;

/// `n` rows of a sorted random walk (x over 0..1000, y uniform steps of
/// ±0.5), the GUP-631 spike's data; a third value for a size channel.
fn walk(n: usize) -> Vec<(f64, f64, f64)> {
    let mut s = 0x2545_f491_4f6c_dd1du64;
    let mut y = 0.0;
    (0..n)
        .map(|i| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            y += (s >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
            (1000.0 * i as f64 / n as f64, y, (i % 7) as f64)
        })
        .collect()
}

type Row = (f64, f64, f64);

/// A plot over explicit linear scales, so tests can zoom.
fn plot() -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>) {
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    (plot, x, y)
}

/// A scatter of `rows` as 1.5 px circles under `lod`.
fn scatter(rows: Vec<Row>, lod: Lod) -> Plot {
    let (mut plot, x, y) = plot();
    plot.add(Selection::<Row, Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(Circle::RADIUS, Px(1.5))
        .lod(lod);
    plot
}

/// What one frame of `plot` draws: the image, the draw calls and the
/// instances of its first data layer.
struct Drawn {
    image: image::RgbaImage,
    draw_calls: usize,
    instances: u64,
    chunks: usize,
}

fn draw(cx: &Context, plot: &mut Plot) -> Drawn {
    draw_at(cx, plot, W, H)
}

fn draw_at(cx: &Context, plot: &mut Plot, w: u32, h: u32) -> Drawn {
    let scene: Scene = plot.resolve(cx, w as f32, h as f32).unwrap().scene.clone();
    let batch = scene
        .items
        .iter()
        .find_map(|i| match &i.kind {
            gup_core::scene::ItemKind::Marks(b) => Some(b.clone()),
            _ => None,
        })
        .expect("a data layer");
    let mut target = ImageTarget::new(cx, w, h).unwrap();
    let desc = gup_core::RenderTarget::desc(&target);
    let draw_calls = Renderer::new()
        .prepare(cx, &scene, &desc)
        .unwrap()
        .draw_calls();
    let image = target.render_blocking(cx, &scene).unwrap();
    Drawn {
        image,
        draw_calls,
        instances: batch.instances(),
        chunks: batch.chunks(),
    }
}

fn builds(plot: &mut Plot) -> u64 {
    gup_core::__private::lod_tier_builds(plot.layer_mut::<Row, Circle>().unwrap())
}

fn save(image: &image::RgbaImage, name: &str) {
    if let Some(dir) = std::env::var_os("GUP_LOD_PNG_DIR") {
        let path = std::path::Path::new(&dir).join(name);
        image.save(&path).unwrap();
    }
}

/// The plot rect's area in pixels, from the layout.
fn plot_rect_pixels(plot: &Plot) -> f64 {
    let r = plot.layout().expect("resolved").plot;
    f64::from(r.width) * f64::from(r.height)
}

#[test]
fn at_the_threshold_nothing_changes_and_one_row_more_takes_lod() {
    let cx = Context::new_blocking().unwrap();
    let rows = Lod::AUTO_MIN_ROWS as usize;
    let mut auto = scatter(walk(rows), Lod::Auto);
    let mut off = scatter(walk(rows), Lod::Off);
    let (a, o) = (draw(&cx, &mut auto), draw(&cx, &mut off));
    assert_eq!(builds(&mut auto), 0, "no tiers at the threshold");
    assert_eq!(a.instances, rows as u64);
    assert_eq!(a.draw_calls, o.draw_calls);
    assert!(
        a.image.as_raw() == o.image.as_raw(),
        "Auto differs from Off at the threshold"
    );

    let mut over = scatter(walk(rows + 1), Lod::Auto);
    let d = draw(&cx, &mut over);
    assert_eq!(builds(&mut over), 1, "one chunk, built once");
    assert!(d.instances < rows as u64 / 4, "{} instances", d.instances);
    // Resolving again builds nothing more.
    draw(&cx, &mut over);
    assert_eq!(builds(&mut over), 1);
}

/// A scatter of `rows` over explicit scales, so a test can zoom.
fn zoomable(rows: usize, lod: Lod) -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>) {
    let (mut plot, x, y) = plot();
    plot.add(Selection::<Row, Circle>::new(walk(rows)))
        .attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(Circle::RADIUS, Px(1.5))
        .lod(lod);
    (plot, x, y)
}

/// Zoom both axes of a resolved plot to a tenth of their view, about the
/// point `(cx, cy)`.
fn zoom_ten(plot: &Plot, x: &ScaleRef<Linear>, y: &ScaleRef<Linear>, (cx, cy): (f64, f64)) {
    let (x0, x1) = plot.x_view().expect("resolved");
    let (y0, y1) = plot.y_view().expect("resolved");
    let (hx, hy) = ((x1 - x0) / 20.0, (y1 - y0) / 20.0);
    x.write().set_domain(cx - hx, cx + hx).unwrap();
    y.write().set_domain(cy - hy, cy + hy).unwrap();
}

/// GUP-631's fidelity measure: the share of pixels more than ΔE00 10 from
/// the full draw, of the image and of the plot rect.
fn shares(auto: &image::RgbaImage, off: &image::RgbaImage, plot: &Plot) -> (f64, f64) {
    let r = plot.layout().expect("resolved").plot;
    let (mut image, mut rect) = (0u32, 0u32);
    for (x, y, p) in auto.enumerate_pixels() {
        let q = off.get_pixel(x, y);
        let pa = gup_visual_regression::Rgba8::new(p[0], p[1], p[2], p[3]);
        let pb = gup_visual_regression::Rgba8::new(q[0], q[1], q[2], q[3]);
        if pa.delta_e(pb) > 10.0 {
            image += 1;
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            if fx >= r.x && fx < r.x + r.width && fy >= r.y && fy < r.y + r.height {
                rect += 1;
            }
        }
    }
    (
        100.0 * f64::from(image) / f64::from(auto.width() * auto.height()),
        100.0 * f64::from(rect) / plot_rect_pixels(plot),
    )
}

/// Shares of the plot rect's pixels over ΔE00 10 from the exact draw.
struct Fidelity {
    full: f64,
    zoom: f64,
}

/// `rows` rows at `w` x `h` under `Auto` against `Off`, in full view and
/// zoomed ten times on both axes; prints both shares.
fn fidelity(w: u32, h: u32, rows: usize) -> Fidelity {
    let cx = Context::new_blocking().unwrap();
    let (mut auto, ax, ay) = zoomable(rows, Lod::Auto);
    let (mut off, ox, oy) = zoomable(rows, Lod::Off);
    let (a, o) = (draw_at(&cx, &mut auto, w, h), draw_at(&cx, &mut off, w, h));
    let pixels = plot_rect_pixels(&auto);
    save(&a.image, &format!("lod-auto-{w}x{h}.png"));
    save(&o.image, &format!("lod-off-{w}x{h}.png"));
    // The guides draw the same in both, and the layer has one instanced
    // draw per chunk in both.
    assert_eq!(a.draw_calls, o.draw_calls);
    assert_eq!(a.chunks, o.chunks);
    assert!(a.chunks >= 1);
    assert_eq!(o.instances, rows as u64);
    assert!(
        (a.instances as f64) < rows as f64 / 10.0,
        "{} instances of {rows} rows",
        a.instances
    );
    assert!(
        (a.instances as f64) <= pixels,
        "{} instances over {pixels} plot pixels",
        a.instances
    );
    let (full_image, full) = shares(&a.image, &o.image, &auto);
    eprintln!(
        "{rows} rows at {w} x {h}, full view: {} representatives, {} chunks, {pixels} plot \
         pixels; ΔE00 > 10: {full_image:.3}% of the image, {full:.3}% of the plot rect",
        a.instances, a.chunks
    );

    // About the walk's middle row, so the zoom shows marks.
    let middle = walk(rows)[rows / 2];
    zoom_ten(&auto, &ax, &ay, (middle.0, middle.1));
    zoom_ten(&off, &ox, &oy, (middle.0, middle.1));
    let (za, zo) = (draw_at(&cx, &mut auto, w, h), draw_at(&cx, &mut off, w, h));
    save(&za.image, &format!("lod-auto-{w}x{h}-zoom.png"));
    save(&zo.image, &format!("lod-off-{w}x{h}-zoom.png"));
    assert_eq!(za.draw_calls, zo.draw_calls);
    let (zoom_image, zoom) = shares(&za.image, &zo.image, &auto);
    eprintln!(
        "{rows} rows at {w} x {h}, 10x zoom: {} representatives of {} rows; \
         ΔE00 > 10: {zoom_image:.3}% of the image, {zoom:.3}% of the plot rect",
        za.instances, zo.instances
    );
    Fidelity { full, zoom }
}

/// The gates are per scene size, from measurements (GUP-717's story has the
/// table and the margins): GUP-714's per-chunk power-of-two grid is coarser
/// than the spike's, and a small chart has more rows per pixel.
#[test]
fn a_million_row_walk_matches_the_exact_draw_at_400_by_300() {
    // Measured 1.566% / 2.287%; the gates are 1.28x / 1.31x those.
    let f = fidelity(W, H, 1_000_000);
    assert!(f.full <= 2.0, "full view {:.3}% of the plot rect", f.full);
    assert!(f.zoom <= 3.0, "10x zoom {:.3}% of the plot rect", f.zoom);
}

#[test]
fn a_million_row_walk_matches_the_exact_draw_at_1080p() {
    // Measured 0.571% / 0.000% (every row draws: the tiers have run out);
    // the full gate is 1.3x, the zoom gate allows a few antialiasing pixels.
    let f = fidelity(1920, 1080, 1_000_000);
    assert!(f.full <= 0.75, "full view {:.3}% of the plot rect", f.full);
    assert!(f.zoom <= 0.1, "10x zoom {:.3}% of the plot rect", f.zoom);
}

/// The golden of the `Auto` full view (GUP-717): a continuous band, the
/// axes and ticks, nothing outside the plot rect.
#[test]
fn a_million_row_lod_scatter_is_a_golden() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, _, _) = zoomable(1_000_000, Lod::Auto);
    let (image, layout) = plot.render_resolved(&cx, W, H).unwrap();
    let vr = gup_visual_regression::RgbaImage::new(W, H, image.into_raw()).unwrap();
    common::vr::harness()
        .run(
            "gup_core/lod_scatter_1m",
            Ok((vr, common::vr::layout_metadata(&layout))),
        )
        .assert_ok();
}

#[test]
fn zoomed_far_in_every_row_draws() {
    let cx = Context::new_blocking().unwrap();
    let rows = Lod::AUTO_MIN_ROWS as usize + 1;
    let (mut plot, x, y) = plot();
    plot.add(Selection::<Row, Circle>::new(walk(rows)))
        .attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(Circle::RADIUS, Px(1.5));
    assert!(draw(&cx, &mut plot).instances < rows as u64);
    // A millionth of the x span: every cell of the finest stored tier is
    // many pixels, so the chunk is past its tiers and draws in full.
    x.write().set_domain(500.0, 500.001).unwrap();
    let d = draw(&cx, &mut plot);
    assert_eq!(d.instances, rows as u64);
}

#[test]
fn tiers_build_one_chunk_a_frame_with_a_host_and_all_at_once_without() {
    let cx = Context::new_blocking().unwrap();
    let make = || {
        let (mut plot, x, y) = plot();
        plot.add(Selection::<Row, Circle>::new(walk(300_000)))
            .attr(Circle::X, x.encode(|r: &Row| r.0))
            .attr(Circle::Y, y.encode(|r: &Row| r.1))
            .attr(Circle::RADIUS, Px(1.5))
            .max_chunk_rows(100_000);
        plot
    };
    // Headless: the first frame is complete.
    let mut headless = make();
    let d = draw(&cx, &mut headless);
    assert_eq!(d.chunks, 3);
    assert_eq!(builds(&mut headless), 3);
    assert!(!Chart::needs_frame(&headless));

    // Under a host that draws again when asked: one chunk a frame, and a
    // redraw is asked for until every visible chunk has its tiers.
    let mut hosted = make();
    Chart::set_waker(&mut hosted, std::task::Waker::noop().clone());
    let mut seen = Vec::new();
    for _ in 0..4 {
        let d = draw(&cx, &mut hosted);
        seen.push((
            builds(&mut hosted),
            Chart::needs_frame(&hosted),
            d.instances,
        ));
    }
    assert_eq!(
        seen.iter().map(|s| (s.0, s.1)).collect::<Vec<_>>(),
        [(1, true), (2, true), (3, false), (3, false)]
    );
    // Unbuilt chunks draw in full, so each frame draws fewer instances.
    assert!(seen[0].2 > seen[1].2 && seen[1].2 > seen[2].2, "{seen:?}");
    assert_eq!(seen[2].2, seen[3].2);
    // The complete frame is the headless one.
    assert_eq!(seen[3].2, d.instances);
}

#[test]
fn a_released_layer_builds_its_full_chunks_before_release() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, x, y) = plot();
    plot.add(Selection::<Row, Circle>::new(walk(300_001)))
        .attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(Circle::RADIUS, Px(1.5))
        .retain(Retain::GpuOnly)
        .max_chunk_rows(100_000);
    Chart::set_waker(&mut plot, std::task::Waker::noop().clone());
    // Zoomed in so far that no chunk takes a tier, and fewer rows than the
    // threshold visible: yet the three full chunks build, since release
    // drops their CPU copies.
    x.write().set_domain(10.0, 10.001).unwrap();
    draw(&cx, &mut plot);
    assert_eq!(builds(&mut plot), 3);
    assert!(plot.layer_mut::<Row, Circle>().unwrap().rows_released());
    // Zoomed out, the released chunks draw their representatives.
    x.write().set_domain(0.0, 1000.0).unwrap();
    let d = draw(&cx, &mut plot);
    assert!(d.instances < 100_000, "{} instances", d.instances);
}

#[test]
fn off_draws_every_row() {
    let cx = Context::new_blocking().unwrap();
    let mut off = scatter(walk(300_000), Lod::Off);
    assert_eq!(draw(&cx, &mut off).instances, 300_000);
    assert_eq!(builds(&mut off), 0);
}

/// Asserts `Auto` and `Off` draw the same image with the same draw calls
/// and every row, for a layer added by `add`.
fn unchanged(rows: usize, add: impl Fn(&mut Plot, &ScaleRef<Linear>, &ScaleRef<Linear>, Lod)) {
    let cx = Context::new_blocking().unwrap();
    let mut drawn = Vec::new();
    for lod in [Lod::Auto, Lod::Off] {
        let (mut plot, x, y) = plot();
        add(&mut plot, &x, &y, lod);
        drawn.push(draw(&cx, &mut plot));
    }
    let (a, o) = (&drawn[0], &drawn[1]);
    assert_eq!(a.draw_calls, o.draw_calls);
    assert_eq!(a.instances, o.instances);
    assert!(a.instances >= rows as u64 - 1, "{} instances", a.instances);
    assert!(
        a.image.as_raw() == o.image.as_raw(),
        "Auto differs from Off"
    );
}

#[test]
fn a_layer_drawn_by_size_ignores_lod() {
    unchanged(1_000_000, |plot, x, y, lod| {
        plot.add(Selection::<Row, Circle>::new(walk(1_000_000)))
            .attr(Circle::X, x.encode(|r: &Row| r.0))
            .attr(Circle::Y, y.encode(|r: &Row| r.1))
            .attr(
                Circle::RADIUS,
                Linear::new().range(Px(1.0), Px(3.0)).encode(|r: &Row| r.2),
            )
            .lod(lod);
    });
}

#[test]
fn a_line_ignores_lod() {
    unchanged(1_000_000, |plot, x, y, lod| {
        plot.add(Selection::<Row, Line>::new(walk(1_000_000)))
            .attr(Line::X, x.encode(|r: &Row| r.0))
            .attr(Line::Y, y.encode(|r: &Row| r.1))
            .lod(lod);
    });
}

#[test]
fn an_area_ignores_lod() {
    unchanged(1_000_000, |plot, x, y, lod| {
        plot.add(Selection::<Row, Area>::new(walk(1_000_000)))
            .attr(Area::X, x.encode(|r: &Row| r.0))
            .attr(Area::Y, y.encode(|r: &Row| r.1))
            .attr(Area::Y_END, y.encode(|r: &Row| r.1 - 1.0))
            .lod(lod);
    });
}

#[test]
fn a_segment_ignores_lod() {
    unchanged(1_000_000, |plot, x, y, lod| {
        plot.add(Selection::<Row, Segment>::new(walk(1_000_000)))
            .attr(Segment::X, x.encode(|r: &Row| r.0))
            .attr(Segment::Y, y.encode(|r: &Row| r.1))
            .attr(Segment::X_END, x.encode(|r: &Row| r.0 + 0.5))
            .attr(Segment::Y_END, y.encode(|r: &Row| r.1 + 0.5))
            .lod(lod);
    });
}

#[test]
fn a_rect_with_per_row_extents_ignores_lod() {
    unchanged(1_000_000, |plot, x, y, lod| {
        plot.add(Selection::<Row, Rect>::new(walk(1_000_000)))
            .attr(Rect::X, x.encode(|r: &Row| r.0))
            .attr(Rect::Y, y.encode(|r: &Row| r.1))
            .attr(Rect::X_END, x.encode(|r: &Row| r.0 + 0.5))
            .attr(Rect::Y_END, y.encode(|r: &Row| r.1 + 0.5))
            .lod(lod);
    });
}

#[test]
fn a_rect_of_constant_extents_takes_lod() {
    let cx = Context::new_blocking().unwrap();
    let rows = 1_000_000;
    let (mut plot, x, y) = plot();
    plot.add(Selection::<Row, Rect>::new(walk(rows)))
        .attr(Rect::X, x.encode(|r: &Row| r.0))
        .attr(Rect::Y, y.encode(|r: &Row| r.1))
        .attr(Rect::WIDTH, Px(2.0))
        .attr(Rect::HEIGHT, Px(2.0));
    let d = draw(&cx, &mut plot);
    assert!(d.instances < rows as u64 / 4, "{} instances", d.instances);
}

#[test]
fn the_scatter_builder_forwards_lod() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(f64, f64)> = walk(300_000).into_iter().map(|r| (r.0, r.1)).collect();
    let mut drawn = Vec::new();
    for lod in [Lod::Auto, Lod::Off] {
        let mut chart = gup_core::scatter(rows.clone())
            .x(|p| p.0)
            .y(|p| p.1)
            .lod(lod);
        let scene = chart
            .resolve(&cx, W as f32, H as f32)
            .unwrap()
            .scene
            .clone();
        let instances = scene
            .items
            .iter()
            .find_map(|i| match &i.kind {
                gup_core::scene::ItemKind::Marks(b) => Some(b.instances()),
                _ => None,
            })
            .unwrap();
        drawn.push(instances);
    }
    assert!(drawn[0] < 100_000, "{drawn:?}");
    assert_eq!(drawn[1], 300_000);
}
