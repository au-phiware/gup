// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU-accelerated layout engines.
//!
//! This module provides a [`LayoutEngine`] for graph and treemap layouts.
//!
//! ## Force-directed graph layout
//!
//! The engine runs force-directed algorithms entirely on the GPU using WGSL
//! compute shaders, positioning 100K+ node graphs in interactive time.
//!
//! ```rust,no_run
//! use gup::layout::{LayoutEngine, ForceDirected, GraphLayout, LayoutNode, LayoutEdge};
//! use gup::render::RenderContext;
//!
//! # async fn example() -> gup::error::GupResult<()> {
//! let ctx = RenderContext::new().await?;
//! let engine = LayoutEngine::new(&ctx)?;
//!
//! let nodes: Vec<LayoutNode> = (0..100)
//!     .map(|i| LayoutNode { id: i as u32, x: 0.0, y: 0.0 })
//!     .collect();
//! let edges = vec![
//!     LayoutEdge { source: 0, target: 1 },
//!     LayoutEdge { source: 1, target: 2 },
//! ];
//!
//! let config = ForceDirected::new();
//! let result = engine.force_directed_layout(&nodes, &edges, &config).await?;
//!
//! for pos in &result.positions {
//!     println!("Node {} -> ({}, {})", pos.id, pos.x, pos.y);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Treemap layout
//!
//! Hierarchical treemap layouts subdivide a rectangle so that each cell's
//! area is proportional to its value.  Four algorithms are supported:
//! Squarified, Binary, Strip and SliceDice.
//!
//! ```rust,no_run
//! use gup::layout::{LayoutEngine, TreeNode, TreemapOptions, LayoutRect};
//! use gup::render::RenderContext;
//!
//! # async fn example() -> gup::error::GupResult<()> {
//! let ctx = RenderContext::new().await?;
//! let engine = LayoutEngine::new(&ctx)?;
//!
//! let nodes = vec![
//!     TreeNode { parent: None, child_start: 1, child_count: 2 },
//!     TreeNode { parent: Some(0), child_start: 3, child_count: 0 },
//!     TreeNode { parent: Some(0), child_start: 3, child_count: 0 },
//! ];
//! let values = vec![0.0, 3.0, 1.0];
//! let viewport = LayoutRect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
//!
//! let result = engine.treemap_layout(&nodes, &values, viewport, &TreemapOptions::default()).await?;
//! for cell in result.cells() {
//!     println!("Node {} at ({}, {}) size {}×{}", cell.node_index, cell.x, cell.y, cell.width, cell.height);
//! }
//! # Ok(())
//! # }
//! ```

mod engine;
mod graph_builder;
pub(crate) mod quadtree;
pub mod treemap;
mod types;

pub use engine::LayoutEngine;
pub use graph_builder::GraphChartBuilder;
pub use treemap::{
    LayoutRect, TreeNode, TreemapAlgorithm, TreemapCell, TreemapOptions, TreemapResult,
};
pub use types::*;
