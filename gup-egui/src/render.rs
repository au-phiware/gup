// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The widget's render path, free of egui types so tests can drive it.

use gup_core::{Chart, Context, Renderer, Result, TextureTarget};

/// Resolve `chart` at `size_pts` (logical pixels, egui points) and draw it
/// into `target`, whose dpr is the pixels per point (Gup's own pass and
/// submit).
///
/// A clean chart re-resolves to the same scene, so calling this on every
/// frame costs one draw and no uploads.
pub(crate) fn render_into(
    cx: &Context,
    renderer: &mut Renderer,
    chart: &mut dyn Chart,
    size_pts: (f32, f32),
    target: &mut TextureTarget,
) -> Result<()> {
    let resolved = chart.resolve(cx, size_pts.0, size_pts.1)?;
    renderer.render(cx, &resolved.scene, target)
}
