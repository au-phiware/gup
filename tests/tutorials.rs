// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The images embedded in `docs/tutorials/` (GUP-530), checked as goldens.
//!
//! Each test repeats the chart calls of one tutorial block, so a tutorial
//! whose chart changes fails here instead of showing a stale picture. Bless
//! with `GUP_BLESS=1 cargo test -p gup --test tutorials -- --test-threads=1`.

use gup::prelude::*;
use gup::scene::TextRole;
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{
    ExpectedFailures, GoldenStore, Harness, LayoutMetadata, PxRect, Rgba8, RgbaImage,
    TextRole as VrTextRole,
};
use std::path::Path;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;

fn rect(r: gup::geom::Rect) -> PxRect {
    PxRect::new(r.x, r.y, r.width, r.height)
}

/// Layout metadata from gup's own `Layout` (a copy of the adapter in
/// `crates/gup-core/tests/common/vr.rs`, which this crate cannot import).
fn layout_metadata(layout: &gup::Layout) -> LayoutMetadata {
    let mut meta = LayoutMetadata::new(rect(layout.plot)).with_background(Rgba8::WHITE);
    for t in &layout.texts {
        let role = match t.run.role {
            TextRole::Title => VrTextRole::Title,
            TextRole::TickLabel => VrTextRole::TickLabel,
            TextRole::AxisTitle => VrTextRole::AxisTitle,
            TextRole::Legend => VrTextRole::Legend,
            TextRole::Tooltip => VrTextRole::Other,
        };
        let [r, g, b, _] = t.run.style.color.to_rgba8();
        meta = meta.with_text(
            role,
            t.run.text.to_string(),
            rect(t.bounds),
            Rgba8::rgb(r, g, b),
        );
    }
    for g in &layout.guides {
        meta = meta.with_guide(rect(*g).inflate(0.5));
    }
    meta
}

fn harness() -> Harness {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let golden = GoldenStore::new(root.join("tests/golden"), default_artifact_dir(root));
    let expected =
        ExpectedFailures::load(root.join("tests/visual_regression/expected_failures.toml"))
            .expect("expected-failure list parses");
    Harness::new(golden, expected)
}

/// Render `chart` at the tutorials' size and check it against the golden.
fn check(name: &str, chart: &mut impl Chart) {
    let cx = Context::shared().unwrap();
    let (image, layout) = chart.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    assert_eq!(image.dimensions(), (WIDTH, HEIGHT));
    let image = RgbaImage::new(WIDTH, HEIGHT, image.into_raw()).unwrap();
    harness()
        .run(name, Ok((image, layout_metadata(&layout))))
        .assert_ok();
}

/// Tutorial 1 (`01_getting_started.md`), "Step 5: Titles, a Grid and a Log
/// Scale": the scatter chart.
#[test]
fn tutorial_1_scatter() {
    struct Country {
        region: &'static str,
        gdp: f64,
        life: f64,
        people: f64,
    }
    let countries = [
        ("Africa", 2.1e3, 53.0, 200e6),
        ("Africa", 1.7e3, 67.0, 54e6),
        ("Africa", 0.9e3, 66.0, 120e6),
        ("Europe", 5.1e4, 81.0, 83e6),
        ("Europe", 4.4e4, 82.0, 67e6),
        ("Europe", 1.8e4, 77.0, 38e6),
        ("Asia", 1.2e4, 77.0, 1400e6),
        ("Asia", 2.3e3, 70.0, 1380e6),
        ("Asia", 4.0e4, 85.0, 126e6),
        ("Americas", 6.3e4, 79.0, 330e6),
        ("Americas", 7.5e3, 75.0, 212e6),
        ("Americas", 9.9e3, 75.0, 128e6),
    ]
    .map(|(region, gdp, life, people)| Country {
        region,
        gdp,
        life,
        people,
    });

    let mut chart = gup::scatter(countries)
        .x(|c| c.gdp)
        .y(|c| c.life)
        .color(|c| c.region)
        .size(|c| c.people)
        .x_scale(Log::new())
        .title("Wealth and health")
        .x_title("GDP per person (US$)")
        .y_title("Life expectancy (years)")
        .grid(Axes::Both);

    check("tutorials/01_scatter", &mut chart);
}

