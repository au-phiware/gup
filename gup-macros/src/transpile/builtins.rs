// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Built-in function library for Rust-to-WGSL transpilation.
//!
//! Provides a comprehensive registry of WGSL built-in functions with
//! type-safe overload resolution. Functions are organized into categories:
//!
//! - **Mathematical**: Trigonometric, exponential, logarithmic, power, utility
//! - **Vector/Matrix**: Geometric operations, vector utilities, matrix functions
//! - **GPU-specific**: Derivative functions, texture sampling, atomics,
//!   barriers, pack/unpack
//!
//! The registry resolves function calls by name and argument types, mapping
//! Rust idioms to the appropriate WGSL built-in.

use std::collections::HashMap;

use super::ast::{ScalarType, WgslType};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors during function overload resolution.
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionResolutionError {
    /// The requested function name was not found in the registry.
    FunctionNotFound {
        name: String,
        available: Vec<String>,
    },
    /// The function exists but no overload matches the provided argument types.
    NoMatchingOverload {
        name: String,
        arg_types: Vec<WgslType>,
        available_overloads: Vec<FunctionSignature>,
    },
}

impl std::fmt::Display for FunctionResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FunctionResolutionError::FunctionNotFound { name, available } => {
                write!(
                    f,
                    "Function '{name}' not found. Similar functions: {}",
                    available.join(", ")
                )
            }
            FunctionResolutionError::NoMatchingOverload {
                name,
                arg_types,
                available_overloads,
            } => {
                let types: Vec<String> = arg_types.iter().map(|t| t.to_string()).collect();
                let overloads: Vec<String> =
                    available_overloads.iter().map(|o| o.to_string()).collect();
                write!(
                    f,
                    "No matching overload for '{name}({})'. Available overloads: {}",
                    types.join(", "),
                    overloads.join("; ")
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Function descriptor types
// ---------------------------------------------------------------------------

/// Category of a built-in function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionCategory {
    /// Trigonometric functions (sin, cos, tan, etc.)
    Trigonometric,
    /// Exponential/logarithmic functions (exp, log, pow, etc.)
    Exponential,
    /// Common math utility functions (abs, floor, clamp, etc.)
    MathUtility,
    /// Interpolation and smoothing (mix, smoothstep, step)
    Interpolation,
    /// Vector/geometry functions (length, normalize, dot, cross, etc.)
    Geometric,
    /// Matrix operations (transpose, determinant)
    Matrix,
    /// Fragment shader derivative functions (dpdx, dpdy, fwidth)
    Derivative,
    /// Texture sampling functions
    Texture,
    /// Atomic operations for compute shaders
    Atomic,
    /// Barrier and synchronization functions
    Barrier,
    /// Data packing/unpacking functions
    PackUnpack,
    /// Logical/comparison functions (select, all, any)
    Logical,
    /// Bit manipulation functions
    BitManipulation,
}

/// Describes the accepted parameter types for a function overload.
///
/// WGSL built-in functions are heavily overloaded; e.g. `abs` accepts
/// `f32`, `i32`, `vec2<f32>`, `vec3<i32>`, etc.  [`ParamPattern`] lets
/// us express these families concisely.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParamPattern {
    /// Matches exactly this type.
    Exact(WgslType),
    /// Matches any float scalar or float vector of any dimension.
    AnyFloatScalarOrVec,
    /// Matches any signed-integer scalar or signed-integer vector.
    AnySignedScalarOrVec,
    /// Matches any numeric scalar or vector (f32, i32, u32 + vecN variants).
    AnyNumericScalarOrVec,
    /// Matches any scalar (f32, i32, u32, bool).
    AnyScalar,
    /// Matches any vector regardless of scalar type and dimension.
    AnyVec,
    /// Matches any float vector (vec2/3/4<f32>).
    AnyFloatVec,
    /// Matches any square matrix.
    AnySquareMatrix,
}

impl ParamPattern {
    /// Test whether the given concrete type matches this pattern.
    pub fn matches(&self, ty: &WgslType) -> bool {
        match self {
            ParamPattern::Exact(expected) => ty == expected,
            ParamPattern::AnyFloatScalarOrVec => matches!(
                ty,
                WgslType::Scalar(ScalarType::F32) | WgslType::Vector(ScalarType::F32, _)
            ),
            ParamPattern::AnySignedScalarOrVec => matches!(
                ty,
                WgslType::Scalar(ScalarType::I32)
                    | WgslType::Scalar(ScalarType::F32)
                    | WgslType::Vector(ScalarType::I32, _)
                    | WgslType::Vector(ScalarType::F32, _)
            ),
            ParamPattern::AnyNumericScalarOrVec => matches!(
                ty,
                WgslType::Scalar(ScalarType::F32)
                    | WgslType::Scalar(ScalarType::I32)
                    | WgslType::Scalar(ScalarType::U32)
                    | WgslType::Vector(ScalarType::F32, _)
                    | WgslType::Vector(ScalarType::I32, _)
                    | WgslType::Vector(ScalarType::U32, _)
            ),
            ParamPattern::AnyScalar => matches!(ty, WgslType::Scalar(_)),
            ParamPattern::AnyVec => matches!(ty, WgslType::Vector(_, _)),
            ParamPattern::AnyFloatVec => matches!(ty, WgslType::Vector(ScalarType::F32, _)),
            ParamPattern::AnySquareMatrix => {
                matches!(ty, WgslType::Matrix(ScalarType::F32, c, r) if c == r)
            }
        }
    }
}

/// Describes how the return type is derived from the arguments.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReturnTypeRule {
    /// Fixed return type, regardless of input.
    Fixed(WgslType),
    /// Return type is the same as the first argument.
    SameAsFirstArg,
    /// Return type is a scalar of the same kind as the first argument's scalar.
    ScalarOfFirstArg,
    /// Always returns void (for barrier functions, etc.)
    Void,
}

/// A function signature describing one overload of a built-in function.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionSignature {
    /// The WGSL function name (may differ from the Rust name).
    pub wgsl_name: String,
    /// Expected parameter type patterns.
    pub param_patterns: Vec<ParamPattern>,
    /// Rule for computing the return type.
    pub return_rule: ReturnTypeRule,
    /// Which category this function belongs to.
    pub category: FunctionCategory,
}

