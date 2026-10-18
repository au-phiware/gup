// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S6a (GUP-422): a mark defined from outside gup-core with
//! `#[derive(Mark)]`: a struct, its WGSL and nothing else. `Square` draws
//! axis-aligned squares with `gup::coverage`; three of them are rendered
//! and their pixels checked.

use gup_core::ImageTarget;
use gup_core::prelude::*;

/// A filled square centred on `(x, y)`, `size` logical pixels across.
#[derive(Debug, Mark)]
#[mark(
    wgsl = r#"
    #import gup::view::{View, px_to_clip}
    #import gup::coverage::{rect_quad, box_coverage}

    struct SquareIn {
        x: f32,
        y: f32,
        size: f32,
        fill: vec4<f32>,
    }

    struct Varyings {
        @builtin(position) clip: vec4<f32>,
        @location(0) local: vec2<f32>,
        @location(1) half: vec2<f32>,
        @location(2) fill: vec4<f32>,
        @location(3) @interpolate(flat) row: u32,
    }

    fn vertex(m: SquareIn, vertex_index: u32, row: u32, view: View) -> Varyings {
        let centre = vec2<f32>(m.x, m.y);
        let half = vec2<f32>(m.size * 0.5);
        let q = rect_quad(centre - half, centre + half, vertex_index, view.dpr);
        var out: Varyings;
        out.clip = px_to_clip(q.position, view);
        out.local = q.local;
        out.half = q.half;
        out.fill = m.fill;
        out.row = row;
        return out;
    }

    fn shade(v: Varyings) -> vec4<f32> {
        let alpha = v.fill.a * box_coverage(v.local, v.half);
        if (alpha <= 0.0) {
            discard;
        }
        return vec4<f32>(v.fill.rgb * alpha, alpha);
    }
"#,
    geometry = Quad
)]
struct Square {
    #[channel(role = X)]
    x: Px,
    #[channel(role = Y)]
    y: Px,
    #[channel(role = Size, default = Px(10.0))]
    size: Px,
    #[channel(role = Color, default = Color::BLACK)]
    fill: Color,
}

#[test]
fn derived_channels_and_defaults() {
    assert_eq!(Square::NAME, "Square");
    assert_eq!(Square::VERTICES_PER_INSTANCE, 6);
    let names: Vec<_> = Square::CHANNELS.iter().map(|c| c.name).collect();
    assert_eq!(names, ["x", "y", "size", "fill"]);
    assert_eq!(Square::SIZE.slot(), 2);
    assert_eq!(Square::MODULE.import_path, "derive_mark::square");
}

const W: u32 = 320;
const H: u32 = 200;

fn close(got: [u8; 4], want: [u8; 3]) -> bool {
    (0..3).all(|i| got[i].abs_diff(want[i]) <= 2)
}

#[test]
fn user_mark_renders_squares() {
    let cx = Context::new_blocking().expect("headless GPU context");
    let fills = [0x0072B2u32, 0xD55E00, 0x009E73];
    let rows = vec![(2.0f64, "a"), (4.0, "b"), (8.0, "c")];

    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, 10.0));
    let y = plot.y(Linear::new().domain(0.0, 10.0));
    let palette = Categorical::from_palette(fills.map(Color::hex));
    plot.add(Selection::<(f64, &'static str), Square>::new(rows.clone()))
        .x_with(x.encode(|r: &(f64, &'static str)| r.0))
        .y_with(y.encode(|_: &(f64, &'static str)| 4.0))
        .size(Px(20.0))
        .fill(palette.encode_key(|r: &(f64, &'static str)| r.1));
    let resolved = plot.resolve(&cx, W as f32, H as f32).unwrap();
    // GUP-467: a user mark has no CPU description, so the SVG skips the
    // layer with a comment instead of failing.
    let mut svg = gup_core::SvgTarget::new();
    gup_core::VectorTarget::render(&mut svg, &resolved.scene).unwrap();
    assert!(
        svg.svg()
            .contains("<!-- layer 0 skipped: `Square` is a user-defined mark"),
        "{}",
        svg.svg()
    );
    let img = ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    // The squares sit on tick values, whose pixel positions the layout gives.
    let tick = |ticks: &[(f64, f32)], v: f64| {
        ticks
            .iter()
            .find(|(t, _)| *t == v)
            .expect("a tick at the value")
            .1 as u32
    };
    let cy = tick(&resolved.layout.y_ticks, 4.0);
    for ((value, _), fill) in rows.into_iter().zip(fills) {
        let cx_px = tick(&resolved.layout.x_ticks, value);
        let centre = img.get_pixel(cx_px, cy).0;
        let want = Color::hex(fill).to_rgba8();
        assert!(
            close(centre, [want[0], want[1], want[2]]),
            "centre of {value}: {centre:?} vs {want:?}"
        );
        let outside = img.get_pixel(cx_px + 10 + 3, cy).0;
        assert!(
            close(outside, [255, 255, 255]),
            "outside {value}: {outside:?}"
        );
    }
}

/// A mark without `gpu_pick` draws as before, and the GPU pick skips its
/// layers (RFC-001 S9d).
#[test]
fn a_mark_without_gpu_pick_is_skipped_by_the_gpu_pick() {
    const { assert!(!Square::GPU_PICK && Circle::GPU_PICK) };
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, 10.0));
    let y = plot.y(Linear::new().domain(0.0, 10.0));
    plot.add(Selection::<f64, Square>::new(vec![5.0]))
        .x_with(x.encode(|v: &f64| *v))
        .y_with(y.encode(|v: &f64| *v))
        .size(Px(20.0));
    plot.resolve(&cx, W as f32, H as f32).unwrap();
    let at = gup_core::geom::Point::new(x.read().eval(5.0) as f32, y.read().eval(5.0) as f32);
    assert_eq!(plot.pick_row_gpu(&cx, at).unwrap(), None);
}
