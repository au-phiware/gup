// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S7b (GUP-425): the theme's null colour, axis titles and
//! truncated titles, end to end.

mod common;

use common::scatter::{Country, countries_n};
use common::svg_marks;
use common::vr::{harness, layout_metadata};
use gup_core::prelude::*;
use gup_core::scene::TextRole;
use gup_visual_regression::RgbaImage;

const MAGENTA: Color = Color::hex(0xFF00FF);
const GREEN: Color = Color::hex(0x00FF00);

#[derive(Clone)]
struct Dot {
    x: f64,
    value: f64,
    key: Option<&'static str>,
}

fn dots() -> Vec<Dot> {
    vec![
        Dot {
            x: 0.0,
            value: 1.0,
            key: Some("a"),
        },
        Dot {
            x: 1.0,
            value: f64::NAN,
            key: None,
        },
        Dot {
            x: 2.0,
            value: 3.0,
            key: Some("b"),
        },
    ]
}

/// Pixel at the centre of dot `i` (x domain 0..2 over the plot rect).
fn centre(image: &image::RgbaImage, layout: &gup_core::Layout, i: usize) -> [u8; 4] {
    let p = layout.plot;
    // A fixed domain starts on the axis and insets its far end by the radius
    // (10) and the plot padding (4).
    let far = 14.0;
    let px = p.left() + (p.width - far) * i as f32 / 2.0;
    image
        .get_pixel(px as u32, (p.top() + p.height / 2.0) as u32)
        .0
}

fn close(got: [u8; 4], want: Color) {
    let want = want.to_rgba8();
    assert!(
        got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 2),
        "{got:?} vs {want:?}"
    );
}

fn render(plot: &mut Plot) -> (image::RgbaImage, gup_core::Layout) {
    let cx = Context::new_blocking().expect("headless GPU context");
    plot.render_resolved(&cx, 400, 300).unwrap()
}

/// Three dots on a line; `fill` encodes the fill channel.
fn dot_plot(fill: impl FnOnce(&mut Selection<Dot, Circle>)) -> Plot {
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, 2.0));
    let y = plot.y(Linear::new().domain(0.0, 1.0));
    let layer = plot.add(Selection::<Dot, Circle>::new(dots()));
    layer
        .attr(Circle::X, x.encode(|d: &Dot| d.x))
        .attr(Circle::Y, y.encode(|_: &Dot| 0.5))
        .attr(Circle::RADIUS, Px(10.0));
    fill(layer);
    plot
}

#[test]
fn themed_null_colour_for_a_sequential_fill() {
    let mut plot = dot_plot(|layer| {
        let fill = Sequential::viridis()
            .domain(0.0, 4.0)
            .encode(|d: &Dot| d.value);
        layer.attr(Circle::FILL, fill);
    });
    plot.theme(Theme {
        null_color: MAGENTA,
        ..Theme::default()
    });
    let (image, layout) = render(&mut plot);
    close(centre(&image, &layout, 1), MAGENTA);
    assert_ne!(centre(&image, &layout, 0), centre(&image, &layout, 1));
}

#[test]
fn themed_null_colour_for_a_categorical_fill() {
    let mut plot = dot_plot(|layer| {
        let fill = Categorical::okabe_ito().encode_nullable_key(|d: &Dot| d.key);
        layer.attr(Circle::FILL, fill);
    });
    plot.theme(Theme {
        null_color: MAGENTA,
        ..Theme::default()
    });
    let (image, layout) = render(&mut plot);
    close(centre(&image, &layout, 1), MAGENTA);
}

#[test]
fn a_categorical_null_colour_overrides_the_theme() {
    let mut plot = dot_plot(|layer| {
        let fill = Categorical::okabe_ito()
            .null_color(GREEN)
            .encode_nullable_key(|d: &Dot| d.key);
        layer.attr(Circle::FILL, fill);
    });
    plot.theme(Theme {
        null_color: MAGENTA,
        ..Theme::default()
    });
    let (image, layout) = render(&mut plot);
    close(centre(&image, &layout, 1), GREEN);
}