/// Tutorial 1 (`01_getting_started.md`), "Step 7: A Bar Chart": the grouped
/// bars.
#[test]
fn tutorial_1_grouped_bars() {
    struct Sale {
        quarter: &'static str,
        region: &'static str,
        total: f64,
    }
    let sales = ["Q1", "Q2", "Q3", "Q4"].into_iter().flat_map(|quarter| {
        [("North", 3.0), ("South", 5.0), ("East", 4.0)].map(|(region, base)| Sale {
            quarter,
            region,
            total: base + if quarter == "Q3" { 2.0 } else { 0.0 },
        })
    });

    let mut chart = gup::bar(sales)
        .x(|s| s.quarter)
        .y(|s| s.total)
        .group(|s| s.region)
        .title("Sales by quarter")
        .y_title("Sales (units)")
        .grid(Axes::Y);

    check("tutorials/01_grouped_bars", &mut chart);
}

/// Tutorial 2 (`02_data_binding.md`), "Step 4: Colour Through a Scale": a line
/// layer and a scatter layer coloured by value, with a legend.
#[test]
fn tutorial_2_layers() {
    #[derive(Clone)]
    struct Month {
        n: f64,
        mean: f64,
    }
    let months = [
        (1.0, 22.0),
        (2.0, 22.0),
        (3.0, 21.0),
        (4.0, 18.0),
        (5.0, 15.0),
        (6.0, 13.0),
        (7.0, 12.0),
        (8.0, 13.0),
        (9.0, 16.0),
        (10.0, 18.0),
        (11.0, 20.0),
        (12.0, 21.0),
    ]
    .map(|(n, mean)| Month { n, mean });

    let mut plot = Plot::new();
    plot.title("Monthly temperature")
        .x_title("Month")
        .y_title("Mean temperature (°C)");
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let heat = Sequential::viridis();

    plot.line(months.clone())
        .x(|m| m.n)
        .y(|m| m.mean)
        .color(Color::hex(0xb0b0b0));
    plot.add(Selection::<Month, Circle>::new(months))
        .attr(Circle::X, x.encode(|m: &Month| m.n))
        .attr(Circle::Y, y.encode(|m: &Month| m.mean))
        .attr(Circle::FILL, heat.encode(|m: &Month| m.mean))
        .attr(Circle::RADIUS, Px(6.0))
        .legend_title(Circle::FILL, "Mean (°C)");

    check("tutorials/02_layers", &mut plot);
}

/// Tutorial 3 (`03_custom_shader_functions.md`), "Step 2: Write Your Own":
/// readings coloured through a user-written ramp.
#[test]
fn tutorial_3_custom_ramp() {
    #[derive(Clone, Debug)]
    struct Ember {
        stops: [Color; 4],
    }

    #[gup::wgsl_function(
        path = "tutorial3_ember",
        wgsl = r#"
        struct Params { dark: vec4<f32>, red: vec4<f32>, amber: vec4<f32>, pale: vec4<f32> }

        fn ember(t: f32, p: Params) -> vec4<f32> {
            let k = clamp(t, 0.0, 1.0) * 3.0;
            let a = mix(p.dark, p.red, clamp(k, 0.0, 1.0));
            let b = mix(a, p.amber, clamp(k - 1.0, 0.0, 1.0));
            return mix(b, p.pale, clamp(k - 2.0, 0.0, 1.0));
        }
    "#
    )]
    impl ShaderFn for Ember {
        fn params(&self) -> EmberParams {
            let [dark, red, amber, pale] = self.stops;
            EmberParams {
                dark,
                red,
                amber,
                pale,
            }
        }
    }

    struct Reading {
        x: f64,
        y: f64,
        level: f64,
    }
    let readings: Vec<Reading> = (0..40)
        .map(|i| {
            let (x, y) = (f64::from(i % 8), f64::from(i / 8));
            Reading {
                x,
                y,
                level: (x + 2.0 * y) / 15.0,
            }
        })
        .collect();
    let ember = Ember {
        stops: [0x1a0a00, 0xb2182b, 0xfdb863, 0xffffbf].map(Color::hex),
    };

    let mut plot = Plot::new();
    plot.title("Sensor readings")
        .x_title("Column")
        .y_title("Row");
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.add(Selection::<Reading, Circle>::new(readings))
        .attr(Circle::X, x.encode(|r: &Reading| r.x))
        .attr(Circle::Y, y.encode(|r: &Reading| r.y))
        .attr(Circle::RADIUS, Px(14.0))
        .attr(Circle::FILL, ember.encode(|r: &Reading| r.level));

    check("tutorials/03_custom_ramp", &mut plot);
}

