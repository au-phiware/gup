// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S7h (GUP-437): legends for chains and for a user's colour
//! functions, on the CPU (no GPU needed).

use gup_core::Legend;
use gup_core::prelude::*;

#[test]
fn a_chain_into_a_palette_labels_the_data_not_the_image() {
    let mut chain = Linear::new().then(Pow::sqrt()).then(Sequential::viridis());
    assert!(chain.legend().is_none(), "unfitted chains have no extent");
    chain.fit((0.0, 4e7)).unwrap();
    let Some(Legend::Ramp(ramp)) = chain.legend() else {
        panic!("a ramp");
    };
    let values: Vec<f64> = ramp.ticks().iter().map(|t| t.value).collect();
    assert_eq!(values, [0.0, 1e7, 2e7, 3e7, 4e7]);
    let labels: Vec<&str> = ramp.ticks().iter().map(|t| &*t.label).collect();
    assert_eq!(
        labels,
        ["0", "10000000", "20000000", "30000000", "40000000"]
    );
    let (lo, hi) = ramp.domain();
    for tick in ramp.ticks() {
        let image = chain.first().eval(tick.value);
        let want = (image - lo) / (hi - lo);
        assert!(
            (tick.t - want).abs() < 1e-9,
            "{}: {} vs {want}",
            tick.label,
            tick.t
        );
    }
    // The square root bends the bar: the middle tick sits past the middle.
    assert!(ramp.ticks()[2].t > 0.6);
}

/// A colour function with a mirror and no `legend`: red to blue.
#[derive(Clone, Debug)]
struct Heat;

#[wgsl_function(
    path = "chain_legend::heat",
    wgsl = r#"
    struct Params { pad: vec4<f32> }
    fn heat(v: f32, p: Params) -> vec4<f32> { return vec4<f32>(v, 0.0, 1.0 - v, 1.0); }
"#
)]
impl ShaderFn for Heat {
    fn params(&self) -> HeatParams {
        HeatParams::default()
    }
}

impl CpuMirror for Heat {
    fn eval(&self, x: f64) -> Color {
        let t = x as f32;
        Color {
            r: t,
            g: 0.0,
            b: 1.0 - t,
            a: 1.0,
        }
    }
}

/// The same, with its own `legend`.
#[derive(Clone, Debug)]
struct Own;

#[wgsl_function(
    path = "chain_legend::own",
    wgsl = r#"
    struct Params { pad: vec4<f32> }
    fn own(v: f32, p: Params) -> vec4<f32> { return vec4<f32>(v, v, v, 1.0); }
"#
)]
impl ShaderFn for Own {
    fn params(&self) -> OwnParams {
        OwnParams::default()
    }

    fn legend(&self) -> Option<Legend> {
        Some(Legend::Swatches(vec![]))
    }
}

impl CpuMirror for Own {
    fn eval(&self, x: f64) -> Color {
        Color {
            r: x as f32,
            g: x as f32,
            b: x as f32,
            a: 1.0,
        }
    }
}

#[test]
fn a_user_colour_function_gets_a_sampled_ramp_unless_it_has_its_own() {
    let Some(Legend::Ramp(ramp)) = Heat.sampled_legend((0.0, 1.0)) else {
        panic!("a sampled ramp");
    };
    assert_eq!(ramp.domain(), (0.0, 1.0));
    assert_eq!(ramp.ticks().len(), 6, "0, 0.2 … 1");
    let mid = ramp.color_at(0.5);
    assert!((mid.r - 0.5).abs() < 0.01 && (mid.b - 0.5).abs() < 0.01);
    let end = ramp.color_at(1.0);
    assert!((end.r - 1.0).abs() < 0.01 && end.b < 0.01);

    // Through a chain, ticks are in the data's units.
    let mut chain = Linear::new().then(Heat);
    chain.fit((0.0, 50.0)).unwrap();
    let Some(Legend::Ramp(chained)) = chain.legend() else {
        panic!("a ramp");
    };
    assert_eq!(chained.ticks().last().unwrap().value, 50.0);

    assert!(matches!(Own.legend(), Some(Legend::Swatches(_))));
    assert!(matches!(
        EncodeFn::encode_legend_over(&Own, (0.0, 1.0)),
        Some(Legend::Swatches(_))
    ));

    // A chain ending in a swatch legend has none: swatches are keys, and
    // a chain's data is not.
    let mut swatched = Linear::new().then(Own);
    swatched.fit((0.0, 1.0)).unwrap();
    assert!(swatched.legend().is_none());
}

/// A plot places a chain's legend, labelled in the data's units.
#[test]
fn a_plot_places_the_legend_of_a_chained_colour() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(f64, f64)> = (0..40)
        .map(|i| (f64::from(i), f64::from(i * i) * 1e4))
        .collect();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let colour = Pow::sqrt().then(Sequential::viridis());
    plot.add(Selection::<(f64, f64), Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Circle::RADIUS, Px(3.0))
        .attr(Circle::FILL, colour.encode(|r: &(f64, f64)| r.1))
        .legend_title(Circle::FILL, "Cost");
    let resolved = plot.resolve(&cx, 640.0, 400.0).unwrap();
    let layout = resolved.layout;
    assert!(layout.legend.is_some());
    let labels: Vec<String> = layout
        .texts
        .iter()
        .filter(|t| t.run.role == gup_core::scene::TextRole::Legend)
        .map(|t| t.run.text.to_string())
        .collect();
    assert!(labels.iter().any(|l| l == "Cost"), "{labels:?}");
    // Round values of the data (up to 1.5e7), not of its square root.
    assert!(labels.iter().any(|l| l == "10000000"), "{labels:?}");
}
