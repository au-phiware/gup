// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The level of detail (GUP-716): `Lod::Auto` draws a bounded number of
//! representatives per visible chunk above `Lod::AUTO_MIN_ROWS` visible
//! rows, and changes nothing at or under it, or for marks that do not take
//! it. Set `GUP_LOD_PNG_DIR` to save the 1M-row scatter under `Auto` and
//! `Off` for reading by eye.

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
    let scene: Scene = plot.resolve(cx, W as f32, H as f32).unwrap().scene.clone();
    let batch = scene
        .items
        .iter()
        .find_map(|i| match &i.kind {
            gup_core::scene::ItemKind::Marks(b) => Some(b.clone()),
            _ => None,
        })
        .expect("a data layer");
    let mut target = ImageTarget::new(cx, W, H).unwrap();
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

#[test]
fn a_million_row_walk_draws_one_draw_per_chunk_and_at_most_four_per_pixel() {
    let cx = Context::new_blocking().unwrap();
    let rows = 1_000_000;
    let mut auto = scatter(walk(rows), Lod::Auto);
    let a = draw(&cx, &mut auto);
    let pixels = plot_rect_pixels(&auto);
    let mut off = scatter(walk(rows), Lod::Off);
    let o = draw(&cx, &mut off);
    save(&a.image, "lod-auto-1m.png");
    save(&o.image, "lod-off-1m.png");
    // The guides draw the same in both: the marks' draws are the
    // difference, and the layer has one instanced draw per chunk in both.
    assert_eq!(a.draw_calls, o.draw_calls);
    assert_eq!(a.chunks, o.chunks);
    assert!(a.chunks >= 1);
    assert_eq!(o.instances, rows as u64);
    assert!(
        (a.instances as f64) <= pixels,
        "{} instances over {pixels} plot pixels",
        a.instances
    );
    // GUP-631's fidelity measure: pixels more than ΔE00 10 from the full
    // draw, as a share of the image and of the plot rect.
    let r = auto.layout().expect("resolved").plot;
    let (mut image, mut rect) = (0u32, 0u32);
    for (x, y, p) in a.image.enumerate_pixels() {
        let q = o.image.get_pixel(x, y);
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
    let image_share = 100.0 * f64::from(image) / f64::from(W * H);
    let rect_share = 100.0 * f64::from(rect) / pixels;
    eprintln!(
        "1M walk at {W} x {H}: {} representatives, {} chunks, {pixels} plot pixels; \
         ΔE00 > 10: {image_share:.3}% of the image, {rect_share:.3}% of the plot rect",
        a.instances, a.chunks
    );
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