impl std::fmt::Display for FunctionSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let params: Vec<String> = self
            .param_patterns
            .iter()
            .map(|p| format!("{p:?}"))
            .collect();
        write!(f, "{}({})", self.wgsl_name, params.join(", "))
    }
}

impl FunctionSignature {
    /// Check whether the given concrete argument types match this signature.
    pub fn matches(&self, arg_types: &[WgslType]) -> bool {
        if arg_types.len() != self.param_patterns.len() {
            return false;
        }
        self.param_patterns
            .iter()
            .zip(arg_types)
            .all(|(pattern, arg)| pattern.matches(arg))
    }

    /// Compute the concrete return type for the given argument types.
    ///
    /// Panics if `arg_types` is empty when the rule requires inspecting
    /// the first argument.
    pub fn resolve_return_type(&self, arg_types: &[WgslType]) -> WgslType {
        match &self.return_rule {
            ReturnTypeRule::Fixed(ty) => ty.clone(),
            ReturnTypeRule::SameAsFirstArg => arg_types
                .first()
                .cloned()
                .unwrap_or(WgslType::Scalar(ScalarType::F32)),
            ReturnTypeRule::ScalarOfFirstArg => match arg_types.first() {
                Some(WgslType::Vector(s, _)) => WgslType::Scalar(*s),
                Some(other) => other.clone(),
                None => WgslType::Scalar(ScalarType::F32),
            },
            ReturnTypeRule::Void => WgslType::Void,
        }
    }
}

// ---------------------------------------------------------------------------
// Built-in function registry
// ---------------------------------------------------------------------------

/// Registry of all WGSL built-in functions with overload resolution.
///
/// The registry is populated at construction time with the complete set of
/// WGSL built-in functions. It provides lookup by Rust-side name and
/// argument types, resolving to the appropriate WGSL function name.
pub struct BuiltinFunctionRegistry {
    /// Map from Rust-side function name to list of overloaded signatures.
    overloads: HashMap<String, Vec<FunctionSignature>>,
}

impl BuiltinFunctionRegistry {
    /// Create a new registry pre-populated with all WGSL built-in functions.
    pub fn new() -> Self {
        let mut registry = Self {
            overloads: HashMap::new(),
        };
        registry.register_math_functions();
        registry.register_interpolation_functions();
        registry.register_geometric_functions();
        registry.register_matrix_functions();
        registry.register_derivative_functions();
        registry.register_texture_functions();
        registry.register_atomic_functions();
        registry.register_barrier_functions();
        registry.register_pack_unpack_functions();
        registry.register_logical_functions();
        registry.register_bit_manipulation_functions();
        registry
    }

    /// Resolve a function call by name and argument types.
    ///
    /// Returns the matching [`FunctionSignature`] or an error describing why
    /// resolution failed.
    pub fn resolve(
        &self,
        name: &str,
        arg_types: &[WgslType],
    ) -> Result<&FunctionSignature, FunctionResolutionError> {
        let overloads = self.overloads.get(name).ok_or_else(|| {
            // Find similar function names for suggestions
            let available = self.suggest_similar(name);
            FunctionResolutionError::FunctionNotFound {
                name: name.to_string(),
                available,
            }
        })?;

        overloads
            .iter()
            .find(|sig| sig.matches(arg_types))
            .ok_or_else(|| FunctionResolutionError::NoMatchingOverload {
                name: name.to_string(),
                arg_types: arg_types.to_vec(),
                available_overloads: overloads.clone(),
            })
    }

