// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end transpilation pipeline with optimization and analysis.
//!
//! Combines [`RustToWgsl`], [`WgslCodeGen`], [`optimizer`], [`performance`],
//! and [`source_map`] into a single API for transpiling Rust functions to
//! optimised WGSL with diagnostics.

use super::ast::WgslModule;
use super::codegen::WgslCodeGen;
use super::convert::RustToWgsl;
use super::diagnostics::TranspilationDiagnostic;
use super::optimizer::{OptimizationConfig, PassResult, optimize_module};
use super::performance::{PerformanceAnalysisConfig, PerformanceWarning, analyse_performance};
use super::source_map::{SourceMap, SourceMapBuilder, SourceMapping};

// ---------------------------------------------------------------------------
// Pipeline configuration
// ---------------------------------------------------------------------------

/// Configuration for the full transpilation pipeline.
#[derive(Debug, Clone)]
pub struct TranspilePipelineConfig {
    /// Optimization settings.
    pub optimization: OptimizationConfig,
    /// Performance analysis settings.
    pub performance_analysis: PerformanceAnalysisConfig,
    /// Whether to generate source maps.
    pub generate_source_map: bool,
}

impl Default for TranspilePipelineConfig {
    fn default() -> Self {
        Self {
            optimization: OptimizationConfig::default(),
            performance_analysis: PerformanceAnalysisConfig::default(),
            generate_source_map: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline result
// ---------------------------------------------------------------------------

/// Result of a full transpilation pipeline run.
#[derive(Debug, Clone)]
pub struct TranspileResult {
    /// Generated WGSL source code.
    pub wgsl: String,
    /// Optimization pass results.
    pub optimization_results: Vec<PassResult>,
    /// Performance warnings.
    pub performance_warnings: Vec<PerformanceWarning>,
    /// All diagnostics (errors, warnings, hints).
    pub diagnostics: Vec<TranspilationDiagnostic>,
    /// Source map (if requested).
    pub source_map: Option<SourceMap>,
}

impl TranspileResult {
    /// Returns true if there are any error-level diagnostics.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }

    /// Returns true if any optimization changed the code.
    pub fn was_optimised(&self) -> bool {
        self.optimization_results.iter().any(|r| r.changed)
    }
}

// ---------------------------------------------------------------------------
// Pipeline execution
// ---------------------------------------------------------------------------

/// Transpile a Rust function to optimised WGSL with full diagnostics.
///
/// # Arguments
///
/// * `func` – The parsed Rust function to transpile.
/// * `uniform_params` – Parameter names that should be packed into a
///   WGSL uniforms struct.
/// * `config` – Pipeline configuration controlling optimization and
///   analysis.
///
/// # Returns
///
/// A `TranspileResult` with the generated WGSL, optimization results,
/// performance warnings, and diagnostics.
pub fn transpile_function(
    func: &syn::ItemFn,
    uniform_params: impl IntoIterator<Item = String>,
    config: &TranspilePipelineConfig,
) -> Result<TranspileResult, super::convert::TranspileError> {
    let mut diagnostics = Vec::new();

    // Phase 1: Convert Rust AST → WGSL AST
    let mut converter = RustToWgsl::new(uniform_params);
    let wgsl_func = converter.convert_function(func)?;

    let mut module = WgslModule {
        structs: vec![],
        functions: vec![wgsl_func],
    };

    // Phase 2: Optimize
    let optimization_results = optimize_module(&mut module, &config.optimization);

    // Phase 3: Performance analysis
    let performance_warnings = analyse_performance(&module, &config.performance_analysis);

    // Convert performance warnings to diagnostics.
    for warning in &performance_warnings {
        diagnostics.push(warning.to_diagnostic());
    }

    // Phase 4: Generate WGSL text (with optional source map)
    let (wgsl, source_map) = if config.generate_source_map {
        let (text, sm) = generate_with_source_map(&module, &func.sig.ident.to_string());
        (text, Some(sm))
    } else {
        let mut codegen = WgslCodeGen::new();
        (codegen.generate_module(&module), None)
    };

    Ok(TranspileResult {
        wgsl,
        optimization_results,
        performance_warnings,
        diagnostics,
        source_map,
    })
}

/// Generate WGSL text with source map tracking.
fn generate_with_source_map(module: &WgslModule, rust_func_name: &str) -> (String, SourceMap) {
    let mut codegen = WgslCodeGen::new();
    let wgsl = codegen.generate_module(module);

    // Build a basic source map: map each WGSL line to the Rust function.
    let mut builder = SourceMapBuilder::new("<input>");
    let line_count = wgsl.lines().count() as u32;

    // Map the function signature line.
    builder.map_position(1, 1, Some(rust_func_name.to_string()));

    // Map remaining lines (basic 1:1 mapping — more precise mapping
    // requires span tracking in the converter, which can be added later).
    for line in 2..=line_count {
        builder.set_wgsl_position(line, 1);
        builder.map_position(line, 1, None);
    }

    (wgsl, builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transpile_function_basic() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn identity(value: f32) -> f32 {
                return value;
            }
        };

        let result = transpile_function(
            &func,
            std::iter::empty::<String>(),
            &TranspilePipelineConfig::default(),
        )
        .unwrap();

        assert!(result.wgsl.contains("fn identity"));
        assert!(result.wgsl.contains("return value;"));
        assert!(!result.has_errors());
    }

    #[test]
    fn transpile_function_with_optimisation() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test_fn(x: f32) -> f32 {
                let unused = 42.0;
                return x * 1.0;
            }
        };

        let result = transpile_function(
            &func,
            std::iter::empty::<String>(),
            &TranspilePipelineConfig::default(),
        )
        .unwrap();

        assert!(result.was_optimised());
        assert!(!result.wgsl.contains("unused"));
        assert!(result.wgsl.contains("return x;"));
    }

