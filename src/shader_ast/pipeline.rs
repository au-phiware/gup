// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! AST-based shader pipeline that integrates with the existing
//! `ComposableShaderPipeline` for backward compatibility.
//!
//! Provides AST-level type checking, optimizations, and WGSL generation
//! while maintaining the same external API.

use super::generator::generate_wgsl_minimal;
use super::optimizer::{AstOptimizationConfig, optimize};
use super::parser::parse_wgsl;
use super::type_check::{CompositionError, FunctionSignature, TypeChecker};
use super::types::*;
use crate::error::GupResult;
use crate::shader_function::{ComposableShaderFunction, ShaderType};
use std::time::Instant;

/// Metadata about a shader function in the AST pipeline.
#[derive(Debug, Clone)]
struct AstPipelineFunctionMeta {
    name: String,
    input_type_name: String,
    output_type_name: String,
    wgsl_code: String,
}

/// An AST-based shader pipeline that provides type-checked composition
/// and AST-level optimizations on top of the existing string-based pipeline.
///
/// ## Usage
///
/// ```rust,ignore
/// use gup::shader_ast::pipeline::AstShaderPipeline;
/// use gup::shader_function::LinearScale;
///
/// let mut pipeline = AstShaderPipeline::new();
/// pipeline.add_function(LinearScale::new(0.0, 100.0, 0.0, 1.0));
///
/// // Validate type compatibility
/// pipeline.validate_types()?;
///
/// // Generate optimized WGSL
/// let wgsl = pipeline.generate_optimized_wgsl();
/// ```
pub struct AstShaderPipeline {
    functions: Vec<AstPipelineFunctionMeta>,
    optimization_config: AstOptimizationConfig,
    /// Cached AST module.
    cached_module: Option<WgslModule>,
    /// Performance tracking.
    composition_time_ms: Option<f64>,
}

impl AstShaderPipeline {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            optimization_config: AstOptimizationConfig::default(),
            cached_module: None,
            composition_time_ms: None,
        }
    }

    /// Set the optimization configuration.
    pub fn with_optimization_config(mut self, config: AstOptimizationConfig) -> Self {
        self.optimization_config = config;
        self
    }

    /// Add a composable shader function to the pipeline.
    pub fn add_function<F: ComposableShaderFunction + 'static>(&mut self, function: F)
    where
        F::Uniforms: Send + Sync + 'static,
    {
        let meta = AstPipelineFunctionMeta {
            name: F::function_name().to_string(),
            input_type_name: F::Input::wgsl_type_name().to_string(),
            output_type_name: F::Output::wgsl_type_name().to_string(),
            wgsl_code: function.generate_wgsl(),
        };
        self.functions.push(meta);
        self.cached_module = None;
    }

    /// Validate that the function chain has compatible types.
    ///
    /// Returns detailed error messages with suggestions for incompatible types.
    pub fn validate_types(&self) -> Result<(), CompositionError> {
        let signatures: Vec<FunctionSignature> = self
            .functions
            .iter()
            .map(|f| FunctionSignature {
                name: f.name.clone(),
                input_type: wgsl_type_from_name(&f.input_type_name),
                output_type: wgsl_type_from_name(&f.output_type_name),
                extra_params: vec![],
            })
            .collect();

        TypeChecker::validate_function_chain(&signatures)
    }

    /// Parse all function WGSL code into a combined AST module.
    pub fn build_ast(&mut self) -> GupResult<&WgslModule> {
        if self.cached_module.is_some() {
            return Ok(self.cached_module.as_ref().unwrap());
        }

        let start = Instant::now();

        let mut module = WgslModule::new();

        for func_meta in &self.functions {
            let wgsl = &func_meta.wgsl_code;
            match parse_wgsl(wgsl) {
                Ok(parsed) => {
                    // Merge parsed functions, structs, globals into the module.
                    for s in parsed.structs {
                        if !module
                            .structs
                            .iter()
                            .any(|existing| existing.name == s.name)
                        {
                            module.structs.push(s);
                        }
                    }
                    for g in parsed.globals {
                        module.globals.push(g);
                    }
                    for f in parsed.functions {
                        module.functions.push(f);
                    }
                }
                Err(e) => {
                    return Err(crate::error::GupError::ShaderCompilationError {
                        shader_type: format!("WGSL parse for function '{}'", func_meta.name),
                        error: e.to_string(),
                    });
                }
            }
        }

        self.composition_time_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
        self.cached_module = Some(module);
        Ok(self.cached_module.as_ref().unwrap())
    }

    /// Generate optimized WGSL from the AST.
    ///
    /// Parses all functions into an AST, runs optimization passes, and
    /// generates clean WGSL output.
    pub fn generate_optimized_wgsl(&mut self) -> GupResult<String> {
        self.build_ast()?;

        let mut module = self.cached_module.take().unwrap();
        let _results = optimize(&mut module, &self.optimization_config);
        let wgsl = generate_wgsl_minimal(&module);

        // Cache the optimized module.
        self.cached_module = Some(module);

        Ok(wgsl)
    }

    /// Get the composition time in milliseconds (from the last `build_ast` call).
    pub fn composition_time_ms(&self) -> Option<f64> {
        self.composition_time_ms
    }

    /// Get the number of functions in the pipeline.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    /// Get the optimization configuration.
    pub fn optimization_config(&self) -> &AstOptimizationConfig {
        &self.optimization_config
    }
}

