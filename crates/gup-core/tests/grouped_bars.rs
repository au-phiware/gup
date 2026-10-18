// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S10b (GUP-461): grouped bars from the public API. Each group is
//! a `Rect` layer whose x is a slot of the quarter's band
//! (`ScaleRef::sub_band`), so the groups sit side by side in each band.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::prelude::*;
use gup_core::{ImageTarget, RowId, ScaleRef};
use gup_visual_regression::RgbaImage;

const WIDTH: u32 = 720;
const HEIGHT: u32 = 450;
const QUARTERS: [&str; 3] = ["Q1", "Q2", "Q3"];
const GROUPS: [&str; 2] = ["North", "South"];
const VALUES: [[f64; 3]; 2] = [[32.0, 45.0, 28.0], [24.0, 51.0, 39.0]];

type Row = (&'static str, f64);

struct Chart {
    plot: Plot,
    x: ScaleRef<Band>,
    y: ScaleRef<Linear>,
}

/// Three quarters, two groups. `slots` is how many slots a band holds and
/// `slot_of` the slot each group takes.
fn chart(slots: u32, slot_of: impl Fn(usize) -> u32, padding: f64) -> Chart {
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Band::new()), plot.y(Linear::new().domain(0.0, 60.0)));
    let colour = ScaleRef::new(Categorical::okabe_ito());
    plot.title("Sales by quarter");
    for (g, group) in GROUPS.iter().enumerate() {
        let rows: Vec<Row> = QUARTERS.iter().copied().zip(VALUES[g]).collect();
        let baseline = y.clone();
        let tag = *group;
        plot.add(Selection::<Row, Rect>::new(rows))
            .attr(
                Rect::X,
                x.sub_band(slot_of(g), slots)
                    .encode_key(|r: &Row| r.0)
                    .domain(QUARTERS),
            )
            .attr(Rect::WIDTH, x.sub_band_width_px(slots, padding))
            .attr(Rect::Y, y.encode(|r: &Row| r.1))
            .attr(
                Rect::Y_END,
                Derived::new(move || Px(baseline.read().eval(0.0) as f32)),
            )
            .attr(Rect::FILL, colour.encode_key(move |_: &Row| tag));
    }
    Chart { plot, x, y }
}

