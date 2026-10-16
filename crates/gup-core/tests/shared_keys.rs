// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S7d (GUP-433): layers that share a key scale give each key one
//! code, so a key has one colour and one position in every layer.

use gup_core::ScaleRef;
use gup_core::prelude::*;

type Row = (&'static str, f64);
type Dot = (&'static str, f64, f64);

fn dot_layer(
    plot: &mut Plot,
    (x, y): (&ScaleRef<Linear>, &ScaleRef<Linear>),
    colour: &ScaleRef<Categorical>,
    rows: Vec<Dot>,
) {
    plot.add(Selection::<Dot, Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &Dot| r.1))
        .attr(Circle::Y, y.encode(|r: &Dot| r.2))
        .attr(Circle::RADIUS, Px(8.0))
        .attr(Circle::FILL, colour.encode_key(|r: &Dot| r.0));
}

#[test]
fn shared_categorical_gives_a_key_one_colour_in_every_layer() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let colour = ScaleRef::new(Categorical::okabe_ito());
    let a = vec![("Asia", 1.0, 1.0), ("Europe", 2.0, 2.0)];
    let b = vec![("Africa", 3.0, 3.0), ("Asia", 4.0, 1.5)];
    dot_layer(&mut plot, (&x, &y), &colour, a.clone());
    dot_layer(&mut plot, (&x, &y), &colour, b.clone());
    let (image, _) = plot.render_resolved(&cx, 600, 400).unwrap();
    let swatches: Vec<_> = colour
        .read()
        .legend()
        .unwrap()
        .swatches()
        .iter()
        .map(|s| s.label.to_string())
        .collect();
    assert_eq!(swatches, ["Asia", "Europe", "Africa"]);

    let at = |&(_, px, py): &Dot| {
        image
            .get_pixel(x.read().eval(px) as u32, y.read().eval(py) as u32)
            .0
    };
    let (asia_a, europe, africa, asia_b) = (at(&a[0]), at(&a[1]), at(&b[0]), at(&b[1]));
    for c in 0..4 {
        assert!(
            asia_a[c].abs_diff(asia_b[c]) <= 2,
            "Asia: {asia_a:?} vs {asia_b:?}"
        );
    }
    assert!(asia_a != europe && europe != africa && asia_a != africa);
    assert_ne!(asia_a, [255, 255, 255, 255], "drawn");
}

fn band_layer(
    plot: &mut Plot,
    x: &ScaleRef<Band>,
    y: &ScaleRef<Linear>,
    keys: &[&'static str],
    h: f64,
) {
    let rows: Vec<Row> = keys.iter().map(|&k| (k, h)).collect();
    plot.add(Selection::<Row, Circle>::new(rows))
        .attr(Circle::X, x.encode_key(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(Circle::RADIUS, Px(6.0))
        .attr(Circle::FILL, Color::BLACK);
}

/// The centres of the runs of dark pixels on pixel row `row`.
fn run_centres(image: &image::RgbaImage, row: u32) -> Vec<f64> {
    let mut runs = Vec::new();
    let mut start = None;
    for px in 0..=image.width() {
        let dark = px < image.width() && image.get_pixel(px, row).0[0] < 128;
        match (dark, start) {
            (true, None) => start = Some(px),
            (false, Some(s)) => {
                if px - s >= 10 {
                    runs.push(f64::from(s + px - 1) / 2.0);
                }
                start = None;
            }
            _ => {}
        }
    }
    runs
}

#[test]
fn shared_band_puts_a_key_at_one_x_in_every_layer() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Band::new()), plot.y(Linear::new()));
    band_layer(&mut plot, &x, &y, &["a", "b"], 1.0);
    band_layer(&mut plot, &x, &y, &["b", "c"], 3.0);
    let (image, _) = plot.render_resolved(&cx, 600, 400).unwrap();
    let band = x.read();
    let keys: Vec<&str> = band.keys().iter().map(AsRef::as_ref).collect();
    assert_eq!(keys, ["a", "b", "c"]);
    assert_eq!(x.read().ticks(10).values.len(), 3, "one tick per key");
    let row_of = |h: f64| y.read().eval(h).round() as u32;
    let (a, b) = (
        run_centres(&image, row_of(1.0)),
        run_centres(&image, row_of(3.0)),
    );
    assert_eq!((a.len(), b.len()), (2, 2), "{a:?} {b:?}");
    assert!((a[1] - b[0]).abs() <= 0.5, "key b: {} vs {}", a[1], b[0]);
}

#[test]
fn a_layer_domain_seeds_the_shared_dictionary() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Band::new()), plot.y(Linear::new()));
    plot.add(Selection::<Row, Circle>::new(vec![("a", 1.0)]))
        .attr(
            Circle::X,
            x.encode_key(|r: &Row| r.0).domain(["c", "b", "a"]),
        )
        .attr(Circle::Y, y.encode(|r: &Row| r.1));
    plot.add(Selection::<Row, Circle>::new(vec![("a", 1.0), ("d", 2.0)]))
        .attr(Circle::X, x.encode_key(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1));
    plot.resolve(&cx, 400.0, 300.0).unwrap();
    let band = x.read();
    let keys: Vec<&str> = band.keys().iter().map(AsRef::as_ref).collect();
    assert_eq!(keys, ["c", "b", "a", "d"]);
}

#[test]
fn mismatched_key_types_on_one_scale_are_an_error() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Band::new()), plot.y(Linear::new()));
    plot.add(Selection::<Row, Circle>::new(vec![("a", 1.0)]))
        .attr(Circle::X, x.encode_key(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1));
    plot.add(Selection::<(u32, f64), Circle>::new(vec![(7, 1.0)]))
        .attr(Circle::X, x.encode_owned_key(|r: &(u32, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(u32, f64)| r.1));
    let err = plot.resolve(&cx, 400.0, 300.0).unwrap_err().to_string();
    assert!(err.contains("shared key scale"), "{err}");
    assert!(err.contains("str") && err.contains("u32"), "{err}");
}
