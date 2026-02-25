// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! AST-based WGSL shader composition system.
//!
//! This module provides a proper Abstract Syntax Tree (AST) for WGSL,
//! enabling type-safe shader function composition with compile-time
//! validation and advanced optimizations.
//!
//! ## Architecture
//!
//! - [`types`] — AST node types (`WgslType`, `Function`, `Expr`, …)
//! - [`parser`] — WGSL text → AST
//! - [`generator`] — AST → WGSL text
//! - [`type_check`] — Type compatibility validation with helpful errors
//! - [`optimizer`] — Dead code elimination, constant folding, function inlining
//!
//! ## Usage
//!
//! ```rust,ignore
//! use gup::shader_ast::{parse_wgsl, generate_wgsl, optimize, AstOptimizationConfig};
//!
//! // Parse WGSL text to AST
//! let mut module = parse_wgsl("fn add(x: f32) -> f32 { return x + 1.0; }").unwrap();
//!
//! // Optimize
//! optimize(&mut module, &AstOptimizationConfig::default());
//!
//! // Generate optimized WGSL
//! let wgsl = generate_wgsl(&module);
//! ```

pub mod generator;
pub mod optimizer;
pub mod parser;
pub mod type_check;
pub mod types;

// Re-export key types for convenience.
pub use generator::{GeneratorConfig, WgslGenerator, generate_wgsl, generate_wgsl_minimal};
pub use optimizer::{
    AstOptimizationConfig, OptimizationResult, constant_folding, dead_code_elimination,
    function_inlining, optimize,
};
pub use parser::{ParseError, WgslParser, parse_wgsl};
pub use type_check::{CompositionError, FunctionSignature, TypeChecker, TypeError};
pub use types::*;
