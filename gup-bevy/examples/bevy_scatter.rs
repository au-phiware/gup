// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Bevy Scatter Plot Demo
//!
//! Demonstrates embedding a Gup scatter plot inside a Bevy application.
//! The plot shows an animated sine wave that updates every frame.
//!
//! Run with: `cargo run -p gup-bevy --example bevy_scatter`

use bevy::prelude::*;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::scatter;
use gup::chart_builder::builders::AccessorFunction;
use gup::chart_builder::ChartBuilder;
use gup::render::RenderContext;
use gup_bevy::prelude::*;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// A simple 2-D point used as chart data.
#[derive(Debug, Clone)]
struct DataPoint {
    x: f32,
    y: f32,
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Startup system: spawn a camera and a GupChart entity.
fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // 2-D camera so the sprite is visible.
    commands.spawn(Camera2d);

    // Build the initial scatter plot.
    let chart = build_scatter_chart(0.0);

    // Create a blank placeholder image for the sprite.
    let placeholder = blank_chart_image(800, 600);
    let image_handle = images.add(placeholder);

    // Spawn the entity: GupChart + Sprite.
    commands.spawn((
        GupChart::new(chart).with_size(800, 600),
        Sprite {
            image: image_handle,
            custom_size: Some(Vec2::new(800.0, 600.0)),
            ..default()
        },
    ));
}

/// Per-frame system: rebuild the scatter data with a time offset so the sine
/// wave animates.
fn animate_chart(time: Res<Time>, mut charts: Query<&mut GupChart>) {
    let t = time.elapsed_secs();

    for mut gup_chart in &mut charts {
        // Replace the inner chart with fresh data.
        let new_chart = build_scatter_chart(t);
        *gup_chart = GupChart::new(new_chart).with_size(800, 600);
    }
}

// ---------------------------------------------------------------------------
// Chart builder helper
// ---------------------------------------------------------------------------

/// Build a scatter chart whose data is a sine wave offset by `time`.
fn build_scatter_chart(
    time: f32,
) -> gup::chart_builder::ComposedChart<DataPoint, gup::mark::Circle> {
    let data: Vec<DataPoint> = (0..60)
        .map(|i| {
            let x = i as f32 / 60.0 * 10.0;
            let y = (x + time).sin() * 3.0 + 5.0;
            DataPoint { x, y }
        })
        .collect();

    let context = Arc::new(
        pollster::block_on(RenderContext::new()).expect("Failed to create RenderContext"),
    );

    let x_acc = AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.x));
    let y_acc = AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.y));

    scatter()
        .x(x_acc)
        .y(y_acc)
        .point_size(6.0)
        .fill_color([0.2, 0.5, 0.9, 1.0])
        .build_with_data(data, context)
        .expect("Failed to build scatter chart")
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Gup × Bevy — Animated Scatter Plot".into(),
                resolution: (900u32, 700u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(GupPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, animate_chart)
        .run();
}