impl Default for AstShaderPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a WGSL type name string to a `WgslType`.
fn wgsl_type_from_name(name: &str) -> WgslType {
    match name {
        "f32" => WgslType::Scalar(ScalarType::F32),
        "i32" => WgslType::Scalar(ScalarType::I32),
        "u32" => WgslType::Scalar(ScalarType::U32),
        "bool" => WgslType::Scalar(ScalarType::Bool),
        n if n.starts_with("vec") && n.contains('<') => {
            // Parse "vec3<f32>" etc.
            let dim = n.chars().nth(3).and_then(|c| c.to_digit(10)).unwrap_or(4) as u8;
            let scalar = if n.contains("i32") {
                ScalarType::I32
            } else if n.contains("u32") {
                ScalarType::U32
            } else {
                ScalarType::F32
            };
            WgslType::Vector(scalar, dim)
        }
        n if n.starts_with("mat") => {
            // Simplified: assume mat4x4<f32> etc.
            WgslType::Matrix(ScalarType::F32, 4, 4)
        }
        other => WgslType::Struct(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_function::{ColorMap, LinearScale, Vec4};
    use crate::vec4;

    #[test]
    fn test_ast_pipeline_creation() {
        let pipeline = AstShaderPipeline::new();
        assert_eq!(pipeline.function_count(), 0);
    }

    #[test]
    fn test_ast_pipeline_add_function() {
        let mut pipeline = AstShaderPipeline::new();
        pipeline.add_function(LinearScale::new(0.0, 100.0, 0.0, 1.0));
        assert_eq!(pipeline.function_count(), 1);
    }

    #[test]
    fn test_ast_pipeline_validate_compatible_chain() {
        let mut pipeline = AstShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

        pipeline.add_function(scale);
        pipeline.add_function(color_map);

        // LinearScale: f32 → f32, ColorMap: f32 → vec4<f32>
        assert!(pipeline.validate_types().is_ok());
    }

    #[test]
    fn test_ast_pipeline_validate_incompatible_chain() {
        let mut pipeline = AstShaderPipeline::new();
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);

        // ColorMap: f32 → vec4<f32>, then LinearScale: f32 → f32
        // vec4<f32> → f32 is NOT compatible
        pipeline.add_function(color_map);
        pipeline.add_function(scale);

        let result = pipeline.validate_types();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.errors.is_empty());
    }

    #[test]
    fn test_ast_pipeline_build_ast() {
        let mut pipeline = AstShaderPipeline::new();
        pipeline.add_function(LinearScale::new(0.0, 100.0, 0.0, 1.0));

        let module = pipeline.build_ast().unwrap();
        // LinearScale should produce at least one function
        assert!(!module.functions.is_empty());
    }

    #[test]
    fn test_ast_pipeline_generate_optimized_wgsl() {
        let mut pipeline = AstShaderPipeline::new();
        pipeline.add_function(LinearScale::new(0.0, 100.0, 0.0, 1.0));

        let wgsl = pipeline.generate_optimized_wgsl().unwrap();
        assert!(!wgsl.is_empty());
        assert!(wgsl.contains("linear_scale") || wgsl.contains("fn "));
    }

    #[test]
    fn test_ast_pipeline_composition_time() {
        let mut pipeline = AstShaderPipeline::new();
        pipeline.add_function(LinearScale::new(0.0, 100.0, 0.0, 1.0));

        pipeline.build_ast().unwrap();
        let time = pipeline.composition_time_ms();
        assert!(time.is_some());
        // Should be well under 10ms
        assert!(
            time.unwrap() < 10.0,
            "Composition took too long: {:.3}ms",
            time.unwrap()
        );
    }

    #[test]
    fn test_wgsl_type_from_name() {
        assert_eq!(
            wgsl_type_from_name("f32"),
            WgslType::Scalar(ScalarType::F32)
        );
        assert_eq!(
            wgsl_type_from_name("i32"),
            WgslType::Scalar(ScalarType::I32)
        );
        assert_eq!(
            wgsl_type_from_name("vec4<f32>"),
            WgslType::Vector(ScalarType::F32, 4)
        );
        assert_eq!(
            wgsl_type_from_name("MyStruct"),
            WgslType::Struct("MyStruct".to_string())
        );
    }
}
