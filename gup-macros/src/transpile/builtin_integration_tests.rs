// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests verifying the built-in function registry works
//! end-to-end with the converter and codegen pipeline.

#[cfg(test)]
mod tests {
    use crate::transpile::builtins::{
        BuiltinFunctionRegistry, FunctionCategory, FunctionResolutionError,
    };
    use crate::transpile::{RustToWgsl, WgslCodeGen};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn transpile_expr(rust_expr: &str) -> String {
        let expr: syn::Expr = syn::parse_str(rust_expr).expect("Failed to parse expression");
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let wgsl_expr = converter
            .convert_expr(&expr)
            .expect("Failed to convert expression");
        let codegen = WgslCodeGen::new();
        codegen.generate_expr(&wgsl_expr)
    }

    fn transpile_fn(code: &str) -> String {
        let func: syn::ItemFn = syn::parse_str(code).expect("Failed to parse function");
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let wgsl_func = converter
            .convert_function(&func)
            .expect("Failed to convert function");
        let mut codegen = WgslCodeGen::new();
        codegen.generate_function(&wgsl_func)
    }

    // ===================================================================
    // AC1: Mathematical functions — transpile pipeline integration
    // ===================================================================

    #[test]
    fn trig_method_chain() {
        assert_eq!(transpile_expr("x.sin()"), "sin(x)");
        assert_eq!(transpile_expr("x.cos()"), "cos(x)");
        assert_eq!(transpile_expr("x.tan()"), "tan(x)");
        assert_eq!(transpile_expr("x.asin()"), "asin(x)");
        assert_eq!(transpile_expr("x.acos()"), "acos(x)");
        assert_eq!(transpile_expr("x.atan()"), "atan(x)");
    }

    #[test]
    fn hyperbolic_functions() {
        assert_eq!(transpile_expr("x.sinh()"), "sinh(x)");
        assert_eq!(transpile_expr("x.cosh()"), "cosh(x)");
        assert_eq!(transpile_expr("x.tanh()"), "tanh(x)");
        assert_eq!(transpile_expr("x.asinh()"), "asinh(x)");
        assert_eq!(transpile_expr("x.acosh()"), "acosh(x)");
        assert_eq!(transpile_expr("x.atanh()"), "atanh(x)");
    }

    #[test]
    fn exp_log_functions() {
        assert_eq!(transpile_expr("x.exp()"), "exp(x)");
        assert_eq!(transpile_expr("x.exp2()"), "exp2(x)");
        assert_eq!(transpile_expr("x.log()"), "log(x)");
        assert_eq!(transpile_expr("x.log2()"), "log2(x)");
        assert_eq!(transpile_expr("x.sqrt()"), "sqrt(x)");
        assert_eq!(transpile_expr("x.inversesqrt()"), "inversesqrt(x)");
    }

    #[test]
    fn power_functions() {
        assert_eq!(transpile_expr("x.pow(y)"), "pow(x, y)");
        assert_eq!(transpile_expr("f32::pow(x, y)"), "pow(x, y)");
    }

    #[test]
    fn rounding_functions() {
        assert_eq!(transpile_expr("x.floor()"), "floor(x)");
        assert_eq!(transpile_expr("x.ceil()"), "ceil(x)");
        assert_eq!(transpile_expr("x.round()"), "round(x)");
        assert_eq!(transpile_expr("x.trunc()"), "trunc(x)");
        assert_eq!(transpile_expr("x.fract()"), "fract(x)");
    }

    #[test]
    fn utility_functions() {
        assert_eq!(transpile_expr("x.abs()"), "abs(x)");
        assert_eq!(transpile_expr("x.sign()"), "sign(x)");
        assert_eq!(transpile_expr("x.saturate()"), "saturate(x)");
        assert_eq!(transpile_expr("x.degrees()"), "degrees(x)");
        assert_eq!(transpile_expr("x.radians()"), "radians(x)");
    }

    #[test]
    fn clamp_and_minmax() {
        assert_eq!(transpile_expr("x.min(y)"), "min(x, y)");
        assert_eq!(transpile_expr("x.max(y)"), "max(x, y)");
        assert_eq!(transpile_expr("x.clamp(lo, hi)"), "clamp(x, lo, hi)");
    }

