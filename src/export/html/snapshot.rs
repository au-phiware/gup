// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Serialisable chart-definition snapshot.
//!
//! [`ChartSnapshot`] captures the configuration fields of a
//! [`ChartConfig`](crate::chart_builder::ChartConfig) in a form that can be
//! round-tripped through JSON without touching GPU resource handles.

use serde::{Deserialize, Serialize};

/// A serialisable snapshot of a chart's definition.
///
/// This DTO captures the subset of [`ChartConfig`](crate::chart_builder::ChartConfig) fields that are
/// meaningful for reconstructing a chart from embedded data — dimensions,
/// margins, title, scales, and visual toggles.  GPU-only state (pipeline
/// caches, text atlases, etc.) is intentionally excluded.
///
/// # Round-trip guarantee
///
/// ```
/// use gup::export::html::ChartSnapshot;
///
/// let snapshot = ChartSnapshot {
///     title: Some("My Chart".into()),
///     subtitle: None,
///     width: 800.0,
///     height: 600.0,
///     margins: gup::export::html::SnapshotMargins {
///         top: 60.0,
///         right: 40.0,
///         bottom: 60.0,
///         left: 60.0,
///     },
///     background_color: Some([1.0, 1.0, 1.0, 1.0]),
///     show_axes: true,
///     show_grid: true,
/// };
///
/// let json = serde_json::to_string(&snapshot).unwrap();
/// let recovered: ChartSnapshot = serde_json::from_str(&json).unwrap();
/// assert_eq!(recovered.title.as_deref(), Some("My Chart"));
/// assert_eq!(recovered.width, 800.0);
/// assert_eq!(recovered.show_grid, true);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartSnapshot {
    /// Chart title (from [`TitleConfig::text`](crate::chart_builder::TitleConfig::text)).
    pub title: Option<String>,

    /// Chart subtitle (from [`TitleConfig::subtitle`](crate::chart_builder::TitleConfig::subtitle)).
    pub subtitle: Option<String>,

    /// Chart width in logical pixels.
    pub width: f32,

    /// Chart height in logical pixels.
    pub height: f32,

    /// Chart margins.
    pub margins: SnapshotMargins,

    /// Background colour as `[R, G, B, A]` in `0.0..=1.0`.
    pub background_color: Option<[f32; 4]>,

    /// Whether axes are visible.
    pub show_axes: bool,

    /// Whether grid lines are visible.
    pub show_grid: bool,
}

/// Serialisable copy of [`Margins`](crate::chart_builder::Margins).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SnapshotMargins {
    /// Top margin in logical pixels.
    pub top: f32,
    /// Right margin in logical pixels.
    pub right: f32,
    /// Bottom margin in logical pixels.
    pub bottom: f32,
    /// Left margin in logical pixels.
    pub left: f32,
}

impl ChartSnapshot {
    /// Build a snapshot from the live [`ChartConfig`](crate::chart_builder::ChartConfig).
    pub fn from_config(config: &crate::chart_builder::ChartConfig) -> Self {
        Self {
            title: config.title_config.as_ref().map(|t| t.text.clone()),
            subtitle: config
                .title_config
                .as_ref()
                .and_then(|t| t.subtitle.clone()),
            width: config.width,
            height: config.height,
            margins: SnapshotMargins {
                top: config.margins.top,
                right: config.margins.right,
                bottom: config.margins.bottom,
                left: config.margins.left,
            },
            background_color: config.background_color,
            show_axes: config.show_axes,
            show_grid: config.show_grid,
        }
    }
}

