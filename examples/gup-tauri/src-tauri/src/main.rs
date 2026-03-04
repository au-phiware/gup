// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tauri backend for the gup-tauri example.
//!
//! This application exposes two IPC commands that the WebView frontend
//! calls via `invoke()`:
//!
//! - `get_scatter_data` — returns a JSON array of `{x, y}` points.
//! - `get_scatter_data_randomised` — returns a fresh random dataset so the
//!   chart can demonstrate live updates without a page reload.

use serde::Serialize;

/// A single scatter data point returned over the IPC bridge.
#[derive(Debug, Clone, Serialize)]
struct ScatterPoint {
    x: f32,
    y: f32,
}

/// Return a deterministic scatter dataset.
///
/// The data resembles a noisy linear trend, suitable for a demo scatter plot.
#[tauri::command]
fn get_scatter_data() -> Vec<ScatterPoint> {
    // 30 points with a linear trend + periodic variation.
    (0..30)
        .map(|i| {
            let t = i as f32 / 29.0;
            ScatterPoint {
                x: t * 100.0,
                y: 20.0 + 60.0 * t + 15.0 * (t * 6.28).sin(),
            }
        })
        .collect()
}

/// Return a randomised scatter dataset.
///
/// Each call produces different data so the frontend can demonstrate
/// live chart updates via the IPC bridge.
#[tauri::command]
fn get_scatter_data_randomised() -> Vec<ScatterPoint> {
    use rand::Rng;
    let mut rng = rand::rng();
    (0..30)
        .map(|i| {
            let t = i as f32 / 29.0;
            ScatterPoint {
                x: t * 100.0 + rng.random_range(-5.0f32..5.0),
                y: 20.0
                    + 60.0 * t
                    + 15.0 * (t * 6.28).sin()
                    + rng.random_range(-10.0f32..10.0),
            }
        })
        .collect()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_scatter_data,
            get_scatter_data_randomised,
        ])
        .run(tauri::generate_context!())
        .expect("error while running gup-tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_scatter_data_returns_30_points() {
        let data = get_scatter_data();
        assert_eq!(data.len(), 30);
    }

    #[test]
    fn test_scatter_data_x_range() {
        let data = get_scatter_data();
        let x_min = data.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let x_max = data
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(x_min >= 0.0);
        assert!(x_max <= 100.0);
    }

    #[test]
    fn test_scatter_data_serialises_to_json() {
        let data = get_scatter_data();
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"x\""));
        assert!(json.contains("\"y\""));
    }

    #[test]
    fn test_randomised_data_differs() {
        let a = get_scatter_data_randomised();
        let b = get_scatter_data_randomised();
        // With 30 random points, it is astronomically unlikely that
        // two calls produce identical y values.
        let same = a
            .iter()
            .zip(b.iter())
            .all(|(pa, pb)| (pa.y - pb.y).abs() < f32::EPSILON);
        assert!(!same, "randomised data should differ between calls");
    }
}
