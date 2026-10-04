// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Task 6: a custom GPU transform with #[wgsl_function] in a downstream
//! crate, wired through `attr_shader`, following Tutorial 3 verbatim.

use gup::prelude::*;
use gup::proc_macros::wgsl_function;
#[allow(unused_imports)]
use gup::*; // GUP-377 workaround: makes `crate::shader_function` resolve
use std::sync::Arc;

/// Map a normalised value to a radius with a soft-knee curve.
#[wgsl_function]
fn knee_radius(v: f32, min_r: f32, max_r: f32) -> f32 {
    return min_r + (max_r - min_r) * (v * v) / (v * v + 0.1);
}

/// Second workaround: the macro's `wgsl_function()` string embeds the
/// uniforms struct and Selection's shader assembly emits it again, so wgpu
/// panics with "redefinition of `KneeRadiusUniforms`". Wrap and strip it.
#[derive(Debug, Clone)]
struct KneeFixed(KneeRadius);

impl gup::shader_function::core::ComposableShaderFunction for KneeFixed {
    type Input = f32;
    type Output = f32;
    type Uniforms = KneeRadiusUniforms;
    fn wgsl_function() -> &'static str {
        static S: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        S.get_or_init(|| {
            let src = KneeRadius::wgsl_function();
            src[src.find("fn ").unwrap()..].to_string()
        })
    }
    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        self.0.create_uniforms()
    }
    fn function_name() -> &'static str {
        KneeRadius::function_name()
    }
}

#[derive(Debug, Clone)]
struct Reading {
    x: f32,
    y: f32,
    v: f32,
}

#[tokio::main]
async fn main() -> GupResult<()> {
    env_logger::init();
    let data: Vec<Reading> = (0..400)
        .map(|i| {
            let (gx, gy) = ((i % 20) as f32 / 19.0, (i / 20) as f32 / 19.0);
            Reading { x: gx * 1.8 - 0.9, y: gy * 1.8 - 0.9, v: gx }
        })
        .collect();

    println!("generated WGSL:\n{}", KneeRadius::wgsl_function());

    let context = Arc::new(RenderContext::new().await?);
    let mut sel = Selection::<Reading, Circle>::new(data, context.clone())?;
    sel.attr("center", |d: &Reading| [d.x, d.y])
        .attr("fill_color", |d: &Reading| [d.v, 0.3, 1.0 - d.v, 1.0])
        .attr("stroke_width", |_: &Reading| 0.0f32)
        ;
    if std::env::var("BUILTIN").is_ok() {
        sel.attr_shader("radius", |d: &Reading| d.v, LinearScale::new(0.0, 1.0, 0.005, 0.04));
    } else if std::env::var("RAW_MACRO").is_ok() {
        // Panics at prepare time: duplicate uniforms struct in the shader.
        sel.attr_shader("radius", |d: &Reading| d.v, KneeRadius::new(0.005, 0.04));
    } else {
        sel.attr_shader("radius", |d: &Reading| d.v, KneeFixed(KneeRadius::new(0.005, 0.04)));
    }
    sel.prepare_render_bound(context.device(), context.queue(), None, None)?;

    // Render it via a ComposedChart so we can use export_png.
    let config = gup::chart_builder::ChartConfig { show_axes: false, ..Default::default() };
    let mut chart = gup::chart_builder::ComposedChart::new(sel, config);
    chart.export_png("/tmp/gup-dogfood/t6_wgsl.png", 600, 600)?;
    println!("wrote /tmp/gup-dogfood/t6_wgsl.png");
    Ok(())
}
