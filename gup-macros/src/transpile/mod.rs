// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rust-to-WGSL transpilation prototype.
//!
//! This module provides a modular pipeline for converting Rust syntax
//! (parsed by `syn`) into WGSL shader code. It is designed as a
//! proof-of-concept for the Phase 2 transpilation initiative.
//!
//! ## Architecture
//!
//! The transpilation pipeline has three phases:
//!
//! 1. **Parse**: `syn` parses Rust source into its AST (`syn::Expr`, etc.)
//! 2. **Convert**: [`RustToWgsl`] converts `syn` AST nodes into lightweight
//!    [`WgslAst`] nodes defined in this module
//! 3. **Generate**: [`WgslCodeGen`] emits WGSL source text from the AST
//!
//! ## Supported Rust Subset
//!
//! - Arithmetic expressions (`+`, `-`, `*`, `/`, `%`)
//! - Comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`)
//! - Logical operators (`&&`, `||`, `!`)
//! - Variable references and field access
//! - `let` bindings (immutable and mutable)
//! - `return` statements
//! - `if`/`else` expressions
//! - Function calls (with Rust→WGSL name mapping)
//! - Method calls on known types (`.abs()`, `.sqrt()`, etc.)
//! - Numeric literals (f32, i32, u32)
//! - Type casts (`as f32`, etc.)
//! - Unary negation and logical not

// This module is a prototype used by tests; suppress dead-code warnings.
// #![allow(...)] only works at the crate root, so we allow on the items inside.
#[allow(dead_code, unused_imports)]
mod ast;
#[allow(dead_code, unused_imports)]
mod codegen;
#[allow(dead_code, unused_imports)]
mod convert;
mod pipeline_tests;
#[allow(dead_code, unused_imports)]
pub mod type_map;

// Re-exports are used by pipeline_tests and future integration.
#[allow(unused_imports)]
pub use ast::*;
#[allow(unused_imports)]
pub use codegen::WgslCodeGen;
#[allow(unused_imports)]
pub use convert::{RustToWgsl, TranspileError};
#[allow(unused_imports)]
pub use type_map::{TypeMapper, TypeMappingError, TypeMappingErrorKind, WgslTypeInfo};