/// Tutorial 6 (`06_custom_marks.md`), "Step 1: A Square": a user-written
/// mark, one square per day, coloured through viridis.
#[test]
fn tutorial_6_squares() {
    #[derive(Debug, Mark)]
    #[mark(
        path = "tutorial6_square",
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

    struct Day {
        week: f64,
        weekday: f64,
        steps: f64,
    }
    let days: Vec<Day> = (0..28)
        .map(|i| {
            let (week, weekday) = (f64::from(i / 7), f64::from(i % 7));
            let steps = 4000.0 + 900.0 * ((i * 5) % 11) as f64 + 1500.0 * f64::from(weekday > 4.5);
            Day {
                week,
                weekday,
                steps,
            }
        })
        .collect();

    let mut plot = Plot::new();
    plot.title("Steps per day")
        .x_title("Weekday (Monday is 0)")
        .y_title("Week");
    let steps = Sequential::viridis();
    plot.add(Selection::<Day, Square>::new(days))
        .x(|d| d.weekday)
        .y(|d| d.week)
        .size(Px(36.0))
        .fill(steps.encode(|d: &Day| d.steps))
        .legend_title(Square::FILL, "Steps");

    check("tutorials/06_squares", &mut plot);
}

/// Tutorial 4 (`04_interactions.md`), "Step 5: Brushing": nine birds with
/// five of them brushed, so `Unbrushed` dims the rest.
#[test]
fn tutorial_4_brush() {
    use gup::geom::Point;
    use gup::{Button, Input};

    struct Bird {
        mass: f64,
        span: f64,
    }
    let birds = [
        (0.03, 0.20),
        (0.11, 0.35),
        (0.17, 0.55),
        (0.50, 0.75),
        (0.90, 0.95),
        (1.00, 1.30),
        (1.50, 0.55),
        (2.10, 0.30),
        (2.70, 0.40),
    ]
    .map(|(mass, span)| Bird { mass, span });

    let mut chart = gup::scatter(birds)
        .x(|b| b.mass)
        .y(|b| b.span)
        .radius(6.0)
        .title("Birds of Aotearoa")
        .x_title("Mass (kg)")
        .y_title("Wingspan (m)");
    chart.plot_mut().drag(Drag::Brush);

    let cx = Context::shared().unwrap();
    let plot = chart
        .resolve(&cx, WIDTH as f32, HEIGHT as f32)
        .unwrap()
        .layout
        .plot;
    let ticks = chart.layout().unwrap().x_ticks.clone();
    let (first, last) = (ticks[0], ticks[ticks.len() - 1]);
    let kea_x =
        first.1 + ((0.95 - first.0) * f64::from(last.1 - first.1) / (last.0 - first.0)) as f32;
    let from = Point::new(plot.left() + 1.0, plot.bottom() - 1.0);
    let to = Point::new(kea_x, plot.top() + 1.0);
    let button = Button::Primary;
    let _ = chart.handle(Input::PointerDown { at: from, button });
    let _ = chart.handle(Input::PointerMoved { at: to });
    let _ = chart.handle(Input::PointerUp { at: to, button });
    assert_eq!(chart.plot_mut().brush().unwrap().rows.len(), 5);

    check("tutorials/04_brush", &mut chart);
}