/// A self-contained chart bundle that pairs the configuration snapshot with
/// the data points from the chart's [`Selection`](crate::selection::Selection).
///
/// When `T: Serialize`, the [`HtmlExporter`](super::HtmlExporter) can produce a
/// `ChartBundle` containing both the chart configuration and the bound data,
/// making the HTML export fully self-contained — the embedded WASM module can
/// reconstruct the entire chart from the JSON alone.
///
/// When data is absent (because `T` does not implement `Serialize` or the
/// caller used the data-free export path), the `data` field is `None` and
/// the JSON omits it entirely.
///
/// # Round-trip guarantee
///
/// ```
/// use gup::export::html::{ChartBundle, ChartSnapshot, SnapshotMargins};
///
/// let bundle = ChartBundle {
///     config: ChartSnapshot {
///         title: Some("Bundled".into()),
///         subtitle: None,
///         width: 800.0,
///         height: 600.0,
///         margins: SnapshotMargins {
///             top: 60.0, right: 40.0, bottom: 60.0, left: 60.0,
///         },
///         background_color: None,
///         show_axes: true,
///         show_grid: false,
///     },
///     data: Some(vec![
///         serde_json::json!({"x": 1, "y": 2}),
///         serde_json::json!({"x": 3, "y": 4}),
///     ]),
/// };
///
/// let json = serde_json::to_string(&bundle).unwrap();
/// let recovered: ChartBundle = serde_json::from_str(&json).unwrap();
/// assert_eq!(recovered.config.title.as_deref(), Some("Bundled"));
/// assert_eq!(recovered.data.unwrap().len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartBundle {
    /// The chart configuration snapshot.
    pub config: ChartSnapshot,

    /// The serialised data points, if available.
    ///
    /// Each element corresponds to one `T` instance from the chart's
    /// [`Selection`](crate::selection::Selection), serialised as a
    /// [`serde_json::Value`].  When `T` does not implement `Serialize`,
    /// or the caller uses the data-free export path, this field is `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<serde_json::Value>>,
}

impl ChartBundle {
    /// Create a config-only bundle with no data.
    pub fn config_only(config: ChartSnapshot) -> Self {
        Self { config, data: None }
    }

