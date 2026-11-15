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

// GUP-718: what you see is what you pick.

/// A scatter of `rows` as shared columns under `Retain::GpuOnly`, as a layer
/// above `Retain::AUTO_MAX_ROWS` rows is (its CPU columns are not kept, so
/// the plot would pick it on the GPU), with 3 px discs.
fn columnar(rows: &[Row], lod: Lod) -> Plot {
    let cols = Columns::new()
        .with("x", rows.iter().map(|r| r.0).collect::<Vec<_>>())
        .unwrap()
        .with("y", rows.iter().map(|r| r.1).collect::<Vec<_>>())
        .unwrap();
    let (mut plot, ..) = plot();
    plot.add_columns::<Circle>(&cols)
        .x_column(&cols["x"])
        .y_column(&cols["y"])
        .attr(Circle::RADIUS, Px(3.0))
        .retain(Retain::GpuOnly)
        .lod(lod);
    plot
}

/// Pixels on a grid over the plot rect, `step` apart.
fn grid(plot: &Plot, step: f32) -> Vec<gup_core::geom::Point> {
    let r = plot.layout().expect("resolved").plot;
    let mut out = Vec::new();
    let mut y = r.top() + step / 2.0;
    while y < r.bottom() {
        let mut x = r.left() + step / 2.0;
        while x < r.right() {
            out.push(gup_core::geom::Point::new(x, y));
            x += step;
        }
        y += step;
    }
    out
}

#[test]
fn the_pick_index_holds_the_drawn_representatives() {
    let cx = Context::new_blocking().unwrap();
    let rows = walk(300_000);
    let mut lod = columnar(&rows, Lod::Auto);
    let d = draw(&cx, &mut lod);
    assert!(d.instances < 30_000, "{} drawn", d.instances);
    let items = lod.pick_item_count().expect("an index");
    assert!(items > 0 && items <= d.instances as usize, "{items} items");
    assert!(items < rows.len() / 10, "{items} items of {}", rows.len());

    // Every hit is a data row, drawn where its data says.
    let mut hits = 0;
    for at in grid(&lod, 7.0) {
        let Some(id) = lod.pick_row(at) else { continue };
        hits += 1;
        let (x, y, _) = rows[id.row];
        let p = lod.pixel(x, y).unwrap();
        assert!(
            (p.x - at.x).hypot(p.y - at.y) <= 3.0 + gup_core::PICK_TOLERANCE.0 + 0.5,
            "row {} is at {p:?}, picked at {at:?}",
            id.row
        );
        // Its anchor is its own position, and picking there finds a row.
        let anchor = lod.pick_anchor(id).expect("a drawn row has an anchor");
        assert!((anchor.x - p.x).abs() < 0.01 && (anchor.y - p.y).abs() < 0.01);
        assert!(lod.pick_row(anchor).is_some());
    }
    assert!(hits > 50, "{hits} grid points hit a representative");

    // Under `Off` every row is indexed: unchanged (the layer is under
    // the bound).
    let mut off = columnar(&rows, Lod::Off);
    draw(&cx, &mut off);
    assert_eq!(off.pick_item_count(), Some(rows.len()));
}

#[test]
fn cpu_and_gpu_picks_agree_under_lod() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = scatter(walk(1_000_000), Lod::Auto);
    let d = draw(&cx, &mut plot);
    assert!(d.instances < 100_000);
    let r = plot.layout().unwrap().plot;
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f32 / (1u64 << 53) as f32
    };
    let (mut hits, mut disagree) = (0, 0);
    for _ in 0..200 {
        let q = gup_core::geom::Point::new(r.x + next() * r.width, r.y + next() * r.height);
        let (cpu, gpu) = (plot.pick_row(q), plot.pick_row_gpu(&cx, q).unwrap());
        hits += usize::from(cpu.is_some() || gpu.is_some());
        disagree += usize::from(cpu != gpu);
    }
    assert!(hits >= 20, "only {hits} of 200 points hit a mark");
    assert!(disagree <= 2, "{disagree} of {hits} picks differ");
}

