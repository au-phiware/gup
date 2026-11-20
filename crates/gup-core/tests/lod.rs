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

/// Ranges of representatives a chunk draws in a frame at most (GUP-720).
const MAX_RANGES: usize = 64;

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
/// instances of its first data layer (`instances`: of its chunk draws, a
/// tier's whole prefix; `drawn`: of the spans it can see, the runs of the
/// prefix it draws, GUP-720).
struct Drawn {
    image: image::RgbaImage,
    draw_calls: usize,
    instances: u64,
    drawn: u64,
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
        drawn: gup_core::__private::visible_instances(&batch),
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
    zoomable_rows(walk(rows), lod)
}

/// `walk(n)` shuffled (Fisher-Yates, a fixed seed), as `lod_spike
/// --shuffle` draws it: every chunk spans the whole extent.
fn shuffled_walk(n: usize) -> Vec<Row> {
    let mut rows = walk(n);
    let mut s = 0x9e37_79b9_7f4a_7c15u64;
    for i in (1..n).rev() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        rows.swap(i, (s % (i as u64 + 1)) as usize);
    }
    rows
}

/// A scatter of `rows` over explicit scales, so a test can zoom.
fn zoomable_rows(rows: Vec<Row>, lod: Lod) -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>) {
    let (mut plot, x, y) = plot();
    plot.add(Selection::<Row, Circle>::new(rows))
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
    fidelity_of(w, h, walk(rows), "")
}