    /// Create a bundle with config and data.
    pub fn with_data(config: ChartSnapshot, data: Vec<serde_json::Value>) -> Self {
        Self {
            config,
            data: Some(data),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_json() {
        let snapshot = ChartSnapshot {
            title: Some("Sales Dashboard".into()),
            subtitle: Some("Q4 2024".into()),
            width: 1024.0,
            height: 768.0,
            margins: SnapshotMargins {
                top: 40.0,
                right: 30.0,
                bottom: 50.0,
                left: 60.0,
            },
            background_color: Some([0.95, 0.95, 0.95, 1.0]),
            show_axes: true,
            show_grid: false,
        };

        let json = serde_json::to_string_pretty(&snapshot).unwrap();
        let recovered: ChartSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(snapshot, recovered);
    }

    #[test]
    fn round_trip_minimal() {
        let snapshot = ChartSnapshot {
            title: None,
            subtitle: None,
            width: 400.0,
            height: 300.0,
            margins: SnapshotMargins {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            },
            background_color: None,
            show_axes: false,
            show_grid: false,
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let recovered: ChartSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(snapshot, recovered);
    }

    #[test]
    fn from_config_extracts_fields() {
        use crate::chart_builder::{ChartConfig, Margins, TitleConfig};

        let config = ChartConfig {
            title_config: Some(TitleConfig::new("Test Title").with_subtitle("Test Subtitle")),
            width: 640.0,
            height: 480.0,
            margins: Margins {
                top: 10.0,
                right: 20.0,
                bottom: 30.0,
                left: 40.0,
            },
            background_color: Some([1.0, 0.0, 0.0, 1.0]),
            show_axes: false,
            show_grid: true,
            ..ChartConfig::default()
        };

        let snapshot = ChartSnapshot::from_config(&config);

        assert_eq!(snapshot.title.as_deref(), Some("Test Title"));
        assert_eq!(snapshot.subtitle.as_deref(), Some("Test Subtitle"));
        assert_eq!(snapshot.width, 640.0);
        assert_eq!(snapshot.height, 480.0);
        assert_eq!(snapshot.margins.left, 40.0);
        assert!(snapshot.background_color.is_some());
        assert!(!snapshot.show_axes);
        assert!(snapshot.show_grid);
    }

    /// Verify that the embedded JSON from an HTML document can be parsed
    /// back into a `ChartSnapshot`.
    #[test]
    fn parse_embedded_json() {
        let snapshot = ChartSnapshot {
            title: Some("Embedded Test".into()),
            subtitle: None,
            width: 800.0,
            height: 600.0,
            margins: SnapshotMargins {
                top: 60.0,
                right: 40.0,
                bottom: 60.0,
                left: 60.0,
            },
            background_color: Some([1.0, 1.0, 1.0, 1.0]),
            show_axes: true,
            show_grid: true,
        };

        let json = serde_json::to_string_pretty(&snapshot).unwrap();

        // Simulate extracting the JSON from an HTML document.
        let html =
            format!(r#"<script type="application/json" id="gup-chart-data">{json}</script>"#);

        let start = html.find('>').unwrap() + 1;
        let end = html.rfind("</script>").unwrap();
        let extracted = &html[start..end];

        let recovered: ChartSnapshot = serde_json::from_str(extracted).unwrap();
        assert_eq!(snapshot, recovered);
    }

    // -- ChartBundle tests -------------------------------------------------

    #[test]
    fn bundle_round_trip_with_data() {
        let bundle = ChartBundle::with_data(
            ChartSnapshot {
                title: Some("Bundle RT".into()),
                subtitle: None,
                width: 640.0,
                height: 480.0,
                margins: SnapshotMargins {
                    top: 10.0,
                    right: 10.0,
                    bottom: 10.0,
                    left: 10.0,
                },
                background_color: None,
                show_axes: true,
                show_grid: false,
            },
            vec![
                serde_json::json!({"x": 1.0, "y": 2.0}),
                serde_json::json!({"x": 3.0, "y": 4.0}),
            ],
        );

        let json = serde_json::to_string_pretty(&bundle).unwrap();
        let recovered: ChartBundle = serde_json::from_str(&json).unwrap();

        assert_eq!(bundle, recovered);
        assert_eq!(recovered.data.as_ref().unwrap().len(), 2);
        assert_eq!(recovered.config.title.as_deref(), Some("Bundle RT"));
    }

    #[test]
    fn bundle_config_only_omits_data() {
        let bundle = ChartBundle::config_only(ChartSnapshot {
            title: None,
            subtitle: None,
            width: 400.0,
            height: 300.0,
            margins: SnapshotMargins {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            },
            background_color: None,
            show_axes: false,
            show_grid: false,
        });

        let json = serde_json::to_string(&bundle).unwrap();

        // The "data" key should not appear in the JSON at all.
        assert!(
            !json.contains("\"data\""),
            "config-only bundle should omit the data field"
        );

        let recovered: ChartBundle = serde_json::from_str(&json).unwrap();
        assert!(recovered.data.is_none());
    }

    #[test]
    fn bundle_backward_compat_from_snapshot_json() {
        // A JSON blob produced by the old ChartSnapshot-only path should
        // still be parseable *as a ChartSnapshot* (i.e. the ChartBundle
        // wrapper is a strict superset).
        let snapshot = ChartSnapshot {
            title: Some("Legacy".into()),
            subtitle: None,
            width: 800.0,
            height: 600.0,
            margins: SnapshotMargins {
                top: 60.0,
                right: 40.0,
                bottom: 60.0,
                left: 60.0,
            },
            background_color: None,
            show_axes: true,
            show_grid: true,
        };

        let json = serde_json::to_string(&snapshot).unwrap();

        // The old format does not have a "config" wrapper, so it should NOT
        // parse as a ChartBundle — this is expected.  The consumer can
        // attempt ChartBundle first, fall back to ChartSnapshot.
        let as_snapshot: ChartSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(as_snapshot, snapshot);
    }
}
