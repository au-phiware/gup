// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU-accelerated graph layout engine.
//!
//! This module provides a [`LayoutEngine`] that runs force-directed layout
//! algorithms entirely on the GPU using WGSL compute shaders.  The engine
//! can position 100K+ node graphs in interactive time without saturating the
//! CPU.
//!
//! # Quick Start
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

mod engine;
mod types;

pub use engine::LayoutEngine;
pub use types::*;