    #[test]
    fn interpolation_functions() {
        assert_eq!(transpile_expr("a.mix(b, t)"), "mix(a, b, t)");
        assert_eq!(transpile_expr("a.step(b)"), "step(a, b)");
        assert_eq!(
            transpile_expr("x.smoothstep(edge0, edge1)"),
            "smoothstep(x, edge0, edge1)"
        );
    }

    #[test]
    fn fma_function() {
        assert_eq!(transpile_expr("a.fma(b, c)"), "fma(a, b, c)");
    }

    // ===================================================================
    // AC2: Vector and matrix operations
    // ===================================================================

    #[test]
    fn vector_length_and_normalize() {
        assert_eq!(transpile_expr("v.length()"), "length(v)");
        assert_eq!(transpile_expr("v.normalize()"), "normalize(v)");
    }

    #[test]
    fn vector_dot_cross() {
        assert_eq!(transpile_expr("a.dot(b)"), "dot(a, b)");
        assert_eq!(transpile_expr("a.cross(b)"), "cross(a, b)");
    }

    #[test]
    fn vector_geometric() {
        assert_eq!(transpile_expr("a.distance(b)"), "distance(a, b)");
        assert_eq!(transpile_expr("v.reflect(n)"), "reflect(v, n)");
        assert_eq!(transpile_expr("v.refract(n, eta)"), "refract(v, n, eta)");
        assert_eq!(
            transpile_expr("v.faceforward(e2, e_ref)"),
            "faceforward(v, e2, e_ref)"
        );
    }

    #[test]
    fn vector_swizzle_component_access() {
        assert_eq!(transpile_expr("v.x()"), "v.x");
        assert_eq!(transpile_expr("v.y()"), "v.y");
        assert_eq!(transpile_expr("v.z()"), "v.z");
        assert_eq!(transpile_expr("v.w()"), "v.w");
    }

    // ===================================================================
    // AC3: GPU-specific functions — registry coverage
    // ===================================================================

