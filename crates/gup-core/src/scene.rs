// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The resolved scene (RFC-001 §7): data marks, guide rules, flat
//! rectangles, colour-legend gradients and text runs, in logical pixels,
//! drawn by a render target or written by a vector target.

use crate::channel::{Color, Px};
use crate::geom::{Point, Rect};
use crate::render::LayerGpu;
use crate::scale::Ramp;
use crate::scale::sequential::sample_lut;
use std::sync::Arc;

/// Default z-order of backgrounds and grids, below data layers.
pub const Z_GRID: i32 = -100;
/// Default z-order of guides (axes) — above data layers (0…n).
pub const Z_AXES: i32 = 100;
/// Default z-order of the title and legends.
pub const Z_TITLE: i32 = 200;

/// Everything a target needs to draw a chart.
#[derive(Clone, Debug)]
pub struct Scene {
    /// Logical width.
    pub width: f32,
    /// Logical height.
    pub height: f32,
    /// Clear colour.
    pub background: Color,
    /// Items, stable-sorted by `z`.
    pub items: Vec<Item>,
    /// Clip rectangles referenced by [`Item::clip`].
    pub clips: Vec<Rect>,
}

impl Scene {
    /// An empty scene.
    pub fn new(width: f32, height: f32, background: Color) -> Self {
        Self {
            width,
            height,
            background,
            items: Vec::new(),
            clips: Vec::new(),
        }
    }

    /// Add a clip rectangle.
    pub fn add_clip(&mut self, rect: Rect) -> ClipId {
        self.clips.push(rect);
        ClipId(self.clips.len() - 1)
    }

    /// Add an item, keeping items stable-sorted by `z`.
    pub fn push(&mut self, item: Item) {
        let at = self.items.partition_point(|i| i.z <= item.z);
        self.items.insert(at, item);
    }

    /// Every text run, in draw order.
    pub fn text_runs(&self) -> impl Iterator<Item = &TextRun> {
        self.items.iter().flat_map(|i| match &i.kind {
            ItemKind::Text(runs) => runs.as_slice(),
            _ => &[],
        })
    }

    /// A copy of this scene without its data layers: what a vector target
    /// that cannot draw marks yet (RFC-001 S3 `SvgTarget`) can write.
    pub fn guides(&self) -> Scene {
        Scene {
            items: self
                .items
                .iter()
                .filter(|i| !matches!(i.kind, ItemKind::Marks(_)))
                .cloned()
                .collect(),
            width: self.width,
            height: self.height,
            background: self.background,
            clips: self.clips.clone(),
        }
    }

    /// Every rule, in draw order.
    pub fn rules(&self) -> impl Iterator<Item = &Rule> {
        self.items.iter().flat_map(|i| match &i.kind {
            ItemKind::Rules(rules) => rules.as_slice(),
            _ => &[],
        })
    }
}

/// Index of a clip rectangle in [`Scene::clips`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ClipId(pub usize);

/// One drawable item.
#[derive(Clone, Debug)]
pub struct Item {
    /// Draw order (lower first).
    pub z: i32,
    /// Optional clip (scissor) rectangle.
    pub clip: Option<ClipId>,
    /// What to draw.
    pub kind: ItemKind,
}

/// The kinds of item.
#[derive(Clone, Debug)]
pub enum ItemKind {
    /// A data layer's GPU batch.
    Marks(MarkBatch),
    /// Axis lines, ticks, grid lines.
    Rules(Vec<Rule>),
    /// Flat rectangles: the plot background, legend swatches.
    Rects(Vec<RectPrim>),
    /// A colour-legend bar.
    Gradient(GradientBar),
    /// Titles, tick labels.
    Text(Vec<TextRun>),
}

/// A prepared data layer: its pipeline key, column chunks and uniforms.
#[derive(Clone, Debug)]
pub struct MarkBatch {
    pub(crate) gpu: Arc<LayerGpu>,
}

impl MarkBatch {
    /// Number of mark instances, across every chunk.
    pub fn instances(&self) -> u64 {
        self.gpu.instances()
    }