    /// List all overloads for a given function name.
    pub fn list_overloads(&self, name: &str) -> Vec<&FunctionSignature> {
        self.overloads
            .get(name)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Check whether a function name is registered.
    pub fn has_function(&self, name: &str) -> bool {
        self.overloads.contains_key(name)
    }

    /// Return all registered function names.
    pub fn function_names(&self) -> Vec<&str> {
        self.overloads.keys().map(|s| s.as_str()).collect()
    }

    /// Return the number of distinct function names registered.
    pub fn function_count(&self) -> usize {
        self.overloads.len()
    }

    /// Return the total number of overloads across all functions.
    pub fn overload_count(&self) -> usize {
        self.overloads.values().map(|v| v.len()).sum()
    }

    /// List all functions in a specific category.
    pub fn functions_in_category(&self, category: FunctionCategory) -> Vec<&str> {
        self.overloads
            .iter()
            .filter(|(_, sigs)| sigs.iter().any(|s| s.category == category))
            .map(|(name, _)| name.as_str())
            .collect()
    }

    // -----------------------------------------------------------------------
    // Registration helpers
    // -----------------------------------------------------------------------

    fn register(&mut self, rust_name: &str, sig: FunctionSignature) {
        self.overloads
            .entry(rust_name.to_string())
            .or_default()
            .push(sig);
    }

    /// Register a unary function that works on float scalars and float
    /// vectors, returning the same type.
    fn register_unary_float(&mut self, name: &str, category: FunctionCategory) {
        self.register(
            name,
            FunctionSignature {
                wgsl_name: name.to_string(),
                param_patterns: vec![ParamPattern::AnyFloatScalarOrVec],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category,
            },
        );
    }

    /// Register a binary function that takes two matching float-or-vec
    /// arguments and returns the same type.
    fn register_binary_float(&mut self, name: &str, category: FunctionCategory) {
        self.register(
            name,
            FunctionSignature {
                wgsl_name: name.to_string(),
                param_patterns: vec![
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category,
            },
        );
    }

    /// Register a unary function that works on any numeric (f32, i32, u32 +
    /// vectors), returning the same type.
    fn register_unary_numeric(&mut self, name: &str, category: FunctionCategory) {
        self.register(
            name,
            FunctionSignature {
                wgsl_name: name.to_string(),
                param_patterns: vec![ParamPattern::AnyNumericScalarOrVec],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category,
            },
        );
    }

    /// Register a binary function on any numeric types, returning the same
    /// type as the first argument.
    fn register_binary_numeric(&mut self, name: &str, category: FunctionCategory) {
        self.register(
            name,
            FunctionSignature {
                wgsl_name: name.to_string(),
                param_patterns: vec![
                    ParamPattern::AnyNumericScalarOrVec,
                    ParamPattern::AnyNumericScalarOrVec,
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category,
            },
        );
    }

    // -----------------------------------------------------------------------
    // Mathematical functions (AC1)
    // -----------------------------------------------------------------------

    fn register_math_functions(&mut self) {
        // Trigonometric
        for name in &[
            "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "asinh", "acosh",
            "atanh",
        ] {
            self.register_unary_float(name, FunctionCategory::Trigonometric);
        }
        // atan2(y, x) — two-argument arctangent
        self.register_binary_float("atan2", FunctionCategory::Trigonometric);

        // Exponential / logarithmic
        for name in &["exp", "exp2", "log", "log2", "sqrt", "inversesqrt"] {
            self.register_unary_float(name, FunctionCategory::Exponential);
        }
        // pow(base, exponent)
        self.register_binary_float("pow", FunctionCategory::Exponential);
        // ldexp(significand, exponent) — significand * 2^exponent
        self.register(
            "ldexp",
            FunctionSignature {
                wgsl_name: "ldexp".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnySignedScalarOrVec,
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::Exponential,
            },
        );

        // Utility — numeric (f32 + i32 + u32)
        for name in &["abs", "sign"] {
            self.register_unary_numeric(name, FunctionCategory::MathUtility);
        }
        for name in &["min", "max"] {
            self.register_binary_numeric(name, FunctionCategory::MathUtility);
        }
        // clamp(x, lo, hi)
        self.register(
            "clamp",
            FunctionSignature {
                wgsl_name: "clamp".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyNumericScalarOrVec,
                    ParamPattern::AnyNumericScalarOrVec,
                    ParamPattern::AnyNumericScalarOrVec,
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::MathUtility,
            },
        );

        // Utility — float-only rounding
        for name in &["floor", "ceil", "round", "trunc", "fract"] {
            self.register_unary_float(name, FunctionCategory::MathUtility);
        }

        // saturate — clamps to [0, 1]
        self.register_unary_float("saturate", FunctionCategory::MathUtility);

        // degrees / radians
        self.register_unary_float("degrees", FunctionCategory::MathUtility);
        self.register_unary_float("radians", FunctionCategory::MathUtility);

        // fma(a, b, c) — fused multiply-add
        self.register(
            "fma",
            FunctionSignature {
                wgsl_name: "fma".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::MathUtility,
            },
        );
    }

    // -----------------------------------------------------------------------
    // Interpolation functions (part of AC1)
    // -----------------------------------------------------------------------

    fn register_interpolation_functions(&mut self) {
        // mix(a, b, t)
        self.register(
            "mix",
            FunctionSignature {
                wgsl_name: "mix".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::Interpolation,
            },
        );
        // step(edge, x)
        self.register_binary_float("step", FunctionCategory::Interpolation);
        // smoothstep(edge0, edge1, x)
        self.register(
            "smoothstep",
            FunctionSignature {
                wgsl_name: "smoothstep".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::Interpolation,
            },
        );
    }

    // -----------------------------------------------------------------------
    // Geometric / vector functions (AC2)
    // -----------------------------------------------------------------------

    fn register_geometric_functions(&mut self) {
        // length(v) → f32  (vector → scalar)
        self.register(
            "length",
            FunctionSignature {
                wgsl_name: "length".to_string(),
                param_patterns: vec![ParamPattern::AnyFloatScalarOrVec],
                return_rule: ReturnTypeRule::ScalarOfFirstArg,
                category: FunctionCategory::Geometric,
            },
        );

        // normalize(v) → same vector type
        self.register(
            "normalize",
            FunctionSignature {
                wgsl_name: "normalize".to_string(),
                param_patterns: vec![ParamPattern::AnyFloatScalarOrVec],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::Geometric,
            },
        );

        // dot(a, b) → scalar
        self.register(
            "dot",
            FunctionSignature {
                wgsl_name: "dot".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                ],
                return_rule: ReturnTypeRule::ScalarOfFirstArg,
                category: FunctionCategory::Geometric,
            },
        );

        // cross(a, b) → vec3<f32>
        self.register(
            "cross",
            FunctionSignature {
                wgsl_name: "cross".to_string(),
                param_patterns: vec![
                    ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 3)),
                    ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 3)),
                ],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::F32, 3)),
                category: FunctionCategory::Geometric,
            },
        );

        // distance(a, b) → f32
        self.register(
            "distance",
            FunctionSignature {
                wgsl_name: "distance".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                ],
                return_rule: ReturnTypeRule::ScalarOfFirstArg,
                category: FunctionCategory::Geometric,
            },
        );

        // reflect(incident, normal) → same vector type
        self.register_binary_float("reflect", FunctionCategory::Geometric);

        // refract(incident, normal, eta) where eta is f32
        self.register(
            "refract",
            FunctionSignature {
                wgsl_name: "refract".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::Exact(WgslType::Scalar(ScalarType::F32)),
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::Geometric,
            },
        );

        // faceforward(e1, e2, e_ref)
        self.register(
            "faceforward",
            FunctionSignature {
                wgsl_name: "faceforward".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                    ParamPattern::AnyFloatScalarOrVec,
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::Geometric,
            },
        );
    }

    // -----------------------------------------------------------------------
    // Matrix functions (AC2)
    // -----------------------------------------------------------------------

    fn register_matrix_functions(&mut self) {
        // transpose(m) — returns transposed matrix
        self.register(
            "transpose",
            FunctionSignature {
                wgsl_name: "transpose".to_string(),
                param_patterns: vec![ParamPattern::AnySquareMatrix],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::Matrix,
            },
        );

        // determinant(m) → f32
        self.register(
            "determinant",
            FunctionSignature {
                wgsl_name: "determinant".to_string(),
                param_patterns: vec![ParamPattern::AnySquareMatrix],
                return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(ScalarType::F32)),
                category: FunctionCategory::Matrix,
            },
        );
    }

    // -----------------------------------------------------------------------
    // Derivative functions (AC3)
    // -----------------------------------------------------------------------

    fn register_derivative_functions(&mut self) {
        for name in &[
            "dpdx",
            "dpdy",
            "fwidth",
            "dpdxCoarse",
            "dpdyCoarse",
            "fwidthCoarse",
            "dpdxFine",
            "dpdyFine",
            "fwidthFine",
        ] {
            self.register_unary_float(name, FunctionCategory::Derivative);
        }
    }

    // -----------------------------------------------------------------------
    // Texture functions (AC3)
    // -----------------------------------------------------------------------

    fn register_texture_functions(&mut self) {
        // textureSample(texture, sampler, coord) → vec4<f32>
        self.register(
            "textureSample",
            FunctionSignature {
                wgsl_name: "textureSample".to_string(),
                param_patterns: vec![
                    ParamPattern::Exact(WgslType::Struct("texture_2d".to_string())),
                    ParamPattern::Exact(WgslType::Struct("sampler".to_string())),
                    ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 2)),
                ],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::F32, 4)),
                category: FunctionCategory::Texture,
            },
        );

        // textureSampleLevel(texture, sampler, coord, level) → vec4<f32>
        self.register(
            "textureSampleLevel",
            FunctionSignature {
                wgsl_name: "textureSampleLevel".to_string(),
                param_patterns: vec![
                    ParamPattern::Exact(WgslType::Struct("texture_2d".to_string())),
                    ParamPattern::Exact(WgslType::Struct("sampler".to_string())),
                    ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 2)),
                    ParamPattern::Exact(WgslType::Scalar(ScalarType::F32)),
                ],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::F32, 4)),
                category: FunctionCategory::Texture,
            },
        );

        // textureSampleBias(texture, sampler, coord, bias) → vec4<f32>
        self.register(
            "textureSampleBias",
            FunctionSignature {
                wgsl_name: "textureSampleBias".to_string(),
                param_patterns: vec![
                    ParamPattern::Exact(WgslType::Struct("texture_2d".to_string())),
                    ParamPattern::Exact(WgslType::Struct("sampler".to_string())),
                    ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 2)),
                    ParamPattern::Exact(WgslType::Scalar(ScalarType::F32)),
                ],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::F32, 4)),
                category: FunctionCategory::Texture,
            },
        );

        // textureSampleGrad(texture, sampler, coord, ddx, ddy) → vec4<f32>
        self.register(
            "textureSampleGrad",
            FunctionSignature {
                wgsl_name: "textureSampleGrad".to_string(),
                param_patterns: vec![
                    ParamPattern::Exact(WgslType::Struct("texture_2d".to_string())),
                    ParamPattern::Exact(WgslType::Struct("sampler".to_string())),
                    ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 2)),
                    ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 2)),
                    ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 2)),
                ],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::F32, 4)),
                category: FunctionCategory::Texture,
            },
        );

        // textureLoad(texture, coord, level) → vec4<f32>
        self.register(
            "textureLoad",
            FunctionSignature {
                wgsl_name: "textureLoad".to_string(),
                param_patterns: vec![
                    ParamPattern::Exact(WgslType::Struct("texture_2d".to_string())),
                    ParamPattern::Exact(WgslType::Vector(ScalarType::I32, 2)),
                    ParamPattern::Exact(WgslType::Scalar(ScalarType::I32)),
                ],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::F32, 4)),
                category: FunctionCategory::Texture,
            },
        );

        // textureStore(texture, coord, value)
        self.register(
            "textureStore",
            FunctionSignature {
                wgsl_name: "textureStore".to_string(),
                param_patterns: vec![
                    ParamPattern::Exact(WgslType::Struct("texture_storage_2d".to_string())),
                    ParamPattern::Exact(WgslType::Vector(ScalarType::I32, 2)),
                    ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 4)),
                ],
                return_rule: ReturnTypeRule::Void,
                category: FunctionCategory::Texture,
            },
        );

        // textureDimensions(texture) → vec2<u32>
        self.register(
            "textureDimensions",
            FunctionSignature {
                wgsl_name: "textureDimensions".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Struct(
                    "texture_2d".to_string(),
                ))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::U32, 2)),
                category: FunctionCategory::Texture,
            },
        );

        // textureNumLevels(texture) → u32
        self.register(
            "textureNumLevels",
            FunctionSignature {
                wgsl_name: "textureNumLevels".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Struct(
                    "texture_2d".to_string(),
                ))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(ScalarType::U32)),
                category: FunctionCategory::Texture,
            },
        );
    }

    // -----------------------------------------------------------------------
    // Atomic functions (AC3)
    // -----------------------------------------------------------------------

    fn register_atomic_functions(&mut self) {
        // atomicLoad(ptr) → i32/u32
        for scalar in &[ScalarType::I32, ScalarType::U32] {
            self.register(
                "atomicLoad",
                FunctionSignature {
                    wgsl_name: "atomicLoad".to_string(),
                    param_patterns: vec![ParamPattern::Exact(WgslType::Struct(format!(
                        "atomic<{scalar}>"
                    )))],
                    return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(*scalar)),
                    category: FunctionCategory::Atomic,
                },
            );
        }

        // atomicStore(ptr, value)
        for scalar in &[ScalarType::I32, ScalarType::U32] {
            self.register(
                "atomicStore",
                FunctionSignature {
                    wgsl_name: "atomicStore".to_string(),
                    param_patterns: vec![
                        ParamPattern::Exact(WgslType::Struct(format!("atomic<{scalar}>"))),
                        ParamPattern::Exact(WgslType::Scalar(*scalar)),
                    ],
                    return_rule: ReturnTypeRule::Void,
                    category: FunctionCategory::Atomic,
                },
            );
        }

        // atomicAdd, atomicSub, atomicMax, atomicMin, atomicAnd, atomicOr, atomicXor
        for op in &[
            "atomicAdd",
            "atomicSub",
            "atomicMax",
            "atomicMin",
            "atomicAnd",
            "atomicOr",
            "atomicXor",
        ] {
            for scalar in &[ScalarType::I32, ScalarType::U32] {
                self.register(
                    op,
                    FunctionSignature {
                        wgsl_name: op.to_string(),
                        param_patterns: vec![
                            ParamPattern::Exact(WgslType::Struct(format!("atomic<{scalar}>"))),
                            ParamPattern::Exact(WgslType::Scalar(*scalar)),
                        ],
                        return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(*scalar)),
                        category: FunctionCategory::Atomic,
                    },
                );
            }
        }

        // atomicExchange(ptr, value) → old value
        for scalar in &[ScalarType::I32, ScalarType::U32] {
            self.register(
                "atomicExchange",
                FunctionSignature {
                    wgsl_name: "atomicExchange".to_string(),
                    param_patterns: vec![
                        ParamPattern::Exact(WgslType::Struct(format!("atomic<{scalar}>"))),
                        ParamPattern::Exact(WgslType::Scalar(*scalar)),
                    ],
                    return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(*scalar)),
                    category: FunctionCategory::Atomic,
                },
            );
        }

        // atomicCompareExchangeWeak(ptr, compare, value)
        for scalar in &[ScalarType::I32, ScalarType::U32] {
            self.register(
                "atomicCompareExchangeWeak",
                FunctionSignature {
                    wgsl_name: "atomicCompareExchangeWeak".to_string(),
                    param_patterns: vec![
                        ParamPattern::Exact(WgslType::Struct(format!("atomic<{scalar}>"))),
                        ParamPattern::Exact(WgslType::Scalar(*scalar)),
                        ParamPattern::Exact(WgslType::Scalar(*scalar)),
                    ],
                    // Returns __atomic_compare_exchange_result struct, model as struct
                    return_rule: ReturnTypeRule::Fixed(WgslType::Struct(
                        "__atomic_compare_exchange_result".to_string(),
                    )),
                    category: FunctionCategory::Atomic,
                },
            );
        }
    }

    // -----------------------------------------------------------------------
    // Barrier / synchronization functions (AC3)
    // -----------------------------------------------------------------------

    fn register_barrier_functions(&mut self) {
        for name in &[
            "storageBarrier",
            "workgroupBarrier",
            "textureBarrier",
            "workgroupUniformLoad",
        ] {
            self.register(
                name,
                FunctionSignature {
                    wgsl_name: name.to_string(),
                    param_patterns: vec![],
                    return_rule: ReturnTypeRule::Void,
                    category: FunctionCategory::Barrier,
                },
            );
        }
    }

    // -----------------------------------------------------------------------
    // Pack / unpack functions (AC3)
    // -----------------------------------------------------------------------

    fn register_pack_unpack_functions(&mut self) {
        // pack4x8snorm(v: vec4<f32>) → u32
        self.register(
            "pack4x8snorm",
            FunctionSignature {
                wgsl_name: "pack4x8snorm".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 4))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(ScalarType::U32)),
                category: FunctionCategory::PackUnpack,
            },
        );
        self.register(
            "pack4x8unorm",
            FunctionSignature {
                wgsl_name: "pack4x8unorm".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 4))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(ScalarType::U32)),
                category: FunctionCategory::PackUnpack,
            },
        );
        self.register(
            "pack2x16snorm",
            FunctionSignature {
                wgsl_name: "pack2x16snorm".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 2))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(ScalarType::U32)),
                category: FunctionCategory::PackUnpack,
            },
        );
        self.register(
            "pack2x16unorm",
            FunctionSignature {
                wgsl_name: "pack2x16unorm".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 2))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(ScalarType::U32)),
                category: FunctionCategory::PackUnpack,
            },
        );
        self.register(
            "pack2x16float",
            FunctionSignature {
                wgsl_name: "pack2x16float".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Vector(ScalarType::F32, 2))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(ScalarType::U32)),
                category: FunctionCategory::PackUnpack,
            },
        );

        // unpack operations: u32 → vec4<f32> or vec2<f32>
        self.register(
            "unpack4x8snorm",
            FunctionSignature {
                wgsl_name: "unpack4x8snorm".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Scalar(ScalarType::U32))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::F32, 4)),
                category: FunctionCategory::PackUnpack,
            },
        );
        self.register(
            "unpack4x8unorm",
            FunctionSignature {
                wgsl_name: "unpack4x8unorm".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Scalar(ScalarType::U32))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::F32, 4)),
                category: FunctionCategory::PackUnpack,
            },
        );
        self.register(
            "unpack2x16snorm",
            FunctionSignature {
                wgsl_name: "unpack2x16snorm".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Scalar(ScalarType::U32))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::F32, 2)),
                category: FunctionCategory::PackUnpack,
            },
        );
        self.register(
            "unpack2x16unorm",
            FunctionSignature {
                wgsl_name: "unpack2x16unorm".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Scalar(ScalarType::U32))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::F32, 2)),
                category: FunctionCategory::PackUnpack,
            },
        );
        self.register(
            "unpack2x16float",
            FunctionSignature {
                wgsl_name: "unpack2x16float".to_string(),
                param_patterns: vec![ParamPattern::Exact(WgslType::Scalar(ScalarType::U32))],
                return_rule: ReturnTypeRule::Fixed(WgslType::Vector(ScalarType::F32, 2)),
                category: FunctionCategory::PackUnpack,
            },
        );
    }

    // -----------------------------------------------------------------------
    // Logical / comparison functions (AC4 helpers)
    // -----------------------------------------------------------------------

    fn register_logical_functions(&mut self) {
        // select(false_val, true_val, cond) — already handled by if-else
        // transpilation, but also register for direct calls.
        self.register(
            "select",
            FunctionSignature {
                wgsl_name: "select".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyNumericScalarOrVec,
                    ParamPattern::AnyNumericScalarOrVec,
                    ParamPattern::Exact(WgslType::Scalar(ScalarType::Bool)),
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::Logical,
            },
        );

        // all(v: vec<bool>) → bool
        self.register(
            "all",
            FunctionSignature {
                wgsl_name: "all".to_string(),
                param_patterns: vec![ParamPattern::AnyVec],
                return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(ScalarType::Bool)),
                category: FunctionCategory::Logical,
            },
        );

        // any(v: vec<bool>) → bool
        self.register(
            "any",
            FunctionSignature {
                wgsl_name: "any".to_string(),
                param_patterns: vec![ParamPattern::AnyVec],
                return_rule: ReturnTypeRule::Fixed(WgslType::Scalar(ScalarType::Bool)),
                category: FunctionCategory::Logical,
            },
        );
    }

    // -----------------------------------------------------------------------
    // Bit manipulation functions
    // -----------------------------------------------------------------------

    fn register_bit_manipulation_functions(&mut self) {
        // countOneBits(v) — popcount
        self.register(
            "countOneBits",
            FunctionSignature {
                wgsl_name: "countOneBits".to_string(),
                param_patterns: vec![ParamPattern::AnyNumericScalarOrVec],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::BitManipulation,
            },
        );

        // countLeadingZeros(v)
        self.register(
            "countLeadingZeros",
            FunctionSignature {
                wgsl_name: "countLeadingZeros".to_string(),
                param_patterns: vec![ParamPattern::AnyNumericScalarOrVec],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::BitManipulation,
            },
        );

        // countTrailingZeros(v)
        self.register(
            "countTrailingZeros",
            FunctionSignature {
                wgsl_name: "countTrailingZeros".to_string(),
                param_patterns: vec![ParamPattern::AnyNumericScalarOrVec],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::BitManipulation,
            },
        );

        // firstLeadingBit(v)
        self.register(
            "firstLeadingBit",
            FunctionSignature {
                wgsl_name: "firstLeadingBit".to_string(),
                param_patterns: vec![ParamPattern::AnyNumericScalarOrVec],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::BitManipulation,
            },
        );

        // firstTrailingBit(v)
        self.register(
            "firstTrailingBit",
            FunctionSignature {
                wgsl_name: "firstTrailingBit".to_string(),
                param_patterns: vec![ParamPattern::AnyNumericScalarOrVec],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::BitManipulation,
            },
        );

        // reverseBits(v)
        self.register(
            "reverseBits",
            FunctionSignature {
                wgsl_name: "reverseBits".to_string(),
                param_patterns: vec![ParamPattern::AnyNumericScalarOrVec],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::BitManipulation,
            },
        );

        // extractBits(v, offset, count)
        self.register(
            "extractBits",
            FunctionSignature {
                wgsl_name: "extractBits".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyNumericScalarOrVec,
                    ParamPattern::Exact(WgslType::Scalar(ScalarType::U32)),
                    ParamPattern::Exact(WgslType::Scalar(ScalarType::U32)),
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::BitManipulation,
            },
        );

        // insertBits(v, newbits, offset, count)
        self.register(
            "insertBits",
            FunctionSignature {
                wgsl_name: "insertBits".to_string(),
                param_patterns: vec![
                    ParamPattern::AnyNumericScalarOrVec,
                    ParamPattern::AnyNumericScalarOrVec,
                    ParamPattern::Exact(WgslType::Scalar(ScalarType::U32)),
                    ParamPattern::Exact(WgslType::Scalar(ScalarType::U32)),
                ],
                return_rule: ReturnTypeRule::SameAsFirstArg,
                category: FunctionCategory::BitManipulation,
            },
        );
    }

    // -----------------------------------------------------------------------
    // Suggestion helpers
    // -----------------------------------------------------------------------

    /// Find function names that are similar to the given name (for error
    /// messages).
    fn suggest_similar(&self, name: &str) -> Vec<String> {
        let name_lower = name.to_lowercase();
        let mut suggestions: Vec<String> = self
            .overloads
            .keys()
            .filter(|k| {
                let k_lower = k.to_lowercase();
                k_lower.contains(&name_lower)
                    || name_lower.contains(&k_lower)
                    || edit_distance(&k_lower, &name_lower) <= 2
            })
            .cloned()
            .collect();
        suggestions.sort();
        suggestions.truncate(5);
        suggestions
    }
}

