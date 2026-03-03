// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! High-level graph chart builder that integrates with [`LayoutEngine`].
//!
//! Provides a fluent API for building force-directed graph visualizations
//! on top of the GPU layout engine.
//!
//! # Examples
//!
//! ```rust,no_run
//! use gup::layout::{ForceDirected, GraphChartBuilder, LayoutNode, LayoutEdge};
//! use gup::render::RenderContext;
//!
//! # async fn example() -> gup::error::GupResult<()> {
//! let ctx = RenderContext::new().await?;
//! let builder = GraphChartBuilder::new(&ctx)?
//!     .graph_layout(ForceDirected::new().iterations(200));
//! # Ok(())
//! # }
//! ```

use super::engine::LayoutEngine;
use super::types::*;
use crate::error::GupResult;
use crate::render::RenderContext;

/// A high-level builder for graph-based visualizations.
///
/// Wraps [`LayoutEngine`] and provides a fluent API for configuring
/// and running force-directed layouts.
#[derive(Debug)]
pub struct GraphChartBuilder {
    engine: LayoutEngine,
    layout_config: Option<ForceDirected>,
    nodes: Vec<LayoutNode>,
    edges: Vec<LayoutEdge>,
}

impl GraphChartBuilder {
    /// Create a new graph chart builder, compiling GPU shaders.
    pub fn new(context: &RenderContext) -> GupResult<Self> {
        Ok(Self {
            engine: LayoutEngine::new(context)?,
            layout_config: None,
            nodes: Vec::new(),
            edges: Vec::new(),
        })
    }

    /// Configure the layout algorithm.
    ///
    /// Accepts a [`ForceDirected`] configuration.
    pub fn graph_layout(mut self, layout: ForceDirected) -> Self {
        self.layout_config = Some(layout);
        self
    }

    /// Set the nodes.
    pub fn nodes(mut self, nodes: Vec<LayoutNode>) -> Self {
        self.nodes = nodes;
        self
    }

    /// Set the edges.
    pub fn edges(mut self, edges: Vec<LayoutEdge>) -> Self {
        self.edges = edges;
        self
    }

    /// Run the layout and return the result.
    pub async fn build(self) -> GupResult<LayoutResult> {
        let config = self.layout_config.unwrap_or_default();
        self.engine
            .force_directed_layout(&self.nodes, &self.edges, &config)
            .await
    }
}
