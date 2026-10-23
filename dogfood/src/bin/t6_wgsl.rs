// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 6: custom GPU transforms with `#[wgsl_function]` in a downstream
//! crate: a soft-knee radius and a blue-to-orange colour ramp, each applied
//! to a `Circle` channel.

use gup::prelude::*;

/// Map a normalised value to a radius with a soft-knee curve.
#[derive(Clone, Debug)]
struct Knee {
    min_r: f32,
    max_r: f32,
}

#[wgsl_function(wgsl = r#"
    struct Params { min_r: f32, max_r: f32 }

    fn knee_radius(v: f32, p: Params) -> f32 {
        return p.min_r + (p.max_r - p.min_r) * (v * v) / (v * v + 0.1);
    }
"#)]
impl ShaderFn for Knee {
    type Out = Px;

    fn params(&self) -> KneeParams {
        KneeParams {
            min_r: self.min_r,
            max_r: self.max_r,
        }
    }
}

/// Map a value in 0..1 to a colour from blue to orange-red.
#[derive(Clone, Debug)]
struct Ramp;

#[wgsl_function(wgsl = r#"
    struct Params { pad: f32 }

    fn ramp(v: f32, p: Params) -> vec4<f32> {
        return vec4<f32>(v, 0.3, 1.0 - v, 1.0);
    }
"#)]
impl ShaderFn for Ramp {
    fn params(&self) -> RampParams {
        RampParams { pad: 0.0 }
    }
}

#[derive(Debug, Clone)]
struct Reading {
    x: f64,
    y: f64,
    v: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let data: Vec<Reading> = (0..400)
        .map(|i| {
            let (gx, gy) = ((i % 20) as f64 / 19.0, (i / 20) as f64 / 19.0);
            Reading {
                x: gx * 1.8 - 0.9,
                y: gy * 1.8 - 0.9,
                v: gx,
            }
        })
        .collect();

    let cx = Context::new_blocking()?;
    let mut plot = Plot::new();
    plot.x(Linear::new().domain(-1.0, 1.0));
    plot.y(Linear::new().domain(-1.0, 1.0));
    plot.circles(data)
        .x(|r: &Reading| r.x)
        .y(|r: &Reading| r.y)
        .attr(
            Circle::RADIUS,
            Knee {
                min_r: 1.5,
                max_r: 12.0,
            }
            .encode(|r: &Reading| r.v),
        )
        .attr(Circle::FILL, Ramp.encode(|r: &Reading| r.v));
    plot.save_png(&cx, "/tmp/gup-dogfood/t6_wgsl.png", 600, 600)?;
    println!("wrote /tmp/gup-dogfood/t6_wgsl.png");
    Ok(())
}
