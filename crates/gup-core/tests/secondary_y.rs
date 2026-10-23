// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S11 (GUP-479): a second y axis on the right. Rainfall bars read
//! against the right axis and a temperature line against the left one.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::geom::Point;
use gup_core::prelude::*;
use gup_core::scene::TextRole;
use gup_core::{Button, ImageTarget, Input, RowId};
use gup_visual_regression::RgbaImage;

const WIDTH: u32 = 720;
const HEIGHT: u32 = 450;
const RAIN: [f64; 12] = [
    78.0, 64.0, 52.0, 38.0, 26.0, 20.0, 22.0, 30.0, 48.0, 90.0, 140.0, 180.0,
];
const TEMP: [f64; 12] = [
    26.0, 25.0, 23.0, 20.0, 16.0, 12.0, 8.0, 10.0, 13.0, 17.0, 21.0, 24.0,
];
const LINE_COLOR: Color = Color::hex(0xd55e00);

struct Month {
    n: f64,
    rain: f64,
    temp: f64,
}

struct Fixture {
    cx: Context,
    plot: Plot,
    x: ScaleRef<Linear>,
    y: ScaleRef<Linear>,
    right: ScaleRef<Linear>,
}

fn months() -> Vec<Month> {
    (0..12)
        .map(|i| Month {
            n: i as f64 + 1.0,
            rain: RAIN[i],
            temp: TEMP[i],
        })
        .collect()
}

fn fixture() -> Fixture {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let right = plot.y_right(Linear::new());
    let floor = right.clone();
    plot.title("Rainfall and temperature")
        .x_title("Month")
        .y_title("Temperature (°C)")
        .y_right_title("Rainfall (mm)");
    plot.add(Selection::<Month, Rect>::new(months()))
        .attr(Rect::X, x.encode(|m: &Month| m.n))
        .attr(Rect::WIDTH, Px(18.0))
        .attr(Rect::Y, right.encode(|m: &Month| m.rain))
        .attr(
            Rect::Y_END,
            Derived::new(move || Px(floor.read().eval(0.0) as f32)),
        );
    plot.add(Selection::<Month, Line>::new(months()))
        .attr(Line::X, x.encode(|m: &Month| m.n))
        .attr(Line::Y, y.encode(|m: &Month| m.temp))
        .attr(Line::WIDTH, Px(2.0))
        .attr(Line::COLOR, LINE_COLOR);
    Fixture {
        cx,
        plot,
        x,
        y,
        right,
    }
}

fn send(s: &mut Fixture, input: Input) {
    let _ = s.plot.handle(input);
}

fn resolve(s: &mut Fixture) {
    s.plot.resolve(&s.cx, WIDTH as f32, HEIGHT as f32).unwrap();
}

