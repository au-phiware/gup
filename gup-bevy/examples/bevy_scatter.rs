// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A scatter on a sprite, extended by an `Appender` every frame.
//! `GUP_SCREENSHOT_PATH=/tmp/x.png` saves a Bevy screenshot after 30 frames
//! and exits; `tests/screenshot_parity.rs` rebuilds the chart.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use gup_bevy::prelude::*;
use gup_core::prelude::*;
use gup_core::{Appender, ScatterChart};

const SIZE: UVec2 = UVec2::new(480, 320);

/// The chart before any rows are appended (also built by the parity test).
fn scatter_chart() -> ScatterChart<(f64, f64)> {
    let pts: Vec<(f64, f64)> = (0..20)
        .map(|i| (i as f64 * 0.5, (i * 7 % 10) as f64 / 2.0))
        .collect();
    gup_core::scatter(pts)
        .x(|p| p.0)
        .x_scale(Linear::new().domain(0.0, 10.0))
        .y(|p| p.1)
        .y_scale(Linear::new().domain(0.0, 10.0))
        .fill(Color::hex(0x1f77b4))
        .radius(5.0)
        .title("Bevy scatter")
}

#[derive(Resource)]
struct Feed(Appender<(f64, f64)>, u32);

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);
    let image = images.add(chart_image(SIZE, 1.0));
    let mut chart = scatter_chart();
    commands.insert_resource(Feed(chart.appender(), 0));
    commands.spawn((
        GupChart::new(chart, SIZE, image.clone()),
        Sprite::from_image(image),
    ));
}

fn append(mut feed: ResMut<Feed>, mut commands: Commands) {
    feed.1 += 1;
    let x = f64::from(feed.1 % 200) * 0.05;
    feed.0.push((x, 9.0 + (x * 3.0).sin() * 0.5)).ok();
    if feed.1 == 30
        && let Ok(path) = std::env::var("GUP_SCREENSHOT_PATH")
    {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if feed.1 == 60 && std::env::var("GUP_SCREENSHOT_PATH").is_ok() {
        commands.write_message(AppExit::Success);
    }
}

fn main() {
    let window = Window {
        resolution: (640, 480).into(),
        title: "Gup in Bevy".into(),
        ..default()
    };
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(window),
                ..default()
            }),
            GupPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, append)
        .run();
}