/// [`fidelity`] of the walk's rows in the order `data` holds them
/// (`tag` names the saved images).
fn fidelity_of(w: u32, h: u32, data: Vec<Row>, tag: &str) -> Fidelity {
    let rows = data.len();
    let cx = Context::new_blocking().unwrap();
    let (mut auto, ax, ay) = zoomable_rows(data.clone(), Lod::Auto);
    let (mut off, ox, oy) = zoomable_rows(data, Lod::Off);
    let (a, o) = (draw_at(&cx, &mut auto, w, h), draw_at(&cx, &mut off, w, h));
    let pixels = plot_rect_pixels(&auto);
    save(&a.image, &format!("lod-auto{tag}-{w}x{h}.png"));
    save(&o.image, &format!("lod-off{tag}-{w}x{h}.png"));
    // The guides draw the same in both, and the layer has one instanced
    // draw per chunk in both: in the full view every run of a tier's
    // prefix meets the plot rect, so its runs merge into one range
    // (GUP-720).
    assert_eq!(a.draw_calls, o.draw_calls);
    assert_eq!(a.drawn, a.instances);
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
    save(&za.image, &format!("lod-auto{tag}-{w}x{h}-zoom.png"));
    save(&zo.image, &format!("lod-off{tag}-{w}x{h}-zoom.png"));
    // A zoomed chunk drawing a tier draws the runs of its prefix that meet
    // the plot rect: at most 64 ranges a chunk (GUP-720).
    let ranges = MAX_RANGES * za.chunks;
    assert!(
        za.draw_calls <= zo.draw_calls - zo.chunks + ranges,
        "{} draw calls against {} for Off",
        za.draw_calls,
        zo.draw_calls
    );
    let (zoom_image, zoom) = shares(&za.image, &zo.image, &auto);
    eprintln!(
        "{rows} rows at {w} x {h}, 10x zoom: {} representatives drawn of a prefix of {} \
         ({} rows), {} draw calls (Off: {}); ΔE00 > 10: {zoom_image:.3}% of the image, \
         {zoom:.3}% of the plot rect",
        za.drawn, za.instances, zo.instances, za.draw_calls, zo.draw_calls
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

/// GUP-720: a shuffled layer (every chunk spanning the extent) is as
/// faithful as the sorted one at the same size; GUP-717's gates.
#[test]
fn a_shuffled_million_row_walk_matches_the_exact_draw_at_400_by_300() {
    let f = fidelity_of(W, H, shuffled_walk(1_000_000), "-shuffled");
    assert!(f.full <= 2.0, "full view {:.3}% of the plot rect", f.full);
    assert!(f.zoom <= 3.0, "10x zoom {:.3}% of the plot rect", f.zoom);
}

/// GUP-720: a zoomed shuffled layer draws the runs of its chunks' prefixes
/// that meet the plot rect, a small share of them, at most 64 ranges a
/// chunk; and the image is the image of every run (no culling), byte for
/// byte, so no run that reaches the plot rect is skipped.
#[test]
fn a_zoomed_shuffled_layer_draws_the_runs_it_can_see() {
    let cx = Context::new_blocking().unwrap();
    // Two full chunks, each spanning the extent (a part chunk stores
    // coarser tiers only, so a zoom draws it whole).
    let rows = 2 * 1_048_576;
    let data = shuffled_walk(rows);
    let middle = walk(rows)[rows / 2];
    let zoomed = |cull: bool| {
        let (mut plot, x, y) = zoomable_rows(data.clone(), Lod::Auto);
        plot.chunk_culling(cull);
        draw(&cx, &mut plot);
        zoom_ten(&plot, &x, &y, (middle.0, middle.1));
        draw(&cx, &mut plot)
    };
    let (culled, every) = (zoomed(true), zoomed(false));
    save(&culled.image, "lod-auto-shuffled-runs-zoom.png");
    assert_eq!(culled.chunks, 2);
    assert_eq!(culled.instances, every.instances, "the same prefixes");
    assert_eq!(every.drawn, every.instances);
    eprintln!(
        "{rows} shuffled rows, 10x zoom: {} of {} representatives in {} draw calls (every \
         run: {})",
        culled.drawn, culled.instances, culled.draw_calls, every.draw_calls
    );
    // Measured 157,311 of 552,708 (28.5%): at 400 x 300 a chunk's prefix
    // is about 135 runs, whose boxes are a few percent of the extent each.
    assert!(
        culled.drawn * 3 < culled.instances,
        "{} of {} representatives drawn",
        culled.drawn,
        culled.instances
    );
    assert!(culled.draw_calls <= every.draw_calls - every.chunks + MAX_RANGES * every.chunks);
    assert!(culled.draw_calls > every.draw_calls, "runs were culled");
    assert!(
        culled.image.as_raw() == every.image.as_raw(),
        "culling runs changed the image"
    );
}

#[test]
fn a_million_row_walk_matches_the_exact_draw_at_1080p() {
    // Measured 0.571% / 0.000%. The zoom asks one tier past the chunk's
    // finest stored tier; few rows are in view (an estimated 35,000, under
    // `Lod::AUTO_MIN_ROWS`), so it draws every row, exact (GUP-720's cap
    // applies only above that).
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

/// A host's waker that counts its wakes: a worker's tier build wakes it
/// when done (GUP-730).
struct Wakes(std::sync::atomic::AtomicUsize);

impl std::task::Wake for Wakes {
    fn wake(self: std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Wakes {
    fn waker() -> (std::sync::Arc<Self>, std::task::Waker) {
        let w = std::sync::Arc::new(Self(0.into()));
        (w.clone(), std::task::Waker::from(w))
    }

    /// Wait (up to a minute) until woken `n` times.
    fn until(&self, n: usize) {
        let t = std::time::Instant::now();
        while self.0.load(std::sync::atomic::Ordering::SeqCst) < n {
            assert!(
                t.elapsed() < std::time::Duration::from_secs(60),
                "woken {} of {n} times",
                self.0.load(std::sync::atomic::Ordering::SeqCst)
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

/// Every worker build sleeps `ms` first while this lives (the build
/// seam), so a test can see a frame that does not wait for one.
struct SlowBuilds;

impl SlowBuilds {
    fn new(ms: u64) -> Self {
        gup_core::__private::set_lod_build_delay(ms);
        Self
    }
}

impl Drop for SlowBuilds {
    fn drop(&mut self) {
        gup_core::__private::set_lod_build_delay(0);
    }
}

/// GUP-730: a headless frame builds every visible chunk's tiers before it
/// returns (on worker threads); a hosted frame never waits for a build. Its
/// chunks draw in full while their builds run (and no redraw is asked for
/// meanwhile), each finished build wakes the host, the next frame installs
/// it, and the picture converges to the headless one.
#[test]
fn hosted_frames_never_wait_for_a_build_and_converge_to_the_headless_frame() {
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
    let complete = draw(&cx, &mut headless);
    assert_eq!(complete.chunks, 3);
    assert_eq!(builds(&mut headless), 3);
    assert!(!Chart::needs_frame(&headless));

    // Hosted, each build taking at least 1.5 s: the first frame returns
    // without one.
    const SLOW: u64 = 1_500;
    let slow = SlowBuilds::new(SLOW);
    let (wakes, waker) = Wakes::waker();
    let mut hosted = make();
    Chart::set_waker(&mut hosted, waker);
    let t = std::time::Instant::now();
    let first = draw(&cx, &mut hosted);
    let took = t.elapsed();
    assert!(
        took < std::time::Duration::from_millis(SLOW),
        "the first frame took {took:?}, as long as a build"
    );
    assert_eq!(builds(&mut hosted), 0, "no build waited for");
    assert_eq!(first.instances, 300_000, "every row draws meanwhile");
    assert!(
        !Chart::needs_frame(&hosted),
        "no redraw while the builds run: they wake the host"
    );
    // A frame while they run draws the same, again without waiting.
    let t = std::time::Instant::now();
    let again = draw(&cx, &mut hosted);
    assert!(t.elapsed() < std::time::Duration::from_millis(SLOW));
    assert_eq!((builds(&mut hosted), again.instances), (0, 300_000));
    drop(slow);

    // Each build wakes the host when done; the frame then installs it.
    wakes.until(3);
    assert!(Chart::needs_frame(&hosted), "finished builds await a frame");
    let done = draw(&cx, &mut hosted);
    assert_eq!(builds(&mut hosted), 3);
    assert!(!Chart::needs_frame(&hosted));
    assert_eq!(done.instances, complete.instances);
    assert_eq!(done.draw_calls, complete.draw_calls);
    assert!(
        done.image == complete.image,
        "the hosted picture converges to the headless one"
    );
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
    let (wakes, waker) = Wakes::waker();
    Chart::set_waker(&mut plot, waker);
    // Zoomed in so far that no chunk takes a tier, and fewer rows than the
    // threshold visible: yet the three full chunks build, since release
    // drops their CPU copies. The builds hold the copies until they are
    // done (GUP-730), so the layer releases at once.
    x.write().set_domain(10.0, 10.001).unwrap();
    let slow = SlowBuilds::new(300);
    draw(&cx, &mut plot);
    assert!(plot.layer_mut::<Row, Circle>().unwrap().rows_released());
    assert_eq!(builds(&mut plot), 0, "built off the frame");
    drop(slow);
    wakes.until(3);
    draw(&cx, &mut plot);
    assert_eq!(builds(&mut plot), 3);
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

/// A size-ordered layer takes a level of detail under `Lod::AutoSized`
/// only (GUP-721): `Auto` still draws every row.
fn sized(lod: Lod) -> Lod {
    if lod == Lod::Auto {
        Lod::AutoSized
    } else {
        lod
    }
}

/// `n` bubbles (GUP-721): the walk's positions (a thin curve, so marks
/// overlap heavily) in 100 clumps of 16 sizes (the third value, 0..16), a
/// clump's size drawn at random. A cell of the thinned draw holds one size
/// but at a clump's edge, as a bubble chart's sizes follow its data; a size
/// drawn at random for every row would make every cell hold all 16, and
/// the full draw shows its smallest marks on top where a thinned one can
/// only show its largest (see the story's retrospective).
fn bubbles(n: usize) -> Vec<Row> {
    let mut s = 0x9e37_79b9_7f4a_7c15u64;
    let mut level = 0.0;
    walk(n)
        .into_iter()
        .enumerate()
        .map(|(i, (x, y, _))| {
            if i % (n / 100) == 0 {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                level = f64::from(((s >> 11) % 16) as u32);
            }
            (x, y, level)
        })
        .collect()
}

/// A bubble layer: radius 1..9 px and a colour by the size level, so a
/// small bubble hidden under a large one, or a large one drawn over a small
/// one, shows.
fn bubble_plot(rows: Vec<Row>, lod: Lod) -> Plot {
    let (mut plot, x, y) = plot();
    let colour = Sequential::viridis();
    plot.add(Selection::<Row, Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(
            Circle::RADIUS,
            Linear::new().range(Px(1.0), Px(9.0)).encode(|r: &Row| r.2),
        )
        .attr(Circle::FILL, colour.encode(|r: &Row| r.2))
        .lod(sized(lod));
    plot
}

#[test]
fn a_layer_drawn_by_size_ignores_auto() {
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
fn a_layer_drawn_by_size_takes_lod_under_auto_sized() {
    let cx = Context::new_blocking().unwrap();
    let rows = bubbles(300_000);
    let (mut auto, mut off) = (
        bubble_plot(rows.clone(), Lod::Auto),
        bubble_plot(rows.clone(), Lod::Off),
    );
    let (a, o) = (draw(&cx, &mut auto), draw(&cx, &mut off));
    save(&a.image, "lod-bubbles-auto.png");
    save(&o.image, "lod-bubbles-off.png");
    assert_eq!(o.instances, rows.len() as u64);
    assert!(
        (a.instances as f64) < rows.len() as f64 / 10.0,
        "{} instances of {} rows",
        a.instances,
        rows.len()
    );
    let (image, rect) = shares(&a.image, &o.image, &auto);
    eprintln!(
        "300000 bubbles: {} representatives; ΔE00 > 10: {image:.3}% of the image, \
         {rect:.3}% of the plot rect",
        a.instances
    );
    // Measured 2.303%: the outlines of bubbles of 16 viridis colours, whose
    // thinned edges are up to a pixel narrower (the same marks in one colour
    // differ 0.884%, `a_layer_drawn_by_size_in_one_colour_...`); the gate
    // is 1.3x as GUP-717's are.
    assert!(rect <= 3.0, "{rect:.3}% of the plot rect differs");
}

/// The bubbles of [`a_layer_drawn_by_size_takes_lod`] in one colour: the
/// thinned picture is within the 2.0% of the plot rect of the scatters.
#[test]
fn a_layer_drawn_by_size_in_one_colour_matches_the_exact_draw() {
    let cx = Context::new_blocking().unwrap();
    let rows = bubbles(300_000);
    let one = |lod| {
        let (mut plot, x, y) = plot();
        plot.add(Selection::<Row, Circle>::new(rows.clone()))
            .attr(Circle::X, x.encode(|r: &Row| r.0))
            .attr(Circle::Y, y.encode(|r: &Row| r.1))
            .attr(
                Circle::RADIUS,
                Linear::new().range(Px(1.0), Px(9.0)).encode(|r: &Row| r.2),
            )
            .lod(sized(lod));
        plot
    };
    let (mut auto, mut off) = (one(Lod::Auto), one(Lod::Off));
    let (a, o) = (draw(&cx, &mut auto), draw(&cx, &mut off));
    assert!((a.instances as f64) < rows.len() as f64 / 10.0);
    let (_, rect) = shares(&a.image, &o.image, &auto);
    // Measured 0.884%.
    assert!(rect <= 2.0, "{rect:.3}% of the plot rect differs");
}

/// A GPU pick over a bubble layer under LOD returns the data row of the
/// representative drawn at the pixel, through the draw order's row map.
#[test]
fn a_pick_over_a_size_ordered_layer_returns_the_representatives_data_row() {
    let cx = Context::new_blocking().unwrap();
    let rows = bubbles(300_000);
    let mut plot = bubble_plot(rows.clone(), Lod::Auto);
    let d = draw(&cx, &mut plot);
    assert!(d.instances < 30_000, "{} drawn", d.instances);
    let (mut hits, mut near, mut disagree) = (0, 0, 0);
    for at in grid(&plot, 9.0) {
        let (cpu, gpu) = (plot.pick_row(at), plot.pick_row_gpu(&cx, at).unwrap());
        disagree += usize::from(cpu != gpu);
        let Some(id) = gpu else { continue };
        hits += 1;
        let (x, y, size) = rows[id.row];
        let p = plot.pixel(x, y).unwrap();
        let radius = 1.0 + 8.0 * size as f32 / 15.0;
        near += usize::from(
            (p.x - at.x).hypot(p.y - at.y) <= radius + gup_core::PICK_TOLERANCE.0 + 1.0,
        );
    }
    assert!(hits > 100, "{hits} grid points hit a bubble");
    assert_eq!(near, hits, "a picked row is not under the pixel");
    assert!(disagree * 20 <= hits, "{disagree} of {hits} picks differ");
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
    let full = draw(&cx, &mut plot).drawn;
    assert_eq!(plot.pick_item_count(), Some(full as usize));
    let middle = walk(rows)[rows / 2];
    zoom_ten(&plot, &x, &y, (middle.0, middle.1));
    let zoomed = draw(&cx, &mut plot).drawn;
    assert_ne!(zoomed, full, "the zoom changes what is drawn");
    // Every instance drawn is indexed (a mark whose centre is outside the
    // plot rect is still an item), and no representative of a run the
    // plot rect cannot see (GUP-720).
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
    extent: (f64, f64),
) -> (Plot, gup_core::Appender<Row>) {
    let rows = (0..first).map(streamed).collect();
    let mut plot = fixed(rows, lod, extent);
    let layer = plot.layer_mut::<Row, Circle>().unwrap();
    layer.max_chunk_rows(chunk);
    let feed = layer.appender();
    (plot, feed)
}

/// A scatter of `rows` over x 0..`xmax` and y 0..`ymax`.
fn fixed(rows: Vec<Row>, lod: Lod, (xmax, ymax): (f64, f64)) -> Plot {
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, xmax));
    let y = plot.y(Linear::new().domain(0.0, ymax));
    plot.add(Selection::<Row, Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(Circle::RADIUS, Px(1.5))
        .lod(lod);
    plot
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

    // One build of the chunk: a one-chunk layer's first frame with tiers,
    // less the same frame without (one chunk: builds run in parallel,
    // GUP-730).
    let frame = |lod| {
        let (mut p, _tx) = appended(CHUNK, CHUNK as u32, lod);
        resolve_time(&cx, &mut p)
    };
    let one = frame(Lod::Auto).saturating_sub(frame(Lod::Off));
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

/// AC2: appends that stretch the extents nine-fold leave full chunks'
/// tiers and draws alone. The new rows stay clear of the scale domains'
/// ends: marks that touch an end make the plot inset the scale range
/// (the edge solve), which moves every old mark by a sub-pixel amount that
/// each device rasterises differently (GUP-741).
#[test]
fn appends_that_grow_the_extent_leave_full_chunks_alone() {
    let cx = Context::new_blocking().unwrap();
    const CHUNK: usize = 250_000;
    // The first chunks span x 0..500, y 0..100, in the plot's bottom left
    // tenth of x and y.
    let (mut plot, tx) = appended_over(2 * CHUNK, CHUNK as u32, Lod::Auto, (5000.0, 1000.0));
    let before = draw(&cx, &mut plot);
    assert_eq!(builds(&mut plot), 2);
    // Nine times the extents (x to 4,500, y to 900), far from the old marks.
    tx.extend((0..10_000).map(|i| (2_000.0 + i as f64 * 0.25, 400.0 + i as f64 * 0.05, 0.0)))
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
    // The old chunks draw the same rows, so the old corner is byte for byte
    // what it was, on every device.
    assert!(marks > 500, "{marks} marked pixels in the old corner");
    assert_eq!(differ, 0, "{differ} of {marks} pixels changed");
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

// GUP-719: windows. `RowWindow::LastRows` drops whole chunks with their
// tiers and representatives; the row ids of what is held stay the data
// rows'.

/// A layer fed through an appender under `RowWindow::LastRows(window)`,
/// over x 0..6000 and y 0..100 (a row is 0.001 of x).
fn windowed(window: usize, lod: Lod) -> (Plot, gup_core::Appender<Row>) {
    let (mut plot, tx) = appended_over(1, 1 << 20, lod, (6000.0, 100.0));
    plot.layer_mut::<Row, Circle>()
        .unwrap()
        .window(RowWindow::LastRows(window));
    (plot, tx)
}

fn held(plot: &mut Plot) -> usize {
    plot.layer_mut::<Row, Circle>().unwrap().len()
}

fn lod_bytes(plot: &mut Plot) -> u64 {
    gup_core::__private::lod_gpu_bytes(plot.layer_mut::<Row, Circle>().unwrap())
}

/// AC3: a 2,000,000-row window fed 100,000 rows at a time to 6,000,000:
/// eviction writes no column bytes, the representatives held are bounded by
/// the chunks held, the picture equals a fresh layer of the held rows
/// (exactly under `Off`, by ΔE00 under `Auto`), and the oldest held chunk's
/// representatives keep their data rows' ids.
#[test]
fn a_rolling_window_keeps_tiers_for_its_chunks_only() {
    let cx = Context::new_blocking().unwrap();
    const WINDOW: usize = 2_000_000;
    const TOTAL: usize = 6_000_000;
    const BATCH: usize = 100_000;

    // Exact: the stream under `Off` against a fresh plot of the held rows,
    // and the columns written are the rows appended, none for eviction.
    let (mut off, tx) = windowed(WINDOW, Lod::Off);
    let start = cx.upload_stats();
    draw(&cx, &mut off);
    let mut total = 1;
    while total < TOTAL {
        let n = BATCH.min(TOTAL - total);
        push(&tx, total, total + n);
        total += n;
        off.resolve(&cx, W as f32, H as f32).unwrap();
    }
    let kept = held(&mut off);
    assert!(
        (WINDOW..=WINDOW + WINDOW / 4).contains(&kept),
        "{kept} held"
    );
    let written = cx.upload_stats() - start;
    // x and y are 4 bytes each, every row uploaded once.
    assert_eq!(
        written.columns.bytes,
        8 * TOTAL as u64,
        "eviction wrote column bytes: {written:?}"
    );
    let first = total - kept;
    let exact = draw(&cx, &mut off);
    let rows = (first..total).map(streamed).collect();
    let mut reference = fixed(rows, Lod::Off, (6000.0, 100.0));
    let expected = draw(&cx, &mut reference);
    let tol = gup_visual_regression::DiffTolerance::default();
    let vr = |i: &image::RgbaImage| {
        gup_visual_regression::RgbaImage::new(i.width(), i.height(), i.as_raw().clone()).unwrap()
    };
    let stats =
        gup_visual_regression::diff::perceptual_diff(&vr(&expected.image), &vr(&exact.image), &tol)
            .unwrap();
    eprintln!("Off, windowed vs fresh: {stats}");
    assert!(
        stats.within(&tol),
        "windowed Off differs from fresh: {stats}"
    );
    drop(off);

    // The same stream under `Auto`.
    let (mut auto, tx) = windowed(WINDOW, Lod::Auto);
    draw(&cx, &mut auto);
    let mut total = 1;
    let (mut most, mut chunks_before) = (0, 0);
    while total < TOTAL {
        let n = BATCH.min(TOTAL - total);
        push(&tx, total, total + n);
        total += n;
        let d = draw_at(&cx, &mut auto, W, H);
        let (bytes, rows) = (lod_bytes(&mut auto), held(&mut auto));
        // At most 6 bytes a held row: 12 per representative (x, y and the
        // row), at most half a chunk's rows.
        assert!(bytes <= 6 * rows as u64, "{bytes} B of tiers, {rows} rows");
        most = most.max(bytes);
        chunks_before = chunks_before.max(d.chunks);
    }
    assert!(most > 0, "no tiers were built");
    let kept = held(&mut auto);
    let first = total - kept;
    let d = draw(&cx, &mut auto);
    eprintln!(
        "Auto: {kept} rows held in {} chunks (at most {chunks_before}), {} instances, tiers \
         {} B (at most {most} B)",
        d.chunks,
        d.instances,
        lod_bytes(&mut auto)
    );
    assert!(d.instances < kept as u64 / 10, "{} instances", d.instances);
    let (image, rect) = shares(&d.image, &expected.image, &auto);
    eprintln!("Auto windowed vs fresh Off: {image:.3}% of the image, {rect:.3}% of the plot rect");
    assert!(rect < 1.0, "{rect:.3}% of the plot rect over ΔE00 10");
    save(&d.image, "lod-window-auto.png");
    save(&expected.image, "lod-window-fresh.png");

    // The oldest held chunk's first row represents its cell at every tier:
    // its id is the data row, the evicted rows counted.
    let id = gup_core::RowId {
        layer: 0,
        row: first,
    };
    let at = auto.pick_anchor(id).expect("the oldest held row is drawn");
    let hit = auto.pick_row(at).expect("a mark at the anchor");
    assert!(
        (first..first + 500_000).contains(&hit.row),
        "row {} picked at the oldest chunk (first row {first})",
        hit.row
    );
    let hit_gpu = auto.pick_row_gpu(&cx, at).unwrap().expect("GPU pick");
    assert!((first..first + 500_000).contains(&hit_gpu.row));
}

/// One frame of [`window_frames`]: its CPU time, the tier builds it
/// installed, and whether it evicted a chunk.
struct WindowFrame {
    cpu: std::time::Duration,
    built: u64,
    evicted: bool,
}

/// Thirty frames of a full 2,000,000-row window fed 100,000 rows a frame,
/// under a host (a waker set): the CPU's frame is resolve (which prepares
/// the layer), the renderer's prepare and the recording and submission of
/// the draw, not waiting for the device. Each is printed.
fn window_frames(cx: &Context, lod: Lod) -> Vec<WindowFrame> {
    let (mut plot, tx) = windowed(2_000_000, lod);
    let (_wakes, waker) = Wakes::waker();
    Chart::set_waker(&mut plot, waker);
    draw(cx, &mut plot);
    let mut total = 1;
    while total < 3_000_000 {
        push(&tx, total, total + 100_000);
        total += 100_000;
        plot.resolve(cx, W as f32, H as f32).unwrap();
    }
    let mut target = ImageTarget::new(cx, W, H).unwrap();
    let mut frames = Vec::new();
    eprintln!("{lod:?}:");
    for _ in 0..30 {
        let rows = held(&mut plot);
        push(&tx, total, total + 100_000);
        total += 100_000;
        let before = builds(&mut plot);
        let t = std::time::Instant::now();
        let scene: Scene = plot.resolve(cx, W as f32, H as f32).unwrap().scene.clone();
        let resolved = t.elapsed();
        let desc = gup_core::RenderTarget::desc(&target);
        Renderer::new().prepare(cx, &scene, &desc).unwrap();
        let cpu = t.elapsed();
        target.render_blocking(cx, &scene).unwrap();
        let built = builds(&mut plot) - before;
        let evicted = held(&mut plot) < rows + 100_000;
        eprintln!(
            "  resolve {resolved:?}, + prepare {cpu:?}, + draw and wait {:?}{}{}",
            t.elapsed(),
            if built > 0 { ", installs a build" } else { "" },
            if evicted { ", evicts a chunk" } else { "" },
        );
        frames.push(WindowFrame {
            cpu,
            built,
            evicted,
        });
    }
    frames
}

fn p95(mut t: Vec<std::time::Duration>) -> std::time::Duration {
    t.sort();
    t[(t.len() * 95).div_ceil(100) - 1]
}

/// AC4 (GUP-719), every frame (GUP-730, GUP-747): the tail's tier builds
/// (about 0.1 s per 1,000,000 rows in release) run on worker threads, and
/// the frame that evicts a chunk moves no held row (the rows are a ring)
/// and grows no chunk (a windowed store's later chunks start full), so the
/// p95 of every frame, under `Off` and `Auto`, those that start or install
/// a build and those that evict included, is under 8 ms (release builds
/// only).
#[test]
fn a_rolling_window_frame_is_cheap() {
    let cx = Context::new_blocking().unwrap();
    let off = window_frames(&cx, Lod::Off);
    let auto = window_frames(&cx, Lod::Auto);
    let installs: u64 = auto.iter().map(|f| f.built).sum();
    let every = |frames: &[WindowFrame]| p95(frames.iter().map(|f| f.cpu).collect());
    let evicting = |frames: &[WindowFrame]| {
        let t: Vec<_> = frames.iter().filter(|f| f.evicted).map(|f| f.cpu).collect();
        (t.len(), t.iter().max().copied())
    };
    let (auto_p95, off_p95) = (every(&auto), every(&off));
    eprintln!(
        "30 frames of 100,000 rows: {installs} builds installed; p95 {auto_p95:?} ({off_p95:?} \
         under Off); evicting frames (count, max) {:?} ({:?} under Off)",
        evicting(&auto),
        evicting(&off),
    );
    assert!(installs > 0, "the tail's builds were installed");
    assert!(
        evicting(&auto).0 > 0 && evicting(&off).0 > 0,
        "frames evict"
    );
    if !cfg!(debug_assertions) {
        assert!(auto_p95.as_secs_f64() < 0.008, "Auto: p95 {auto_p95:?}");
        assert!(off_p95.as_secs_f64() < 0.008, "Off: p95 {off_p95:?}");
    }
}

/// A bubble layer over fixed scales (so appends move no mark), with an
/// appender and chunks of `chunk` rows.
fn bubbles_appended(first: &[Row], chunk: u32, lod: Lod) -> (Plot, gup_core::Appender<Row>) {
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(-50.0, 1050.0));
    let y = plot.y(Linear::new().domain(-250.0, 50.0));
    let colour = ScaleRef::new(Sequential::viridis().domain(0.0, 15.0));
    plot.add(Selection::<Row, Circle>::new(first.to_vec()))
        .attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(
            Circle::RADIUS,
            Linear::new()
                .domain(0.0, 15.0)
                .range(Px(1.0), Px(9.0))
                .encode(|r: &Row| r.2),
        )
        .attr(Circle::FILL, colour.encode(|r: &Row| r.2))
        .lod(sized(lod));
    let layer = plot.layer_mut::<Row, Circle>().unwrap();
    layer.max_chunk_rows(chunk);
    let feed = layer.appender();
    (plot, feed)
}

/// GUP-721 with GUP-719: appends to a size-ordered layer. The newest chunk
/// builds tiers over `TAIL` rows and its rows past them draw in full, each
/// through its own entry among the layer's size-ordered spans; the picture
/// stays that of every row.
#[test]
fn appends_to_a_size_ordered_layer_draw_the_tiers_and_the_rows_past_them() {
    let cx = Context::new_blocking().unwrap();
    const CHUNK: usize = 300_000;
    let rows = bubbles(CHUNK + 2 * TAIL);
    let (mut auto, tx) = bubbles_appended(&rows[..CHUNK], CHUNK as u32, Lod::Auto);
    let (mut off, ox) = bubbles_appended(&rows[..CHUNK], CHUNK as u32, Lod::Off);
    let first = draw(&cx, &mut auto);
    assert_eq!(builds(&mut auto), 1);
    let mid = CHUNK + TAIL + 1_000;
    for feed in [&tx, &ox] {
        feed.extend(rows[CHUNK..mid].iter().copied()).unwrap();
    }
    let a = draw(&cx, &mut auto);
    let o = draw(&cx, &mut off);
    assert_eq!(builds(&mut auto), 2, "the tail builds over {TAIL} rows");
    assert_eq!(o.instances, mid as u64);
    assert!(
        a.instances < first.instances * 2,
        "{} instances",
        a.instances
    );
    // Rows past the tail's tiers draw in full.
    for feed in [&tx, &ox] {
        feed.extend(rows[mid..mid + 2_000].iter().copied()).unwrap();
    }
    let b = draw(&cx, &mut auto);
    let ob = draw(&cx, &mut off);
    assert_eq!(builds(&mut auto), 2, "no rebuild for 2,000 rows");
    assert_eq!(
        b.instances,
        a.instances + 2_000,
        "the new rows draw in full"
    );
    for (name, x, y) in [("tiers", &a, &o), ("past the tiers", &b, &ob)] {
        let (_, rect) = shares(&x.image, &y.image, &auto);
        eprintln!("appended bubbles, {name}: {rect:.3}% of the plot rect");
        // Measured 3.258% and 3.363%; the same scene built at once (a
        // smaller picture of the same rows, so more rows a pixel) differs
        // 2.927%.
        assert!(rect <= 4.5, "{name}: {rect:.3}% of the plot rect differs");
    }
}

/// SVG of a size-ordered layer under LOD writes the circles it draws.
#[test]
fn svg_of_a_size_ordered_layer_writes_the_representatives() {
    let cx = Context::new_blocking().unwrap();
    let rows = bubbles(300_000);
    let mut plot = bubble_plot(rows, Lod::Auto);
    let drawn = draw(&cx, &mut plot).instances as usize;
    let svg = svg_of(&cx, &mut plot, W, H);
    assert_eq!(svg.matches("<circle ").count(), drawn);
}

/// Row `i` of a stream of bubbles: x climbs 0.001 a row, y is a wavy curve
/// with a little noise, the size one of 16 levels drawn at random for each
/// stretch of 50,000 rows.
fn streamed_bubble(i: usize) -> Row {
    let h = |k: u64| k.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40;
    let x = i as f64 * 0.001;
    let noise = (h(i as u64) % 1000) as f64 / 250.0;
    let level = h((i / 50_000) as u64 + 7) % 16;
    (x, 50.0 + 30.0 * (x / 40.0).sin() + noise, level as f64)
}

/// A bubble layer of `rows` over fixed scales (x 0..2100, y 0..100).
fn fixed_bubbles(rows: Vec<Row>, lod: Lod) -> Plot {
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, 2100.0));
    let y = plot.y(Linear::new().domain(0.0, 100.0));
    let colour = ScaleRef::new(Sequential::viridis().domain(0.0, 15.0));
    plot.add(Selection::<Row, Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(
            Circle::RADIUS,
            Linear::new()
                .domain(0.0, 15.0)
                .range(Px(1.0), Px(7.0))
                .encode(|r: &Row| r.2),
        )
        .attr(Circle::FILL, colour.encode(|r: &Row| r.2))
        .lod(sized(lod));
    plot
}

/// GUP-721, GUP-492: a rolling window over a size-ordered layer takes a
/// level of detail: each chunk is sorted on its own (bucketed keys), and the
/// ranks of its rows, read from the draw order after evictions renumber it,
/// pick the representatives. The picture is that of a fresh plot of the held
/// rows, and a pick names the data row, evicted rows counted.
#[test]
fn a_rolling_window_over_a_size_ordered_layer_takes_lod() {
    let cx = Context::new_blocking().unwrap();
    const WINDOW: usize = 1_000_000;
    const TOTAL: usize = 2_000_000;
    const BATCH: usize = 100_000;
    let mut auto = fixed_bubbles((0..1).map(streamed_bubble).collect(), Lod::Auto);
    let layer = auto.layer_mut::<Row, Circle>().unwrap();
    layer.max_chunk_rows(1 << 20);
    layer.window(RowWindow::LastRows(WINDOW));
    let tx = layer.appender();
    draw(&cx, &mut auto);
    let mut total = 1;
    while total < TOTAL {
        let n = BATCH.min(TOTAL - total);
        tx.extend((total..total + n).map(streamed_bubble)).unwrap();
        total += n;
        draw(&cx, &mut auto);
    }
    let kept = auto.layer_mut::<Row, Circle>().unwrap().len();
    let first = total - kept;
    assert!(kept >= WINDOW, "{kept} held");
    let a = draw(&cx, &mut auto);
    assert!(
        a.instances < kept as u64 / 10,
        "{} instances of {kept} rows",
        a.instances
    );
    let mut fresh = fixed_bubbles((first..total).map(streamed_bubble).collect(), Lod::Off);
    let o = draw(&cx, &mut fresh);
    assert_eq!(o.instances, kept as u64);
    let (image, rect) = shares(&a.image, &o.image, &auto);
    eprintln!(
        "windowed bubbles: {kept} held, {} instances; ΔE00 > 10: {image:.3}% of the image, \
         {rect:.3}% of the plot rect",
        a.instances
    );
    save(&a.image, "lod-window-bubbles-auto.png");
    save(&o.image, "lod-window-bubbles-fresh.png");
    // Measured 2.437%; the gate is 1.3x.
    assert!(rect <= 3.2, "{rect:.3}% of the plot rect differs");

    // A pick names a held data row drawn where its data says.
    let (mut hits, mut near) = (0, 0);
    for at in grid(&auto, 9.0) {
        let Some(id) = auto.pick_row_gpu(&cx, at).unwrap() else {
            continue;
        };
        hits += 1;
        let (x, y, size) = streamed_bubble(id.row);
        let p = auto.pixel(x, y).unwrap();
        let radius = 1.0 + 6.0 * size as f32 / 15.0;
        near += usize::from(
            id.row >= first
                && (p.x - at.x).hypot(p.y - at.y) <= radius + gup_core::PICK_TOLERANCE.0 + 1.0,
        );
    }
    assert!(hits > 100, "{hits} grid points hit a bubble");
    assert_eq!(near, hits, "a picked row is not a held row under the pixel");
}

/// PROBE (GUP-742, temporary).
#[test]
fn probe_gup749() {
    let cx = Context::new_blocking().unwrap();
    let mut s = 0x1234_5678_9abc_def1u64;
    let random: Vec<Row> = walk(probe_rows())
        .into_iter()
        .map(|(x, y, _)| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (x, y, f64::from(((s >> 11) % 16) as u32))
        })
        .collect();
    let tag = std::env::var("GUP749_TAG").unwrap_or_default();
    for (name, rows) in [("random", random), ("clumps", bubbles(probe_rows()))] {
        let (mut auto, mut off) = (
            bubble_plot(rows.clone(), Lod::Auto),
            bubble_plot(rows.clone(), Lod::Off),
        );
        let (a, o) = (draw(&cx, &mut auto), draw(&cx, &mut off));
        save(&a.image, &format!("p749-{name}-auto{tag}.png"));
        save(&o.image, &format!("p749-{name}-off.png"));
        let (_, rect) = shares(&a.image, &o.image, &auto);
        let mut diff = a.image.clone();
        for (x, y, p) in diff.enumerate_pixels_mut() {
            let q = o.image.get_pixel(x, y);
            let pa = gup_visual_regression::Rgba8::new(p[0], p[1], p[2], p[3]);
            let pb = gup_visual_regression::Rgba8::new(q[0], q[1], q[2], q[3]);
            *p = if pa.delta_e(pb) > 10.0 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([q[0] / 3 + 170, q[1] / 3 + 170, q[2] / 3 + 170, 255])
            };
        }
        save(&diff, &format!("p749-{name}-diff{tag}.png"));
        eprintln!(
            "PROBE {tag} {name}: {} instances, {} draws; {rect:.3}% of the plot rect",
            a.instances, a.draw_calls
        );
    }
}

fn probe_rows() -> usize {
    std::env::var("GUP749_ROWS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300_000)
}
