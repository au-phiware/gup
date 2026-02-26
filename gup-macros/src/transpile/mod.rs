// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rust-to-WGSL transpilation pipeline.
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
//!    [`WgslAst`] nodes defined in this module, using [`TypeMapper`] for
//!    comprehensive type mapping with memory layout tracking
//! 3. **Generate**: [`WgslCodeGen`] emits WGSL source text from the AST
//!
//! ## Type System Mapping
//!
//! The [`type_map`] module provides the [`TypeMapper`] which handles:
//!
//! - **Primitive types**: `f32`, `i32`, `u32`, `bool`
//! - **Vector types**: `Vec2-4`, `IVec2-4`, `UVec2-4`, `BVec2-4`
//! - **Matrix types**: `Mat2-4`, `Mat{C}x{R}` (all non-square variants)
//! - **Array types**: `[T; N]` → `array<T, N>`
//! - **Struct types**: Custom struct registration with field mapping
//! - **Memory layout**: Size and alignment per WGSL specification
//! - **Error diagnostics**: Clear messages with suggestions for fixes
//!
//! ## Supported Rust Subset
//!
//! ### Operators
//!
//! - Arithmetic expressions (`+`, `-`, `*`, `/`, `%`)
//! - Comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`)
//! - Logical operators (`&&`, `||`, `!`)
//! - Bitwise operators (`&`, `|`, `^`, `<<`, `>>`)
//! - Compound assignments (`+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, etc.)
//! - Unary negation and logical not
//!
//! ### Variables and Statements
//!
//! - Variable references and field access
//! - `let` bindings (immutable and mutable)
//! - `return` statements
//! - `if`/`else`/`else if` statements and expressions (→ WGSL `select()`)
//! - `for i in 0..n` loops (→ C-style `for` in WGSL)
//! - `while condition` loops
//! - `loop` (infinite loop with explicit `break`)
//! - `break` and `continue` statements
//! - Assignment statements
//! - Array/vector indexing
//! - Type-annotated let bindings
//!
//! ### Functions and Calls
//!
//! - Function calls (with Rust→WGSL name mapping)
//! - Method calls on known types (`.abs()`, `.sqrt()`, `.length()`, etc.)
//! - Qualified path calls (`f32::sin(x)`, `Vec3::new(...)`)
//! - Vector/matrix constructor calls
//! - Static methods (`Vec3::splat(v)`, `Vec3::zero()`)
//! - Conversion methods (`.to_f32()`, `.to_i32()`, `.to_u32()`)
//!
//! ### Expressions
//!
//! - Numeric literals (f32, i32, u32)
//! - Boolean literals
//! - Type casts (`as f32`, etc.)
//! - Reference stripping (`&x` → `x`)
//! - Parenthesised expressions
//! - Block expressions

// This module is a prototype used by tests; suppress dead-code warnings.
// #![allow(...)] only works at the crate root, so we allow on the items inside.
#[allow(dead_code, unused_imports)]
mod ast;
mod builtin_integration_tests;
#[allow(dead_code, unused_imports)]
pub mod builtins;
#[allow(dead_code, unused_imports)]
mod codegen;
mod control_flow_tests;
#[allow(dead_code, unused_imports)]
mod convert;
pub mod diagnostics;
mod expression_tests;
#[allow(dead_code, unused_imports)]
pub mod optimizer;
mod optimizer_tests;
pub mod performance;
mod pipeline_tests;
pub mod source_map;
#[allow(dead_code, unused_imports)]
pub mod type_map;
mod type_map_integration_tests;

// Re-exports are used by pipeline_tests and future integration.
#[allow(unused_imports)]
pub use ast::*;
#[allow(unused_imports)]
pub use builtins::{BuiltinFunctionRegistry, FunctionCategory, FunctionResolutionError};
#[allow(unused_imports)]
pub use codegen::WgslCodeGen;
#[allow(unused_imports)]
pub use convert::{RustToWgsl, TranspileError};
#[allow(unused_imports)]
pub use diagnostics::{
    DiagnosticBuilder, DiagnosticLevel, DiagnosticOutputFormat, Suggestion, TranspilationDiagnostic,
};
#[allow(unused_imports)]
pub use optimizer::{OptimizationConfig, OptimizationLevel, PassResult, optimize_module};
#[allow(unused_imports)]
pub use source_map::{SourceMap, SourceMapping};
#[allow(unused_imports)]
pub use type_map::{TypeMapper, TypeMappingError, TypeMappingErrorKind, WgslTypeInfo};