fn render(cx: &Context, plot: &mut Plot) -> (gup_core::Resolved, image::RgbaImage) {
    let resolved = plot.resolve(cx, WIDTH as f32, HEIGHT as f32).unwrap();
    let image = ImageTarget::new(cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap();
    (resolved.clone(), image)
}

/// Non-background runs of at least 3 px along row `y` between `x0` and
/// `x1` (the rule of the dogfood crate's `count_runs`).
fn count_runs(image: &image::RgbaImage, y: u32, x0: u32, x1: u32) -> usize {
    let bg = image.get_pixel(0, 0).0;
    let (mut runs, mut len) = (0, 0);
    for x in x0..x1 {
        let p = image.get_pixel(x, y).0;
        if (0..3).any(|c| p[c].abs_diff(bg[c]) > 8) {
            len += 1;
        } else {
            if len >= 3 {
                runs += 1;
            }
            len = 0;
        }
    }
    if len >= 3 {
        runs += 1;
    }
    runs
}

#[test]
fn groups_sit_side_by_side_in_each_band() {
    let cx = Context::new_blocking().unwrap();
    let mut c = chart(2, |g| g as u32, 0.1);
    let (resolved, image) = render(&cx, &mut c.plot);
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    let meta = layout_metadata(&resolved.layout)
        .with_expected_color("North fill", rgba8(Categorical::okabe_ito().eval(0.0)))
        .with_expected_color("South fill", rgba8(Categorical::okabe_ito().eval(1.0)));
    harness()
        .run("gup_core/grouped_bars", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    svg_marks::assert_matches("gup_core/grouped_bars", &resolved.scene, &vr, &meta);

    // One run per bar along a row just above the baseline: six bars, with
    // a gap inside each band.
    let floor = c.y.read().eval(0.0) as u32;
    let plot = resolved.layout.plot;
    let (x0, x1) = (plot.left() as u32 + 2, plot.right() as u32 - 2);
    assert_eq!(count_runs(&image, floor - 3, x0, x1), 6);

    // The ticks stay under each band's centre, and every bar's span is the
    // slot's: half a band each, less the padding.
    let band = c.x.read().band_width();
    for q in 0..3 {
        let centre = c.x.read().eval(f64::from(q));
        for (g, slot) in [(0usize, 0u32), (1, 1)] {
            let want = centre + ((f64::from(slot) + 0.5) / 2.0 - 0.5) * band;
            let width = band / 2.0 * 0.9;
            let ink = |px: f64| image.get_pixel(px as u32, floor - 3).0[0] < 250;
            assert!(
                ink(want) && ink(want - width / 2.0 + 1.0) && ink(want + width / 2.0 - 1.0),
                "group {g}, quarter {q}: no bar around {want}"
            );
            assert!(
                !ink(want - width / 2.0 - 2.0) || g == 1,
                "group {g}, quarter {q}: bar wider than its slot"
            );
        }
        // The gap between the pair is at the band's centre.
        assert!(
            image.get_pixel(centre as u32, floor - 3).0 == image.get_pixel(0, 0).0,
            "quarter {q}: no gap at the centre"
        );
    }
}

/// A pick at each bar's middle returns that bar's row, on the CPU and on
/// the GPU.
#[test]
fn picks_return_the_bar_under_the_cursor() {
    let cx = Context::new_blocking().unwrap();
    let mut c = chart(2, |g| g as u32, 0.1);
    let (_, _) = render(&cx, &mut c.plot);
    let floor = c.y.read().eval(0.0);
    let band = c.x.read().band_width();
    for (g, values) in VALUES.iter().enumerate() {
        for (q, v) in values.iter().enumerate() {
            let centre = c.x.read().eval(q as f64) + ((g as f64 + 0.5) / 2.0 - 0.5) * band;
            let mid = (floor + c.y.read().eval(*v)) / 2.0;
            let at = gup_core::geom::Point::new(centre as f32, mid as f32);
            let id = RowId { layer: g, row: q };
            assert_eq!(c.plot.pick_row(at), Some(id), "CPU pick of {id:?}");
            assert_eq!(c.plot.pick_row_gpu(&cx, at).unwrap(), Some(id), "GPU pick");
        }
    }
}

/// `sub_band(0, 1)` is the band itself: the same pixels as encoding the
/// band directly with `band_width_px`.
#[test]
fn a_single_slot_is_the_band() {
    let cx = Context::new_blocking().unwrap();
    let mut plain = Plot::new();
    let (x, y) = (plot_axes(&mut plain), plain_y(&mut plain));
    for (g, _) in GROUPS.iter().enumerate() {
        let rows: Vec<Row> = QUARTERS.iter().copied().zip(VALUES[g]).collect();
        let baseline = y.clone();
        plain
            .add(Selection::<Row, Rect>::new(rows))
            .attr(Rect::X, x.encode_key(|r: &Row| r.0).domain(QUARTERS))
            .attr(Rect::WIDTH, x.band_width_px())
            .attr(Rect::Y, y.encode(|r: &Row| r.1))
            .attr(
                Rect::Y_END,
                Derived::new(move || Px(baseline.read().eval(0.0) as f32)),
            )
            .attr(Rect::FILL, Color::hex(0x336699));
    }
    // Same fill in both, so only the geometry differs.
    let mut slotted_plain = Plot::new();
    let (sx, sy) = (plot_axes(&mut slotted_plain), plain_y(&mut slotted_plain));
    for (g, _) in GROUPS.iter().enumerate() {
        let rows: Vec<Row> = QUARTERS.iter().copied().zip(VALUES[g]).collect();
        let baseline = sy.clone();
        slotted_plain
            .add(Selection::<Row, Rect>::new(rows))
            .attr(
                Rect::X,
                sx.sub_band(0, 1).encode_key(|r: &Row| r.0).domain(QUARTERS),
            )
            .attr(Rect::WIDTH, sx.sub_band_width_px(1, 0.0))
            .attr(Rect::Y, sy.encode(|r: &Row| r.1))
            .attr(
                Rect::Y_END,
                Derived::new(move || Px(baseline.read().eval(0.0) as f32)),
            )
            .attr(Rect::FILL, Color::hex(0x336699));
    }
    let (_, a) = render(&cx, &mut plain);
    let (_, b) = render(&cx, &mut slotted_plain);
    assert!(
        a.as_raw() == b.as_raw(),
        "sub_band(0, 1) differs from the band"
    );
}

fn plot_axes(plot: &mut Plot) -> ScaleRef<Band> {
    plot.x(Band::new())
}

fn plain_y(plot: &mut Plot) -> ScaleRef<Linear> {
    plot.y(Linear::new().domain(0.0, 60.0))
}

/// The offset is a uniform: changing the plot's size moves the bars
/// without writing column bytes, and the bars still tile their slots.
#[test]
fn a_resize_moves_the_bars_with_uniform_writes_only() {
    let cx = Context::new_blocking().unwrap();
    let mut c = chart(2, |g| g as u32, 0.1);
    let _ = c.plot.resolve(&cx, 720.0, 450.0).unwrap();
    let start = cx.upload_stats();
    let resolved = c.plot.resolve(&cx, 500.0, 450.0).unwrap();
    let image = ImageTarget::new(&cx, 500, 450)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let written = cx.upload_stats() - start;
    assert_eq!(written.columns.bytes, 0, "{written:?}");
    let floor = c.y.read().eval(0.0) as u32;
    let plot = resolved.layout.plot;
    assert_eq!(
        count_runs(
            &image,
            floor - 3,
            plot.left() as u32 + 2,
            plot.right() as u32 - 2
        ),
        6
    );
    let band = c.x.read().band_width();
    let want = c.x.read().eval(0.0) + 0.25 * band;
    assert!(
        image.get_pixel(want as u32, floor - 3).0[0] < 250,
        "lags the resize"
    );
}

/// GUP-461 AC1: an `Offset` whose `Derived` changes between two resolves
/// moves the marks with no column bytes written, and only uniform writes.
#[test]
fn a_changed_offset_is_a_uniform_write_only() {
    use std::sync::{Arc, Mutex};
    let cx = Context::new_blocking().unwrap();
    let dx = Arc::new(Mutex::new(0.0f32));
    let shared = Arc::clone(&dx);
    let mut plot = Plot::new();
    let (x, y) = (
        plot.x(Linear::new().domain(0.0, 10.0)),
        plot.y(Linear::new().domain(0.0, 10.0)),
    );
    plot.add(Selection::<(f64, f64), Rect>::new(vec![(5.0, 5.0)]))
        .attr(
            Rect::X,
            x.then(Offset::by(Derived::new(move || {
                Px(*shared.lock().unwrap())
            })))
            .encode(|r: &(f64, f64)| r.0),
        )
        .attr(Rect::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Rect::WIDTH, Px(20.0))
        .attr(Rect::HEIGHT, Px(20.0))
        .attr(Rect::FILL, Color::BLACK);
    let (_, before) = render(&cx, &mut plot);
    let centre = x.read().eval(5.0);
    let row = y.read().eval(5.0) as u32;
    assert!(before.get_pixel(centre as u32, row).0[0] < 10);
    assert!(before.get_pixel(centre as u32 + 60, row).0[0] > 245);

    *dx.lock().unwrap() = 60.0;
    // The closure reads state the plot cannot see: touch the layer so the
    // cached resolve is not reused (see `Derived`).
    let _ = plot.layer_mut::<(f64, f64), Rect>();
    let start = cx.upload_stats();
    let (_, after) = render(&cx, &mut plot);
    let written = cx.upload_stats() - start;
    assert!(
        after.get_pixel(centre as u32 + 60, row).0[0] < 10,
        "did not move"
    );
    assert!(after.get_pixel(centre as u32, row).0[0] > 245);
    assert_eq!(written.columns.bytes, 0, "{written:?}");
    assert!(written.uniforms.bytes > 0, "{written:?}");
    assert_eq!(written.textures.bytes, 0, "{written:?}");
    eprintln!("offset change wrote {written:?}");
}
