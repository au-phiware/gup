// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The resolved scene (RFC-001 §7, S0a subset): data marks, guide rules
//! and text runs, in logical pixels, drawn by a render target. Rects,
//! gradients (legends) and vector targets are RFC-001 S3.

use crate::channel::{Color, Px};
use crate::geom::{Point, Rect};
use crate::render::LayerGpu;
use std::sync::Arc;

/// Default z-order of guides (axes) — above data layers (0…n).
pub const Z_AXES: i32 = 100;
/// Default z-order of the title and legends.
pub const Z_TITLE: i32 = 200;

/// Everything a target needs to draw a chart.
#[derive(Debug)]
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
#[derive(Debug)]
pub struct Item {
    /// Draw order (lower first).
    pub z: i32,
    /// Optional clip (scissor) rectangle.
    pub clip: Option<ClipId>,
    /// What to draw.
    pub kind: ItemKind,
}

/// The kinds of item.
#[derive(Debug)]
pub enum ItemKind {
    /// A data layer's GPU batch.
    Marks(MarkBatch),
    /// Axis lines, ticks, grid lines.
    Rules(Vec<Rule>),
    /// Titles, tick labels.
    Text(Vec<TextRun>),
}

/// A prepared data layer: its pipeline key, column chunk and uniforms.
#[derive(Clone, Debug)]
pub struct MarkBatch {
    pub(crate) gpu: Arc<LayerGpu>,
}

impl MarkBatch {
    /// Number of mark instances.
    pub fn instances(&self) -> u32 {
        self.gpu.instances
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
