// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 12 of the tour: below the builders. A colour ramp and a mark,
//! each written in WGSL, drawn through a `Plot` with layers.

use gup::prelude::*;

// tour:begin ember
/// A colour ramp through four stops, dark to pale.
#[derive(Clone, Debug)]
struct Ember {
    stops: [Color; 4],
}

#[gup::wgsl_function(
    path = "tour_ember",
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
// tour:end ember

struct Reading {
    x: f64,
    y: f64,
    level: f64,
}

pub fn function() -> impl Chart {
    // tour:begin function
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
    plot
    // tour:end function
}

/// One day of steps, in a grid of weeks.
struct Day {
    week: f64,
    weekday: f64,
    steps: f64,
}

/// Four weeks of made-up step counts, busier at the weekend.
fn days() -> Vec<Day> {
    (0..28)
        .map(|i| {
            let (week, weekday) = (f64::from(i / 7), f64::from(i % 7));
            let steps = 4000.0 + 900.0 * ((i * 5) % 11) as f64 + 1500.0 * f64::from(weekday > 4.5);
            Day {
                week,
                weekday,
                steps,
            }
        })
        .collect()
}

/// The label of the tick at `v` on the weekday axis: no label between days.
fn weekday(v: f64) -> String {
    const NAMES: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    match NAMES.get(v as usize) {
        Some(name) if v.fract() == 0.0 && v >= 0.0 => name.to_string(),
        _ => String::new(),
    }
}

/// The label of the tick at `v` on the week axis, which counts from week 1.
fn week(v: f64) -> String {
    if v.fract() == 0.0 && v >= 0.0 {
        format!("Week {}", v as u32 + 1)
    } else {
        String::new()
    }
}

pub fn mark() -> impl Chart {
    // tour:begin mark
    #[derive(Debug, Mark)]
    #[mark(
        path = "tour_square",
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
            @location(1) half_size: vec2<f32>,
            @location(2) fill: vec4<f32>,
        }

        fn vertex(m: SquareIn, vertex_index: u32, row: u32, view: View) -> Varyings {
            let centre = vec2<f32>(m.x, m.y);
            let half_size = vec2<f32>(m.size * 0.5);
            let q = rect_quad(centre - half_size, centre + half_size, vertex_index, view.dpr);
            var out: Varyings;
            out.clip = px_to_clip(q.position, view);
            out.local = q.local;
            out.half_size = q.half_size;
            out.fill = m.fill;
            return out;
        }

        fn shade(v: Varyings) -> vec4<f32> {
            let alpha = v.fill.a * box_coverage(v.local, v.half_size);
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

    let mut plot = Plot::new();
    plot.title("Steps per day")
        .x_tick_format(weekday)
        .y_tick_format(week);
    plot.add(Selection::<Day, Square>::new(days()))
        .x(|d| d.weekday)
        .y(|d| d.week)
        .size(Px(36.0))
        .fill(Sequential::viridis().encode(|d: &Day| d.steps))
        .legend_title(Square::FILL, "Steps");
    plot
    // tour:end mark
}

fn main() -> gup::Result<()> {
    gup::show(function())
}