    #[test]
    fn derivative_functions_in_registry() {
        let reg = BuiltinFunctionRegistry::new();
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
            assert!(
                reg.has_function(name),
                "Registry should have derivative function: {name}"
            );
        }
    }

    #[test]
    fn texture_functions_in_registry() {
        let reg = BuiltinFunctionRegistry::new();
        for name in &[
            "textureSample",
            "textureSampleLevel",
            "textureSampleBias",
            "textureSampleGrad",
            "textureLoad",
            "textureStore",
            "textureDimensions",
            "textureNumLevels",
        ] {
            assert!(
                reg.has_function(name),
                "Registry should have texture function: {name}"
            );
        }
    }

    #[test]
    fn atomic_functions_in_registry() {
        let reg = BuiltinFunctionRegistry::new();
        for name in &[
            "atomicLoad",
            "atomicStore",
            "atomicAdd",
            "atomicSub",
            "atomicMax",
            "atomicMin",
            "atomicAnd",
            "atomicOr",
            "atomicXor",
            "atomicExchange",
            "atomicCompareExchangeWeak",
        ] {
            assert!(
                reg.has_function(name),
                "Registry should have atomic function: {name}"
            );
        }
    }

    #[test]
    fn barrier_functions_in_registry() {
        let reg = BuiltinFunctionRegistry::new();
        for name in &[
            "storageBarrier",
            "workgroupBarrier",
            "textureBarrier",
            "workgroupUniformLoad",
        ] {
            assert!(
                reg.has_function(name),
                "Registry should have barrier function: {name}"
            );
        }
    }

    #[test]
    fn pack_unpack_functions_in_registry() {
        let reg = BuiltinFunctionRegistry::new();
        for name in &[
            "pack4x8snorm",
            "pack4x8unorm",
            "pack2x16snorm",
            "pack2x16unorm",
            "pack2x16float",
            "unpack4x8snorm",
            "unpack4x8unorm",
            "unpack2x16snorm",
            "unpack2x16unorm",
            "unpack2x16float",
        ] {
            assert!(
                reg.has_function(name),
                "Registry should have pack/unpack function: {name}"
            );
        }
    }

    // ===================================================================
    // AC4: Type-safe overload resolution
    // ===================================================================

    #[test]
    fn overload_resolution_selects_correct_overload() {
        let reg = BuiltinFunctionRegistry::new();

        // abs(f32) → f32
        let sig = reg
            .resolve(
                "abs",
                &[crate::transpile::WgslType::Scalar(
                    crate::transpile::ScalarType::F32,
                )],
            )
            .unwrap();
        assert_eq!(sig.wgsl_name, "abs");

        // abs(vec3<f32>) → vec3<f32>
        let sig = reg
            .resolve(
                "abs",
                &[crate::transpile::WgslType::Vector(
                    crate::transpile::ScalarType::F32,
                    3,
                )],
            )
            .unwrap();
        assert_eq!(sig.wgsl_name, "abs");
    }

    #[test]
    fn overload_resolution_error_on_bool() {
        let reg = BuiltinFunctionRegistry::new();
        let result = reg.resolve(
            "sin",
            &[crate::transpile::WgslType::Scalar(
                crate::transpile::ScalarType::Bool,
            )],
        );
        assert!(matches!(
            result,
            Err(FunctionResolutionError::NoMatchingOverload { .. })
        ));
    }

    #[test]
    fn overload_resolution_error_unknown_function() {
        let reg = BuiltinFunctionRegistry::new();
        let result = reg.resolve(
            "nonexistent",
            &[crate::transpile::WgslType::Scalar(
                crate::transpile::ScalarType::F32,
            )],
        );
        assert!(matches!(
            result,
            Err(FunctionResolutionError::FunctionNotFound { .. })
        ));
    }

    // ===================================================================
    // End-to-end pipeline tests — complex shader functions
    // ===================================================================

    #[test]
    fn complex_math_expression_pipeline() {
        let wgsl = transpile_fn(
            r#"
            fn compute(x: f32) -> f32 {
                let y = x.sin() * x.cos();
                let z = y.abs().sqrt();
                return z.clamp(0.0, 1.0)
            }
        "#,
        );
        assert!(wgsl.contains("sin(x) * cos(x)"), "got:\n{wgsl}");
        assert!(wgsl.contains("sqrt(abs(y))"), "got:\n{wgsl}");
        assert!(wgsl.contains("clamp(z, 0.0, 1.0)"), "got:\n{wgsl}");
    }

    #[test]
    fn vector_operations_pipeline() {
        let wgsl = transpile_fn(
            r#"
            fn process(v: Vec3) -> f32 {
                let n = v.normalize();
                let d = n.dot(Vec3(0.0, 1.0, 0.0));
                return d.abs()
            }
        "#,
        );
        assert!(wgsl.contains("normalize(v)"), "got:\n{wgsl}");
        assert!(
            wgsl.contains("dot(n, vec3<f32>(0.0, 1.0, 0.0))"),
            "got:\n{wgsl}"
        );
        assert!(wgsl.contains("abs(d)"), "got:\n{wgsl}");
    }

    #[test]
    fn interpolation_pipeline() {
        let wgsl = transpile_fn(
            r#"
            fn blend(a: f32, b: f32, t: f32) -> f32 {
                let smooth_t = t.smoothstep(0.0, 1.0);
                return a.mix(b, smooth_t)
            }
        "#,
        );
        assert!(wgsl.contains("smoothstep(t, 0.0, 1.0)"), "got:\n{wgsl}");
        assert!(wgsl.contains("mix(a, b, smooth_t)"), "got:\n{wgsl}");
    }

    // ===================================================================
    // Category and coverage queries
    // ===================================================================

    #[test]
    fn all_categories_have_functions() {
        let reg = BuiltinFunctionRegistry::new();
        for category in &[
            FunctionCategory::Trigonometric,
            FunctionCategory::Exponential,
            FunctionCategory::MathUtility,
            FunctionCategory::Interpolation,
            FunctionCategory::Geometric,
            FunctionCategory::Matrix,
            FunctionCategory::Derivative,
            FunctionCategory::Texture,
            FunctionCategory::Atomic,
            FunctionCategory::Barrier,
            FunctionCategory::PackUnpack,
            FunctionCategory::Logical,
            FunctionCategory::BitManipulation,
        ] {
            let funcs = reg.functions_in_category(*category);
            assert!(
                !funcs.is_empty(),
                "Category {category:?} should have at least one function"
            );
        }
    }

    #[test]
    fn registry_has_comprehensive_coverage() {
        let reg = BuiltinFunctionRegistry::new();
        // Verify we have a substantial library
        assert!(
            reg.function_count() >= 50,
            "Expected at least 50 functions, got {}",
            reg.function_count()
        );
    }
}