impl Default for BuiltinFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Levenshtein distance for suggestions
// ---------------------------------------------------------------------------

/// Simple Levenshtein edit distance for function name suggestions.
fn edit_distance(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();

    let mut prev = (0..=n).collect::<Vec<_>>();
    let mut curr = vec![0; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transpile::ast::{ScalarType, WgslType};

    #[test]
    fn registry_creates_with_functions() {
        let reg = BuiltinFunctionRegistry::new();
        assert!(reg.function_count() > 50, "Expected >50 distinct functions");
        assert!(
            reg.overload_count() > 60,
            "Expected >60 overloads, got {}",
            reg.overload_count()
        );
    }

    // -- Math function resolution --

    #[test]
    fn resolve_sin_f32() {
        let reg = BuiltinFunctionRegistry::new();
        let sig = reg
            .resolve("sin", &[WgslType::Scalar(ScalarType::F32)])
            .unwrap();
        assert_eq!(sig.wgsl_name, "sin");
        assert_eq!(
            sig.resolve_return_type(&[WgslType::Scalar(ScalarType::F32)]),
            WgslType::Scalar(ScalarType::F32)
        );
    }

    #[test]
    fn resolve_sin_vec3() {
        let reg = BuiltinFunctionRegistry::new();
        let sig = reg
            .resolve("sin", &[WgslType::Vector(ScalarType::F32, 3)])
            .unwrap();
        assert_eq!(sig.wgsl_name, "sin");
        assert_eq!(
            sig.resolve_return_type(&[WgslType::Vector(ScalarType::F32, 3)]),
            WgslType::Vector(ScalarType::F32, 3)
        );
    }

    #[test]
    fn resolve_abs_f32() {
        let reg = BuiltinFunctionRegistry::new();
        let sig = reg
            .resolve("abs", &[WgslType::Scalar(ScalarType::F32)])
            .unwrap();
        assert_eq!(sig.wgsl_name, "abs");
    }

    #[test]
    fn resolve_abs_i32() {
        let reg = BuiltinFunctionRegistry::new();
        let sig = reg
            .resolve("abs", &[WgslType::Scalar(ScalarType::I32)])
            .unwrap();
        assert_eq!(sig.wgsl_name, "abs");
    }

    #[test]
    fn resolve_abs_vec3_f32() {
        let reg = BuiltinFunctionRegistry::new();
        let sig = reg
            .resolve("abs", &[WgslType::Vector(ScalarType::F32, 3)])
            .unwrap();
        assert_eq!(
            sig.resolve_return_type(&[WgslType::Vector(ScalarType::F32, 3)]),
            WgslType::Vector(ScalarType::F32, 3)
        );
    }

    #[test]
    fn resolve_clamp_f32() {
        let reg = BuiltinFunctionRegistry::new();
        let f32_type = WgslType::Scalar(ScalarType::F32);
        let sig = reg
            .resolve("clamp", &[f32_type.clone(), f32_type.clone(), f32_type])
            .unwrap();
        assert_eq!(sig.wgsl_name, "clamp");
    }

    #[test]
    fn resolve_min_i32() {
        let reg = BuiltinFunctionRegistry::new();
        let i32_type = WgslType::Scalar(ScalarType::I32);
        let sig = reg.resolve("min", &[i32_type.clone(), i32_type]).unwrap();
        assert_eq!(sig.wgsl_name, "min");
    }

    #[test]
    fn resolve_pow_f32() {
        let reg = BuiltinFunctionRegistry::new();
        let f32_type = WgslType::Scalar(ScalarType::F32);
        let sig = reg.resolve("pow", &[f32_type.clone(), f32_type]).unwrap();
        assert_eq!(sig.wgsl_name, "pow");
    }

    // -- Vector/geometric resolution --

    #[test]
    fn resolve_length_vec3() {
        let reg = BuiltinFunctionRegistry::new();
        let sig = reg
            .resolve("length", &[WgslType::Vector(ScalarType::F32, 3)])
            .unwrap();
        assert_eq!(sig.wgsl_name, "length");
        assert_eq!(
            sig.resolve_return_type(&[WgslType::Vector(ScalarType::F32, 3)]),
            WgslType::Scalar(ScalarType::F32)
        );
    }

    #[test]
    fn resolve_dot_vec3() {
        let reg = BuiltinFunctionRegistry::new();
        let v3 = WgslType::Vector(ScalarType::F32, 3);
        let sig = reg.resolve("dot", &[v3.clone(), v3]).unwrap();
        assert_eq!(sig.wgsl_name, "dot");
    }

    #[test]
    fn resolve_cross_vec3() {
        let reg = BuiltinFunctionRegistry::new();
        let v3 = WgslType::Vector(ScalarType::F32, 3);
        let sig = reg.resolve("cross", &[v3.clone(), v3.clone()]).unwrap();
        assert_eq!(sig.wgsl_name, "cross");
        assert_eq!(sig.resolve_return_type(&[v3.clone(), v3.clone()]), v3);
    }

    #[test]
    fn resolve_normalize_vec2() {
        let reg = BuiltinFunctionRegistry::new();
        let v2 = WgslType::Vector(ScalarType::F32, 2);
        let sig = reg.resolve("normalize", &[v2.clone()]).unwrap();
        assert_eq!(sig.resolve_return_type(&[v2.clone()]), v2);
    }

    #[test]
    fn resolve_distance_vec4() {
        let reg = BuiltinFunctionRegistry::new();
        let v4 = WgslType::Vector(ScalarType::F32, 4);
        let sig = reg.resolve("distance", &[v4.clone(), v4.clone()]).unwrap();
        assert_eq!(
            sig.resolve_return_type(&[v4.clone(), v4]),
            WgslType::Scalar(ScalarType::F32)
        );
    }

    #[test]
    fn resolve_reflect_vec3() {
        let reg = BuiltinFunctionRegistry::new();
        let v3 = WgslType::Vector(ScalarType::F32, 3);
        let sig = reg.resolve("reflect", &[v3.clone(), v3.clone()]).unwrap();
        assert_eq!(sig.resolve_return_type(&[v3.clone(), v3.clone()]), v3);
    }

    // -- Matrix resolution --

    #[test]
    fn resolve_transpose_mat4() {
        let reg = BuiltinFunctionRegistry::new();
        let m4 = WgslType::Matrix(ScalarType::F32, 4, 4);
        let sig = reg.resolve("transpose", &[m4.clone()]).unwrap();
        assert_eq!(sig.resolve_return_type(&[m4.clone()]), m4);
    }

    #[test]
    fn resolve_determinant_mat3() {
        let reg = BuiltinFunctionRegistry::new();
        let m3 = WgslType::Matrix(ScalarType::F32, 3, 3);
        let sig = reg.resolve("determinant", &[m3]).unwrap();
        assert_eq!(
            sig.resolve_return_type(&[WgslType::Matrix(ScalarType::F32, 3, 3)]),
            WgslType::Scalar(ScalarType::F32)
        );
    }

    // -- Derivative functions --

    #[test]
    fn resolve_dpdx() {
        let reg = BuiltinFunctionRegistry::new();
        let sig = reg
            .resolve("dpdx", &[WgslType::Scalar(ScalarType::F32)])
            .unwrap();
        assert_eq!(sig.wgsl_name, "dpdx");
    }

    #[test]
    fn resolve_fwidth_vec2() {
        let reg = BuiltinFunctionRegistry::new();
        let v2 = WgslType::Vector(ScalarType::F32, 2);
        let sig = reg.resolve("fwidth", &[v2.clone()]).unwrap();
        assert_eq!(sig.resolve_return_type(&[v2.clone()]), v2);
    }

    // -- Pack/unpack --

    #[test]
    fn resolve_pack4x8snorm() {
        let reg = BuiltinFunctionRegistry::new();
        let sig = reg
            .resolve("pack4x8snorm", &[WgslType::Vector(ScalarType::F32, 4)])
            .unwrap();
        assert_eq!(
            sig.resolve_return_type(&[WgslType::Vector(ScalarType::F32, 4)]),
            WgslType::Scalar(ScalarType::U32)
        );
    }

    #[test]
    fn resolve_unpack4x8snorm() {
        let reg = BuiltinFunctionRegistry::new();
        let sig = reg
            .resolve("unpack4x8snorm", &[WgslType::Scalar(ScalarType::U32)])
            .unwrap();
        assert_eq!(
            sig.resolve_return_type(&[WgslType::Scalar(ScalarType::U32)]),
            WgslType::Vector(ScalarType::F32, 4)
        );
    }

    // -- Bit manipulation --

    #[test]
    fn resolve_count_one_bits() {
        let reg = BuiltinFunctionRegistry::new();
        let sig = reg
            .resolve("countOneBits", &[WgslType::Scalar(ScalarType::U32)])
            .unwrap();
        assert_eq!(sig.wgsl_name, "countOneBits");
    }

    // -- Logical --

    #[test]
    fn resolve_select() {
        let reg = BuiltinFunctionRegistry::new();
        let f32_type = WgslType::Scalar(ScalarType::F32);
        let sig = reg
            .resolve(
                "select",
                &[
                    f32_type.clone(),
                    f32_type.clone(),
                    WgslType::Scalar(ScalarType::Bool),
                ],
            )
            .unwrap();
        assert_eq!(sig.wgsl_name, "select");
    }

    // -- Barrier --

    #[test]
    fn resolve_workgroup_barrier() {
        let reg = BuiltinFunctionRegistry::new();
        let sig = reg.resolve("workgroupBarrier", &[]).unwrap();
        assert_eq!(sig.resolve_return_type(&[]), WgslType::Void);
    }

    // -- Error cases --

    #[test]
    fn unknown_function_returns_error() {
        let reg = BuiltinFunctionRegistry::new();
        let result = reg.resolve("unknown_func", &[WgslType::Scalar(ScalarType::F32)]);
        assert!(matches!(
            result,
            Err(FunctionResolutionError::FunctionNotFound { .. })
        ));
    }

    #[test]
    fn wrong_arg_types_returns_error() {
        let reg = BuiltinFunctionRegistry::new();
        // sin(bool) should fail
        let result = reg.resolve("sin", &[WgslType::Scalar(ScalarType::Bool)]);
        assert!(matches!(
            result,
            Err(FunctionResolutionError::NoMatchingOverload { .. })
        ));
    }

    #[test]
    fn wrong_arg_count_returns_error() {
        let reg = BuiltinFunctionRegistry::new();
        // sin() with no args should fail
        let result = reg.resolve("sin", &[]);
        assert!(matches!(
            result,
            Err(FunctionResolutionError::NoMatchingOverload { .. })
        ));
    }

    // -- Category queries --

    #[test]
    fn list_trig_functions() {
        let reg = BuiltinFunctionRegistry::new();
        let trig = reg.functions_in_category(FunctionCategory::Trigonometric);
        assert!(trig.contains(&"sin"));
        assert!(trig.contains(&"cos"));
        assert!(trig.contains(&"tan"));
        assert!(trig.contains(&"atan2"));
    }

    #[test]
    fn list_geometric_functions() {
        let reg = BuiltinFunctionRegistry::new();
        let geo = reg.functions_in_category(FunctionCategory::Geometric);
        assert!(geo.contains(&"length"));
        assert!(geo.contains(&"normalize"));
        assert!(geo.contains(&"dot"));
        assert!(geo.contains(&"cross"));
        assert!(geo.contains(&"distance"));
        assert!(geo.contains(&"reflect"));
    }

    #[test]
    fn list_derivative_functions() {
        let reg = BuiltinFunctionRegistry::new();
        let deriv = reg.functions_in_category(FunctionCategory::Derivative);
        assert!(deriv.contains(&"dpdx"));
        assert!(deriv.contains(&"dpdy"));
        assert!(deriv.contains(&"fwidth"));
    }

    // -- Utility queries --

    #[test]
    fn has_function_works() {
        let reg = BuiltinFunctionRegistry::new();
        assert!(reg.has_function("sin"));
        assert!(reg.has_function("dot"));
        assert!(!reg.has_function("nonexistent"));
    }

    #[test]
    fn list_overloads_works() {
        let reg = BuiltinFunctionRegistry::new();
        let overloads = reg.list_overloads("sin");
        assert!(!overloads.is_empty());
    }

    // -- Interpolation functions --

    #[test]
    fn resolve_mix() {
        let reg = BuiltinFunctionRegistry::new();
        let f32_type = WgslType::Scalar(ScalarType::F32);
        let sig = reg
            .resolve("mix", &[f32_type.clone(), f32_type.clone(), f32_type])
            .unwrap();
        assert_eq!(sig.wgsl_name, "mix");
    }

    #[test]
    fn resolve_smoothstep() {
        let reg = BuiltinFunctionRegistry::new();
        let f32_type = WgslType::Scalar(ScalarType::F32);
        let sig = reg
            .resolve(
                "smoothstep",
                &[f32_type.clone(), f32_type.clone(), f32_type],
            )
            .unwrap();
        assert_eq!(sig.wgsl_name, "smoothstep");
    }

    #[test]
    fn resolve_step() {
        let reg = BuiltinFunctionRegistry::new();
        let f32_type = WgslType::Scalar(ScalarType::F32);
        let sig = reg.resolve("step", &[f32_type.clone(), f32_type]).unwrap();
        assert_eq!(sig.wgsl_name, "step");
    }

    // -- Error display --

    #[test]
    fn error_display_function_not_found() {
        let err = FunctionResolutionError::FunctionNotFound {
            name: "sine".to_string(),
            available: vec!["sin".to_string()],
        };
        let msg = err.to_string();
        assert!(msg.contains("sine"));
        assert!(msg.contains("sin"));
    }

    #[test]
    fn error_display_no_matching_overload() {
        let err = FunctionResolutionError::NoMatchingOverload {
            name: "sin".to_string(),
            arg_types: vec![WgslType::Scalar(ScalarType::Bool)],
            available_overloads: vec![],
        };
        let msg = err.to_string();
        assert!(msg.contains("sin"));
        assert!(msg.contains("bool"));
    }

    // -- Edit distance --

    #[test]
    fn edit_distance_identical() {
        assert_eq!(super::edit_distance("sin", "sin"), 0);
    }

    #[test]
    fn edit_distance_one_char() {
        assert_eq!(super::edit_distance("sin", "sit"), 1);
    }

    #[test]
    fn edit_distance_different() {
        assert!(super::edit_distance("sin", "normalize") > 3);
    }
}