    #[test]
    fn transpile_function_no_optimisation() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test_fn(x: f32) -> f32 {
                let unused = 42.0;
                return x * 1.0;
            }
        };

        let config = TranspilePipelineConfig {
            optimization: OptimizationConfig::none(),
            ..Default::default()
        };

        let result = transpile_function(&func, std::iter::empty::<String>(), &config).unwrap();

        assert!(!result.was_optimised());
        assert!(result.wgsl.contains("unused"));
    }

    #[test]
    fn transpile_function_with_performance_warnings() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn expensive(x: f32) -> f32 {
                let mut total = 0.0;
                for i in 0..100 {
                    total += x;
                }
                return total;
            }
        };

        let result = transpile_function(
            &func,
            std::iter::empty::<String>(),
            &TranspilePipelineConfig::default(),
        )
        .unwrap();

        assert!(!result.performance_warnings.is_empty());
        assert!(
            result
                .performance_warnings
                .iter()
                .any(|w| w.category == super::super::performance::WarningCategory::LargeLoop)
        );
        // Performance warnings are also in diagnostics.
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn transpile_function_with_source_map() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn my_func(x: f32) -> f32 {
                return x + 1.0;
            }
        };

        let config = TranspilePipelineConfig {
            generate_source_map: true,
            ..Default::default()
        };

        let result = transpile_function(&func, std::iter::empty::<String>(), &config).unwrap();

        assert!(result.source_map.is_some());
        let sm = result.source_map.unwrap();
        assert!(!sm.is_empty());

        // First line should map to the function name.
        let mapping = sm.find_rust_location(1, 1);
        assert!(mapping.is_some());
        assert_eq!(mapping.unwrap().name.as_deref(), Some("my_func"));
    }

    #[test]
    fn transpile_result_has_errors() {
        let result = TranspileResult {
            wgsl: String::new(),
            optimization_results: vec![],
            performance_warnings: vec![],
            diagnostics: vec![],
            source_map: None,
        };
        assert!(!result.has_errors());
    }
}