    /// Number of column chunks, each drawn by one instanced draw.
    pub fn chunks(&self) -> usize {
        self.gpu.chunks.len()
    }

    /// The encoding signature (the pipeline cache key).
    pub fn signature(&self) -> &str {
        &self.gpu.program.glue.signature
    }
}

/// A straight line segment.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rule {
    /// Start.
    pub p0: Point,
    /// End.
    pub p1: Point,
    /// Line width.
    pub width: Px,
    /// Colour.
    pub color: Color,
}

impl Rule {
    /// The rectangle the rule covers (for layout metadata).
    pub fn bounds(&self) -> Rect {
        let half = self.width.0 / 2.0;
        Rect::from_edges(
            self.p0.x.min(self.p1.x) - half,
            self.p0.y.min(self.p1.y) - half,
            self.p0.x.max(self.p1.x) + half,
            self.p0.y.max(self.p1.y) + half,
        )
    }
}

/// A flat-coloured rectangle.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RectPrim {
    /// Where, in logical pixels.
    pub rect: Rect,
    /// Fill colour (straight alpha).
    pub color: Color,
}

/// Which way a [`GradientBar`]'s values run.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GradientDirection {
    /// Domain minimum at the left, maximum at the right.
    Horizontal,
    /// Domain minimum at the bottom, maximum at the top.
    Vertical,
}

/// A colour-legend bar for a continuous colour scale's [`Ramp`]
/// ([`Sequential`](crate::Sequential), [`Diverging`](crate::Diverging)):
/// the scale's own palette LUT, sampled as the WGSL that colours the data
/// samples it (linear filtering between texel centres), from the ramp's
/// low end at one end to its high end at the other.
#[derive(Clone, Debug, PartialEq)]
pub struct GradientBar {
    /// Where, in logical pixels.
    pub rect: Rect,
    /// Which way the values run.
    pub direction: GradientDirection,
    /// The palette (shared with the scale, not copied).
    pub(crate) lut: Arc<[[u8; 4]]>,
    /// The scale runs its palette backwards.
    pub(crate) reverse: bool,
}

impl GradientBar {
    /// A legend bar for `ramp` (a scale's [`ramp`](crate::Sequential::ramp)
    /// or the [`Ramp`] of its legend), filling `rect`. A tick at `t` along
    /// the ramp is at `t` along the bar.
    pub fn new(ramp: &Ramp, rect: Rect, direction: GradientDirection) -> Self {
        Self {
            rect,
            direction,
            lut: Arc::clone(&ramp.lut),
            reverse: ramp.reverse,
        }
    }

    /// The colour at normalised position `t` (0 at the ramp's low end, 1
    /// at its high end), interpolated between LUT entries as the GPU's
    /// linear filter does. Vector targets use it to place gradient stops.
    pub fn color_at(&self, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        sample_lut(&self.lut, if self.reverse { 1.0 - t } else { t })
    }
}
// Text anchors come from `gup-text`, which lays the runs out.
pub use gup_text::{Anchor, HAlign, VAlign};

/// Font size and colour.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// Font size.
    pub size: Px,
    /// Colour.
    pub color: Color,
}

/// What a text run is for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextRole {
    /// The chart title.
    Title,
    /// An axis tick label.
    TickLabel,
    /// A legend entry or legend title.
    Legend,
}

/// One line of horizontal text.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    /// The text.
    pub text: Arc<str>,
    /// The anchor point.
    pub at: Point,
    /// How `at` relates to the text.
    pub anchor: Anchor,
    /// Size and colour.
    pub style: TextStyle,
    /// What the text is.
    pub role: TextRole,
}

impl TextRun {
    /// The run as `gup-text` measures and lays it out.
    pub(crate) fn layout_run(&self) -> gup_text::Run<'_> {
        gup_text::Run {
            text: &self.text,
            size: self.style.size.0,
            at: [self.at.x, self.at.y],
            anchor: self.anchor,
        }
    }
}