/// The tooltip text shown, if any.
fn tooltip_text(plot: &mut Plot, cx: &Context) -> Option<String> {
    use gup_core::scene::{ItemKind, Z_TOOLTIP};
    let resolved = plot.resolve(cx, W as f32, H as f32).unwrap();
    resolved
        .scene
        .items
        .iter()
        .filter(|i| i.z == Z_TOOLTIP)
        .find_map(|i| match &i.kind {
            ItemKind::Text(runs) => runs.first().map(|r| r.text.to_string()),
            _ => None,
        })
}

fn with_tooltip(rows: Vec<Row>, lod: Lod) -> Plot {
    let (mut plot, x, y) = plot();
    plot.add(Selection::<Row, Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(Circle::RADIUS, Px(4.0))
        .tooltip(|r| format!("x {:.3}, y {:.3}", r.0, r.1))
        .lod(lod);
    plot
}

#[test]
fn hover_outlines_a_representative_and_shows_its_row() {
    let cx = Context::new_blocking().unwrap();
    let rows = walk(1_000_000);
    let text = |r: usize| format!("x {:.3}, y {:.3}", rows[r].0, rows[r].1);
    let mut plot = with_tooltip(rows.clone(), Lod::Auto);
    let d = draw(&cx, &mut plot);
    assert!(d.instances < 100_000);
    let plain = d.image;

    // A representative near the middle of the plot.
    let rect = plot.layout().unwrap().plot;
    let mid = gup_core::geom::Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    let points = grid(&plot, 3.0);
    let hits: Vec<_> = points
        .into_iter()
        .filter_map(|at| plot.pick_row(at))
        .collect();
    let hit = hits
        .into_iter()
        .min_by(|a, b| {
            let d = |r: &gup_core::RowId| {
                let p = plot.pixel(rows[r.row].0, rows[r.row].1).unwrap();
                (p.x - mid.x).hypot(p.y - mid.y)
            };
            d(a).total_cmp(&d(b))
        })
        .expect("a representative under the grid");
    let at = plot.pick_anchor(hit).expect("anchored at its own position");
    let p = plot.pixel(rows[hit.row].0, rows[hit.row].1).unwrap();
    assert!((at.x - p.x).abs() < 0.01 && (at.y - p.y).abs() < 0.01);

    assert!(plot.handle(gup_core::Input::pointer_moved(at)).redraw);
    let hovered = plot.hovered().expect("hovering a representative");
    assert!(hovered.row < rows.len());
    // Ringed at the representative, with the tooltip of its data row.
    let ring = plot.resolve(&cx, W as f32, H as f32).unwrap().layout.hover;
    let ring = ring.expect("a hover ring");
    let anchor = plot.pick_anchor(hovered).unwrap();
    assert!((ring.x + ring.width / 2.0 - anchor.x).abs() <= 1.0);
    assert!((ring.y + ring.height / 2.0 - anchor.y).abs() <= 1.0);
    assert_eq!(tooltip_text(&mut plot, &cx), Some(text(hovered.row)));
    // Under `Off` the same pointer gets the tooltip of the row it shows.
    let mut off = with_tooltip(rows.clone(), Lod::Off);
    draw(&cx, &mut off);
    let _ = off.handle(gup_core::Input::pointer_moved(at));
    let h = off.hovered().expect("a row at the pointer");
    assert_eq!(tooltip_text(&mut off, &cx), Some(text(h.row)));

    let image = draw(&cx, &mut plot).image;
    save(&image, "hover_1m.png");
    // One ring: the hover changes few pixels, all near the anchor.
    let near =
        |x: u32, y: u32| (x as f32 - anchor.x).abs() <= 14.0 && (y as f32 - anchor.y).abs() <= 14.0;
    let (mut changed, mut away) = (0, 0);
    for (x, y, p) in image.enumerate_pixels() {
        if p != plain.get_pixel(x, y) {
            changed += 1;
            // The tooltip is the other change, beside the ring.
            away += usize::from(!near(x, y));
        }
    }
    assert!(changed > 20, "the ring changes {changed} pixels");
    assert!(away < changed, "all changes are away from the pointer");

    // Leaving clears the ring and the tooltip.
    let _ = plot.handle(gup_core::Input::PointerLeft);
    assert_eq!(plot.hovered(), None);
    assert_eq!(tooltip_text(&mut plot, &cx), None);
}

/// `plot` as SVG after resolving it at the test size.
fn svg_of(cx: &Context, plot: &mut Plot, w: u32, h: u32) -> String {
    let scene = plot.resolve(cx, w as f32, h as f32).unwrap().scene.clone();
    let mut target = gup_core::SvgTarget::new();
    gup_core::VectorTarget::render(&mut target, &scene).unwrap();
    target.into_string()
}

fn rasterise(svg: &str, w: u32, h: u32) -> image::RgbaImage {
    use resvg::{tiny_skia, usvg};
    let mut opt = usvg::Options::default();
    opt.fontdb_mut()
        .load_font_data(gup_text::INTER_REGULAR.to_vec());
    let tree = usvg::Tree::from_str(svg, &opt).expect("SvgTarget output parses");
    let mut pixmap = tiny_skia::Pixmap::new(w, h).unwrap();
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    let data = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(w, h, data).unwrap()
}

/// 300,000 rows written as SVG at `w` x `h` under `Auto` and `Off`:
/// the circle counts, the SVG's share of the plot rect over ΔE00 10 from
/// `Off`'s, and the PNG's.
fn svg_parity(w: u32, h: u32) -> ((usize, usize, usize), (f64, f64)) {
    let cx = Context::new_blocking().unwrap();
    let rows = walk(300_000);
    let mut auto = scatter(rows.clone(), Lod::Auto);
    let mut off = scatter(rows, Lod::Off);
    let (png_a, png_o) = (draw_at(&cx, &mut auto, w, h), draw_at(&cx, &mut off, w, h));
    let (a, o) = (svg_of(&cx, &mut auto, w, h), svg_of(&cx, &mut off, w, h));
    let counts = (
        png_a.instances as usize,
        a.matches("<circle ").count(),
        o.matches("<circle ").count(),
    );
    // The circles are the drawn ones: the same size.
    let first = &a[a.find("<circle ").expect("a circle")..];
    assert!(first[..first.find("/>").unwrap()].contains(r#"r="1.5""#));
    let (ra, ro) = (rasterise(&a, w, h), rasterise(&o, w, h));
    let svg = shares(&ra, &ro, &auto).1;
    let png = shares(&png_a.image, &png_o.image, &auto).1;
    eprintln!(
        "300,000 rows at {w} x {h}: {} representatives; ΔE00 > 10 from Off, of the plot \
         rect: SVG {svg:.3}%, PNG {png:.3}%",
        counts.0
    );
    (counts, (svg, png))
}

#[test]
fn svg_writes_the_representatives_of_a_small_chart() {
    let ((drawn, auto, off), (svg, png)) = svg_parity(W, H);
    assert_eq!(off, 300_000, "Off writes every row");
    assert_eq!(auto, drawn, "Auto writes what it draws");
    assert!(auto < off / 10, "{auto} circles of {off}");
    // A small chart draws coarsely (GUP-717's gates): the SVG is as close
    // to every row as the PNG is.
    assert!(svg <= png + 0.5, "SVG {svg:.3}% against PNG {png:.3}%");
}

#[test]
fn svg_looks_like_every_row_at_1080p() {
    let ((drawn, auto, off), (svg, _)) = svg_parity(1920, 1080);
    assert_eq!((auto, off), (drawn, 300_000));
    assert!(auto < off / 2, "{auto} circles of {off}");
    assert!(svg <= 1.0, "{svg:.3}% of the plot rect differs");
}

#[test]
fn a_gpu_only_lod_layer_still_rasterises() {
    let cx = Context::new_blocking().unwrap();
    let rows = walk(300_000);
    let mut plot = columnar(&rows, Lod::Auto);
    let drawn = draw(&cx, &mut plot).instances as usize;
    let svg = svg_of(&cx, &mut plot, W, H);
    assert_eq!(svg.matches("<circle ").count(), drawn, "{svg:.300}");
    assert!(!svg.contains("skipped"));
}

#[test]
fn a_brush_is_exact_and_the_description_is_unchanged_under_lod() {
    use gup_core::{Button, Input};
    let cx = Context::new_blocking().unwrap();
    let rows = walk(300_000);
    let run = |lod| {
        let mut plot = scatter(rows.clone(), lod);
        plot.drag(gup_core::Drag::Brush);
        draw(&cx, &mut plot);
        let r = plot.layout().unwrap().plot;
        let a = gup_core::geom::Point::new(r.x + 0.2 * r.width, r.y + 0.1 * r.height);
        let b = gup_core::geom::Point::new(r.x + 0.5 * r.width, r.y + 0.9 * r.height);
        let button = Button::Primary;
        let mut feed = |input| {
            let _ = plot.handle(input);
        };
        feed(Input::PointerDown { at: a, button });
        feed(Input::PointerMoved {
            at: gup_core::geom::Point::new(a.x + 20.0, a.y + 20.0),
        });
        feed(Input::PointerMoved { at: b });
        feed(Input::PointerUp { at: b, button });
        let brushed = plot.brush().expect("a brush").rows.clone();
        draw(&cx, &mut plot);
        let text = plot.describe().expect("resolved").alt_text();
        (brushed, text)
    };
    let (auto, off) = (run(Lod::Auto), run(Lod::Off));
    assert!(auto.0.len() > 50_000, "{} rows brushed", auto.0.len());
    assert!(auto.0 == off.0, "the brushed rows differ");
    assert_eq!(auto.1, off.1);
}

/// The index follows the view: every resolve (a zoom writes the scales)
/// prepares the tiers again and the index is rebuilt for them, so it never
/// holds representatives the frame did not draw.
#[test]
fn the_pick_index_follows_the_zoom() {
    let cx = Context::new_blocking().unwrap();
    let rows = 1_000_000;
    let (mut plot, x, y) = zoomable(rows, Lod::Auto);
    let full = draw(&cx, &mut plot).instances;
    assert_eq!(plot.pick_item_count(), Some(full as usize));
    let middle = walk(rows)[rows / 2];
    zoom_ten(&plot, &x, &y, (middle.0, middle.1));
    let zoomed = draw(&cx, &mut plot).instances;
    assert_ne!(zoomed, full, "the zoom changes what is drawn");
    // Every instance of a visible chunk is indexed (a mark whose centre is
    // outside the plot rect is still an item).
    assert_eq!(plot.pick_item_count(), Some(zoomed as usize));
    // And what the index holds picks rows of the zoomed view.
    let hit = grid(&plot, 5.0)
        .into_iter()
        .find_map(|at| plot.pick_row(at));
    assert!(hit.is_some_and(|r| r.row < rows));
}

// GUP-719: appends. The newest chunk of an appended layer draws every row
// under `TAIL` rows, builds tiers from there and again each time its rows
// double, and draws the rows past its tiers in full.

/// Rows an appended chunk holds before it builds tiers.
const TAIL: usize = 65_536;

/// A stream of scattered rows: x climbs 0.001 a row, y is a hash.
fn streamed(i: usize) -> Row {
    let h = (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40;
    (i as f64 * 0.001, (h % 10_000) as f64 / 100.0, 0.0)
}

/// An appended layer of `first` rows in `chunk`-row chunks over fixed
/// scales (x 0..2000, y 0..100), and its appender.
fn appended(first: usize, chunk: u32, lod: Lod) -> (Plot, gup_core::Appender<Row>) {
    appended_over(first, chunk, lod, (2000.0, 100.0))
}

fn appended_over(
    first: usize,
    chunk: u32,
    lod: Lod,
    (xmax, ymax): (f64, f64),
) -> (Plot, gup_core::Appender<Row>) {
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, xmax));
    let y = plot.y(Linear::new().domain(0.0, ymax));
    plot.add(Selection::<Row, Circle>::new(
        (0..first).map(streamed).collect::<Vec<_>>(),
    ))
    .attr(Circle::X, x.encode(|r: &Row| r.0))
    .attr(Circle::Y, y.encode(|r: &Row| r.1))
    .attr(Circle::RADIUS, Px(1.5))
    .lod(lod);
    let layer = plot.layer_mut::<Row, Circle>().unwrap();
    layer.max_chunk_rows(chunk);
    let feed = layer.appender();
    (plot, feed)
}

fn push(feed: &gup_core::Appender<Row>, from: usize, to: usize) {
    feed.extend((from..to).map(streamed)).unwrap();
}

fn resolve_time(cx: &Context, plot: &mut Plot) -> std::time::Duration {
    let t = std::time::Instant::now();
    plot.resolve(cx, W as f32, H as f32).unwrap();
    t.elapsed()
}

/// AC1: the tail draws every row under `TAIL` rows, then builds at its
/// doublings and at its last row (exactly 5 builds to a 1,048,576-row
/// chunk), and the amortised cost stays within 2.5 builds of the chunk. A
/// full first chunk keeps the layer over the threshold throughout.
#[test]
fn the_tail_builds_at_doubling_sizes() {
    let cx = Context::new_blocking().unwrap();
    const CHUNK: usize = 1 << 20;
    let (mut plot, tx) = appended(CHUNK, CHUNK as u32, Lod::Auto);
    draw(&cx, &mut plot);
    let first = builds(&mut plot);
    assert_eq!(first, 1, "the full chunk builds once");

    let mut total = CHUNK;
    let mut at = Vec::new();
    let (mut build_time, mut quiet) = (std::time::Duration::ZERO, Vec::new());
    while total < 2 * CHUNK {
        let n = 1_000.min(2 * CHUNK - total);
        push(&tx, total, total + n);
        total += n;
        let before = builds(&mut plot);
        let took = resolve_time(&cx, &mut plot);
        let tail = total - CHUNK;
        match builds(&mut plot) - before {
            0 => quiet.push(took),
            1 => {
                at.push(tail);
                build_time += took;
            }
            n => panic!("{n} builds in one frame at {tail} tail rows"),
        }
        if tail < TAIL {
            assert_eq!(builds(&mut plot), first, "no tiers under {TAIL} rows");
        }
    }
    eprintln!("tail builds at {at:?}");
    assert_eq!(at.len(), 5, "builds at {at:?}");
    assert_eq!(*at.last().unwrap(), CHUNK, "the last, when full");
    // The first just over `TAIL`, each next as its rows double.
    assert!((TAIL..TAIL + 1_000).contains(&at[0]), "{at:?}");
    for w in at[..4].windows(2) {
        assert!((2 * w[0]..2 * w[0] + 1_000).contains(&w[1]), "{at:?}");
    }

    // One build of the chunk: a two-chunk layer's first frame with tiers,
    // less the same frame without, over its two builds.
    let frame = |lod| {
        let (mut p, _tx) = appended(2 * CHUNK, CHUNK as u32, lod);
        resolve_time(&cx, &mut p)
    };
    let one = frame(Lod::Auto).saturating_sub(frame(Lod::Off)) / 2;
    quiet.sort();
    let typical = quiet[quiet.len() / 2];
    let spent = build_time.saturating_sub(typical * 5);
    eprintln!(
        "tail builds {spent:?} (5 frames {build_time:?} less 5 quiet frames of {typical:?}); one \
         build of a {CHUNK}-row chunk {one:?}"
    );
    assert!(
        spent <= one.mul_f64(2.5),
        "appends cost {spent:?}, one build {one:?}"
    );
}

/// AC1: a row appended after a build draws at once, in a second range.
#[test]
fn rows_past_the_tiers_draw_in_full() {
    let cx = Context::new_blocking().unwrap();
    const CHUNK: usize = 300_000;
    let (mut plot, tx) = appended(CHUNK, CHUNK as u32, Lod::Auto);
    let base = draw(&cx, &mut plot).instances;
    assert_eq!(builds(&mut plot), 1);
    push(&tx, CHUNK, CHUNK + TAIL + 4_000);
    let built = draw(&cx, &mut plot);
    assert_eq!(builds(&mut plot), 2, "the tail builds over {TAIL} rows");
    assert_eq!(built.chunks, 2);
    let tail_reps = built.instances - base;
    assert!(tail_reps < (TAIL as u64) / 2, "{tail_reps} tail instances");

    // A far-out row (the corner of the plot) is drawn by the next frame.
    let row = CHUNK + TAIL + 4_000;
    tx.extend([(1999.0, 99.5, 0.0)]).unwrap();
    let next = draw(&cx, &mut plot);
    assert_eq!(builds(&mut plot), 2, "no rebuild for one row");
    assert_eq!(next.instances, built.instances + 1, "one more instance");
    let at = plot
        .pick_anchor(gup_core::RowId { layer: 0, row })
        .expect("the new row is in the pick index");
    let around = |image: &image::RgbaImage| {
        let (px, py) = (at.x.round() as i32, at.y.round() as i32);
        let mut hit = 0;
        for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let p = image.get_pixel((px + dx) as u32, (py + dy) as u32).0;
            hit += usize::from(p != [255, 255, 255, 255] && p[3] > 0);
        }
        hit
    };
    assert!(around(&next.image) > 0, "no mark at {at:?}");
    assert_eq!(around(&built.image), 0, "something was there before");
}

/// AC2: appends that stretch the extents ten-fold leave full chunks'
/// tiers and draws alone.
#[test]
fn appends_that_grow_the_extent_leave_full_chunks_alone() {
    let cx = Context::new_blocking().unwrap();
    const CHUNK: usize = 250_000;
    // The first chunks span x 0..500, y 0..100, in the plot's bottom left
    // tenth of x and y.
    let (mut plot, tx) = appended_over(2 * CHUNK, CHUNK as u32, Lod::Auto, (5000.0, 1000.0));
    let before = draw(&cx, &mut plot);
    assert_eq!(builds(&mut plot), 2);
    // Ten times the extents, far from the old marks.
    tx.extend((0..10_000).map(|i| (2_500.0 + i as f64 * 0.25, 500.0 + i as f64 * 0.05, 0.0)))
        .unwrap();
    let after = draw(&cx, &mut plot);
    assert_eq!(builds(&mut plot), 2, "no tiers rebuilt");
    assert_eq!(after.instances, before.instances + 10_000);
    let r = plot.layout().unwrap().plot;
    let (x0, x1) = (r.x as u32, (r.x + r.width * 0.12) as u32);
    let (y0, y1) = ((r.y + r.height * 0.88) as u32, (r.y + r.height) as u32 - 1);
    let (mut differ, mut marks) = (0, 0);
    for y in y0..y1 {
        for x in x0..x1 {
            let (a, b) = (before.image.get_pixel(x, y), after.image.get_pixel(x, y));
            differ += usize::from(a != b);
            marks += usize::from(a.0 != [255, 255, 255, 255]);
        }
    }
    // The old chunks draw the same rows: at most the antialiased top edge
    // of the cloud moves (it does under `Lod::Off` too, so it is not the
    // tiers).
    assert!(marks > 500, "{marks} marked pixels in the old corner");
    assert!(
        differ <= 2 * (x1 - x0) as usize,
        "{differ} of {marks} pixels changed"
    );
}

/// AC3: a rebuild bumps the pick epoch: a GPU pick requested before is
/// dropped, one after is answered with the data row.
#[test]
fn a_rebuild_drops_a_pending_gpu_pick() {
    let cx = Context::new_blocking().unwrap();
    const CHUNK: usize = 300_000;
    let (mut plot, tx) = appended(CHUNK, CHUNK as u32, Lod::Auto);
    push(&tx, CHUNK, CHUNK + TAIL + 4_000);
    draw(&cx, &mut plot);
    assert_eq!(builds(&mut plot), 2);
    // Row 0 represents its cell at every tier.
    let at = plot
        .pick_anchor(gup_core::RowId { layer: 0, row: 0 })
        .expect("row 0 is drawn");
    // Doubling the tail rebuilds it.
    let dropped = plot.request_pick(&cx, at).unwrap();
    push(&tx, CHUNK + TAIL + 5_000, CHUNK + 2 * (TAIL + 5_000));
    draw(&cx, &mut plot);
    assert_eq!(builds(&mut plot), 3, "the tail rebuilt");
    let answered = plot.pick_row_gpu(&cx, at).unwrap();
    assert!(
        answered.is_some_and(|r| r.row < CHUNK),
        "a pick after the rebuild finds a row of the first chunk: {answered:?}"
    );
    let picks = plot.poll_picks(&cx);
    let row_of = |t| picks.iter().find(|p| p.ticket == t).expect("polled").row;
    assert_eq!(row_of(dropped), None, "a pick across a rebuild is dropped");
}