fn scatter(rows: Vec<Country>) -> Plot {
    let max_pop = rows.iter().map(|c| c.population).fold(0.0, f64::max);
    let mut plot = Plot::new();
    plot.legends(false);
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let area = Pow::sqrt().domain(0.0, max_pop).range(Px(0.0), Px(28.0));
    plot.add(Selection::<Country, Circle>::new(rows))
        .attr(Circle::X, x.encode(|c: &Country| c.gdp_per_capita))
        .attr(Circle::Y, y.encode(|c: &Country| c.life_expectancy))
        .attr(Circle::RADIUS, area.encode(|c: &Country| c.population))
        .attr(
            Circle::FILL,
            Sequential::viridis().encode(|c: &Country| c.life_expectancy),
        );
    plot
}

#[test]
fn axis_titles_are_placed_without_overlap() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = scatter(countries_n(60));
    plot.title("Wealth and health")
        .x_title("GDP per capita (US$)")
        .y_title("Life expectancy (years)");
    let (image, layout) = plot.render_resolved(&cx, 720, 450).unwrap();

    let boxes: Vec<_> = layout.texts.iter().collect();
    for (i, a) in boxes.iter().enumerate() {
        let b = a.bounds;
        assert!(
            b.left() >= 0.0 && b.right() <= 720.0 && b.top() >= 0.0 && b.bottom() <= 450.0,
            "{:?} leaves the canvas: {b:?}",
            a.run.text
        );
        for c in &boxes[i + 1..] {
            let d = c.bounds;
            let disjoint = b.right() <= d.left()
                || d.right() <= b.left()
                || b.bottom() <= d.top()
                || d.bottom() <= b.top();
            assert!(
                disjoint,
                "{:?} {b:?} overlaps {:?} {d:?}",
                a.run.text, c.run.text
            );
        }
        if a.run.role == TextRole::AxisTitle {
            let p = layout.plot;
            let apart = b.right() <= p.left()
                || p.right() <= b.left()
                || b.bottom() <= p.top()
                || p.bottom() <= b.top();
            assert!(apart, "axis title {:?} touches the plot {p:?}", a.run.text);
        }
    }
    let titles = boxes
        .iter()
        .filter(|t| t.run.role == TextRole::AxisTitle)
        .count();
    assert_eq!(titles, 2);

    let meta = layout_metadata(&layout);
    let vr = RgbaImage::new(720, 450, image.into_raw()).unwrap();
    harness()
        .run("gup_core/axis_titles", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    // GUP-467: the SVG draws the same marks.
    let scene = plot.resolve(&cx, 720.0, 450.0).unwrap().scene.clone();
    svg_marks::assert_matches("gup_core/axis_titles", &scene, &vr, &meta);
}

#[test]
fn long_titles_are_truncated_with_an_ellipsis() {
    let cx = Context::new_blocking().unwrap();
    let long = "Wide ".repeat(60);
    assert_eq!(long.len(), 300);
    let mut plot = scatter(countries_n(20));
    plot.title(long.clone()).x_title(long.clone()).y_title(long);
    let layout = plot.resolve(&cx, 320.0, 200.0).unwrap().layout.clone();
    let by_role =
        |role: TextRole| -> Vec<_> { layout.texts.iter().filter(|t| t.run.role == role).collect() };
    let title = by_role(TextRole::Title);
    let axis = by_role(TextRole::AxisTitle);
    assert_eq!((title.len(), axis.len()), (1, 2));
    for t in title.iter().chain(&axis) {
        assert!(t.run.text.ends_with('…'), "{:?}", t.run.text);
        assert!(t.run.text.chars().count() < 300);
    }
    assert!(title[0].bounds.width <= layout.plot.width);
    // The x title is first (centred), the y title second.
    assert!(axis[0].bounds.width <= layout.plot.width);
    assert!(axis[1].bounds.width <= 320.0 - 24.0);
}