fn render(s: &mut Fixture) -> (image::RgbaImage, gup_core::Resolved) {
    let resolved = s
        .plot
        .resolve(&s.cx, WIDTH as f32, HEIGHT as f32)
        .unwrap()
        .clone();
    let image = ImageTarget::new(&s.cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(&s.cx, &resolved.scene)
        .unwrap();
    (image, resolved)
}

fn ink_is(image: &image::RgbaImage, x: f32, y: f32, want: impl Fn([u8; 4]) -> bool) -> bool {
    (-1..=1).any(|dy| {
        (-1..=1).any(|dx| {
            let (px, py) = ((x as i32 + dx) as u32, (y as i32 + dy) as u32);
            want(image.get_pixel(px, py).0)
        })
    })
}

/// AC3: two labelled y axes, each series against its own.
#[test]
fn bars_read_against_the_right_axis_and_the_line_against_the_left() {
    let mut s = fixture();
    let (image, resolved) = render(&mut s);
    let layout = &resolved.layout;
    let plot = layout.plot;

    let ticks = |right_side: bool| -> Vec<_> {
        layout
            .texts
            .iter()
            .filter(|t| t.run.role == TextRole::TickLabel)
            .filter(|t| {
                if right_side {
                    t.run.at.x > plot.right()
                } else {
                    t.run.at.y <= plot.bottom() + 1.0 && t.run.at.x < plot.left()
                }
            })
            .collect()
    };
    let (right_labels, left_labels) = (ticks(true), ticks(false));
    assert_eq!(right_labels.len(), layout.y_right_ticks.len());
    assert_eq!(left_labels.len(), layout.y_ticks.len());
    assert!(layout.y_right_ticks.len() >= 3);
    for t in &right_labels {
        assert!(t.bounds.left() > plot.right(), "{t:?}");
        assert!(t.bounds.right() <= WIDTH as f32, "{t:?}");
    }
    for t in &left_labels {
        assert!(t.bounds.right() < plot.left(), "{t:?}");
    }
    for &(v, px) in &layout.y_right_ticks {
        assert_eq!(px, s.right.read().eval(v) as f32);
    }
    for &(v, px) in &layout.y_ticks {
        assert_eq!(px, s.y.read().eval(v) as f32);
    }

    // The top of the 180 mm bar, off the line's vertex, and the 26 °C vertex.
    let bar_x = s.x.read().eval(12.0) as f32 - 6.0;
    let want_top = s.right.read().eval(180.0) as f32;
    let top = (plot.top() as u32..HEIGHT)
        .find(|&y| image.get_pixel(bar_x as u32, y).0[0] < 60)
        .unwrap();
    assert!((top as f32 - want_top).abs() <= 1.0, "{top} vs {want_top}");
    let (vx, vy) = (s.x.read().eval(1.0) as f32, s.y.read().eval(26.0) as f32);
    assert!(
        ink_is(&image, vx.round(), vy.round(), |p| p[0] > 180
            && p[1] < 140
            && p[2] < 60),
        "no line at its 26 °C vertex ({vx}, {vy})"
    );

    // Both titles are drawn.
    for want in ["Temperature (°C)", "Rainfall (mm)"] {
        assert!(
            layout.texts.iter().any(|t| t.run.text.as_ref() == want),
            "{want}"
        );
    }

    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    let meta = layout_metadata(layout)
        .with_expected_color("bars", rgba8(Rect::DEFAULT_FILL))
        .with_expected_color("line", rgba8(LINE_COLOR));
    harness()
        .run("gup_core/secondary_y", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    // GUP-467: the SVG writes the same scene.
    svg_marks::assert_matches("gup_core/secondary_y", &resolved.scene, &vr, &meta);
}

/// GUP-445: a pick is made against each layer's own scale.
#[test]
fn picks_find_bars_and_line_vertices() {
    let mut s = fixture();
    let (_, resolved) = render(&mut s);
    let plot = resolved.layout.plot;
    let top = s.right.read().eval(180.0) as f32;
    let base = s.right.read().eval(0.0) as f32;
    let at = Point::new(s.x.read().eval(12.0) as f32 - 6.0, (top + base) / 2.0);
    assert!(at.x > plot.left() && at.x < plot.right() && at.y > plot.top());
    assert_eq!(s.plot.pick_row(at), Some(RowId { layer: 0, row: 11 }));
    let vertex = Point::new(
        s.x.read().eval(1.0) as f32,
        s.y.read().eval(26.0) as f32 + 1.0,
    );
    assert_eq!(s.plot.pick_row(vertex), Some(RowId { layer: 1, row: 0 }));
}

fn domains(s: &Fixture) -> [(f64, f64); 2] {
    [
        s.y.read().current_domain().unwrap(),
        s.right.read().current_domain().unwrap(),
    ]
}

/// The value drawn at pixel row `py` on each y axis.
fn under(s: &Fixture, py: f32) -> [f64; 2] {
    [
        s.y.read().invert(f64::from(py)),
        s.right.read().invert(f64::from(py)),
    ]
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// AC4: everything that moves the y axis moves both.
#[test]
fn zoom_pan_and_reset_move_both_y_axes() {
    let mut s = fixture();
    let (_, resolved) = render(&mut s);
    let plot = resolved.layout.plot;
    let home = domains(&s);
    let centre = Point::new(plot.x + plot.width / 2.0, plot.y + plot.height / 2.0);

    // Wheel zoom: the values under the pointer stay put on both axes.
    let before = under(&s, centre.y);
    send(
        &mut s,
        Input::Wheel {
            at: centre,
            lines: 2.0,
        },
    );
    let after = under(&s, centre.y);
    assert_ne!(domains(&s), home);
    for k in 0..2 {
        assert!(close(before[k], after[k]), "{before:?} {after:?}");
        let (a, b) = domains(&s)[k];
        assert!((b - a).abs() < (home[k].1 - home[k].0).abs());
    }

    // A clean resolve after the zoom writes no column bytes.
    let start = s.cx.upload_stats();
    resolve(&mut s);
    assert_eq!((s.cx.upload_stats() - start).columns.bytes, 0);
    assert_eq!(s.plot.layout().unwrap().plot, plot, "the rect moved");

    // Pan: a value drawn at some row is drawn `dy` pixels further on.
    let (v_left, v_right) = (s.y.read().invert(200.0), s.right.read().invert(200.0));
    s.plot.pan(0.0, 25.0).unwrap();
    assert!((s.y.read().eval(v_left) - 225.0).abs() < 1e-6);
    assert!((s.right.read().eval(v_right) - 225.0).abs() < 1e-6);

    // Only x: both y domains stay.
    s.plot.view_axes(Axes::X);
    let kept = domains(&s);
    send(
        &mut s,
        Input::Wheel {
            at: centre,
            lines: 1.0,
        },
    );
    assert_eq!(domains(&s), kept);
    s.plot.view_axes(Axes::Both);

    // Reset restores both.
    s.plot.reset_view();
    resolve(&mut s);
    for (got, want) in domains(&s).into_iter().zip(home) {
        assert!(close(got.0, want.0) && close(got.1, want.1), "{got:?}");
    }
}

#[test]
fn a_dragged_pan_and_zoom_rectangle_move_both_y_axes() {
    let mut s = fixture();
    let (_, resolved) = render(&mut s);
    let plot = resolved.layout.plot;
    let button = Button::Primary;
    let from = Point::new(plot.x + 100.0, plot.y + 100.0);
    let (v_left, v_right) = (s.y.read().invert(150.0), s.right.read().invert(150.0));
    send(&mut s, Input::PointerDown { at: from, button });
    for step in [20.0, 40.0] {
        let at = Point::new(from.x, from.y + step);
        send(&mut s, Input::PointerMoved { at });
    }
    let to = Point::new(from.x, from.y + 40.0);
    send(&mut s, Input::PointerUp { at: to, button });
    assert!((s.y.read().eval(v_left) - 190.0).abs() < 1e-6);
    assert!((s.right.read().eval(v_right) - 190.0).abs() < 1e-6);

    // Drag::Zoom: both y domains become the rectangle's pixel bounds.
    s.plot.reset_view();
    resolve(&mut s);
    s.plot.drag(Drag::Zoom);
    let (a, b) = (
        Point::new(plot.x + 50.0, plot.y + 60.0),
        Point::new(plot.x + 250.0, plot.y + 200.0),
    );
    let top = under(&s, a.y);
    let bottom = under(&s, b.y);
    send(&mut s, Input::PointerDown { at: a, button });
    send(&mut s, Input::PointerMoved { at: b });
    send(&mut s, Input::PointerUp { at: b, button });
    for (k, (d0, d1)) in domains(&s).into_iter().enumerate() {
        assert!(close(d0, bottom[k]) && close(d1, top[k]), "{k}: {d0} {d1}");
    }
}

/// GUP-426: a colour legend sits right of the right axis and its labels.
#[test]
fn the_legend_clears_the_right_axis() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let right = plot.y_right(Linear::new());
    let heat = Sequential::viridis();
    plot.y_right_title("Rainfall (mm)");
    plot.add(Selection::<Month, Circle>::new(months()))
        .attr(Circle::X, x.encode(|m: &Month| m.n))
        .attr(Circle::Y, y.encode(|m: &Month| m.temp))
        .attr(Circle::FILL, heat.encode(|m: &Month| m.temp))
        .attr(Circle::RADIUS, Px(4.0));
    plot.add(Selection::<Month, Line>::new(months()))
        .attr(Line::X, x.encode(|m: &Month| m.n))
        .attr(Line::Y, right.encode(|m: &Month| m.rain));
    let layout = plot
        .resolve(&cx, WIDTH as f32, HEIGHT as f32)
        .unwrap()
        .layout
        .clone();
    let legend = layout.legend.expect("a colour legend");
    let axis_ink = layout
        .texts
        .iter()
        .filter(|t| t.run.at.x > layout.plot.right() && t.run.at.x < legend.left())
        .map(|t| t.bounds.right())
        .fold(layout.plot.right(), f32::max);
    assert!(axis_ink < legend.left(), "{axis_ink} vs {legend:?}");
    assert!(legend.right() <= WIDTH as f32);
}

/// No right axis: no right ticks.
#[test]
fn a_plot_without_a_right_axis_has_no_right_ticks() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    plot.circles(vec![(1.0, 2.0), (2.0, 3.0)])
        .x(|r| r.0)
        .y(|r| r.1);
    let layout = plot.resolve(&cx, 320.0, 200.0).unwrap().layout.clone();
    assert!(layout.y_right_ticks.is_empty());
}
