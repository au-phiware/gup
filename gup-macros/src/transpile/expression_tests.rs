// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive expression transpilation tests for GUP-057.
//!
//! Tests cover all acceptance criteria:
//! - AC1: Arithmetic and logical operators
//! - AC2: Variable access and assignment
//! - AC3: Function calls and methods
//! - AC4: Complex expressions

#[cfg(test)]
mod tests {
    use crate::transpile::{RustToWgsl, WgslCodeGen};

    /// Helper to run the full transpile pipeline on a Rust function.
    fn transpile(func: &syn::ItemFn, uniform_params: impl IntoIterator<Item = String>) -> String {
        let mut converter = RustToWgsl::new(uniform_params);
        let wgsl_func = converter.convert_function(func).unwrap();
        let mut codegen = WgslCodeGen::new();
        codegen.generate_function(&wgsl_func)
    }

    /// Helper to convert a single expression and generate WGSL text.
    fn transpile_expr(rust_expr: &str) -> String {
        let expr: syn::Expr = syn::parse_str(rust_expr).expect("Failed to parse expression");
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let wgsl_expr = converter
            .convert_expr(&expr)
            .expect("Failed to convert expression");
        let codegen = WgslCodeGen::new();
        codegen.generate_expr(&wgsl_expr)
    }

    /// Helper that expects a transpilation error.
    fn transpile_expr_err(rust_expr: &str) -> String {
        let expr: syn::Expr = syn::parse_str(rust_expr).expect("Failed to parse expression");
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        converter
            .convert_expr(&expr)
            .expect_err("Expected error")
            .message
    }

    // ===================================================================
    // AC1: Arithmetic and Logical Operators
    // ===================================================================

    #[test]
    fn ac1_basic_addition() {
        assert_eq!(transpile_expr("a + b"), "a + b");
    }

    #[test]
    fn ac1_basic_subtraction() {
        assert_eq!(transpile_expr("a - b"), "a - b");
    }

    #[test]
    fn ac1_basic_multiplication() {
        assert_eq!(transpile_expr("a * b"), "a * b");
    }

    #[test]
    fn ac1_basic_division() {
        assert_eq!(transpile_expr("a / b"), "a / b");
    }

    #[test]
    fn ac1_basic_modulo() {
        assert_eq!(transpile_expr("a % b"), "a % b");
    }

    #[test]
    fn ac1_comparison_equal() {
        assert_eq!(transpile_expr("a == b"), "a == b");
    }

    #[test]
    fn ac1_comparison_not_equal() {
        assert_eq!(transpile_expr("a != b"), "a != b");
    }

    #[test]
    fn ac1_comparison_less() {
        assert_eq!(transpile_expr("a < b"), "a < b");
    }

    #[test]
    fn ac1_comparison_less_equal() {
        assert_eq!(transpile_expr("a <= b"), "a <= b");
    }

    #[test]
    fn ac1_comparison_greater() {
        assert_eq!(transpile_expr("a > b"), "a > b");
    }

    #[test]
    fn ac1_comparison_greater_equal() {
        assert_eq!(transpile_expr("a >= b"), "a >= b");
    }

    #[test]
    fn ac1_logical_and() {
        assert_eq!(transpile_expr("a && b"), "a && b");
    }

    #[test]
    fn ac1_logical_or() {
        assert_eq!(transpile_expr("a || b"), "a || b");
    }

    #[test]
    fn ac1_logical_not() {
        assert_eq!(transpile_expr("!a"), "!a");
    }

    #[test]
    fn ac1_bitwise_and() {
        assert_eq!(transpile_expr("a & b"), "a & b");
    }

    #[test]
    fn ac1_bitwise_or() {
        assert_eq!(transpile_expr("a | b"), "a | b");
    }

    #[test]
    fn ac1_bitwise_xor() {
        assert_eq!(transpile_expr("a ^ b"), "a ^ b");
    }

    #[test]
    fn ac1_bitwise_shl() {
        assert_eq!(transpile_expr("a << b"), "a << b");
    }

    #[test]
    fn ac1_bitwise_shr() {
        assert_eq!(transpile_expr("a >> b"), "a >> b");
    }

    #[test]
    fn ac1_operator_precedence_mul_before_add() {
        // Rust parses `a + b * c` as `a + (b * c)` — check it stays correct
        assert_eq!(transpile_expr("a + b * c"), "a + b * c");
    }

    #[test]
    fn ac1_operator_precedence_parenthesised() {
        assert_eq!(transpile_expr("(a + b) * c"), "(a + b) * c");
    }

    #[test]
    fn ac1_unary_negate() {
        assert_eq!(transpile_expr("-x"), "-x");
    }

    #[test]
    fn ac1_complex_boolean_expression() {
        assert_eq!(transpile_expr("a > 0.0 && b < 1.0"), "a > 0.0 && b < 1.0");
    }

    // ===================================================================
    // AC2: Variable Access and Assignment
    // ===================================================================

    #[test]
    fn ac2_local_variable_reference() {
        assert_eq!(transpile_expr("x"), "x");
    }

    #[test]
    fn ac2_struct_field_access() {
        assert_eq!(transpile_expr("point.x"), "point.x");
    }

    #[test]
    fn ac2_nested_struct_field_access() {
        assert_eq!(transpile_expr("config.inner.value"), "config.inner.value");
    }

    #[test]
    fn ac2_array_indexing() {
        assert_eq!(transpile_expr("arr[i]"), "arr[i]");
    }

    #[test]
    fn ac2_array_indexing_literal() {
        assert_eq!(transpile_expr("arr[0]"), "arr[0]");
    }

    #[test]
    fn ac2_uniform_parameter_access() {
        let expr: syn::Expr = syn::parse_str("scale").unwrap();
        let mut converter = RustToWgsl::new(["scale".to_string()]);
        let wgsl_expr = converter.convert_expr(&expr).unwrap();
        let codegen = WgslCodeGen::new();
        assert_eq!(codegen.generate_expr(&wgsl_expr), "uniforms.scale");
    }

    #[test]
    fn ac2_uniform_field_access() {
        let expr: syn::Expr = syn::parse_str("config.min_val").unwrap();
        let mut converter = RustToWgsl::new(["config".to_string()]);
        let wgsl_expr = converter.convert_expr(&expr).unwrap();
        let codegen = WgslCodeGen::new();
        assert_eq!(codegen.generate_expr(&wgsl_expr), "uniforms.config.min_val");
    }

    #[test]
    fn ac2_mutable_variable() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut acc = 0.0;
                return acc;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(
            wgsl.contains("var acc = 0.0;"),
            "Mutable should use 'var', got:\n{wgsl}"
        );
    }

    #[test]
    fn ac2_immutable_variable() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let value = x * 2.0;
                return value;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(
            wgsl.contains("let value = x * 2.0;"),
            "Immutable should use 'let', got:\n{wgsl}"
        );
    }

    #[test]
    fn ac2_assignment_statement() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut y = 0.0;
                y = x * 2.0;
                return y;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(
            wgsl.contains("y = x * 2.0;"),
            "Should contain assignment, got:\n{wgsl}"
        );
    }

    #[test]
    fn ac2_compound_assignment_add() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut acc = 0.0;
                acc += x;
                return acc;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(
            wgsl.contains("acc += x;"),
            "Should contain compound assignment, got:\n{wgsl}"
        );
    }

    #[test]
    fn ac2_compound_assignment_sub() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut acc = 10.0;
                acc -= x;
                return acc;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(
            wgsl.contains("acc -= x;"),
            "Should contain -= assignment, got:\n{wgsl}"
        );
    }

    #[test]
    fn ac2_compound_assignment_mul() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut acc = 1.0;
                acc *= x;
                return acc;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(
            wgsl.contains("acc *= x;"),
            "Should contain *= assignment, got:\n{wgsl}"
        );
    }

    #[test]
    fn ac2_compound_assignment_div() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let mut acc = 100.0;
                acc /= x;
                return acc;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(
            wgsl.contains("acc /= x;"),
            "Should contain /= assignment, got:\n{wgsl}"
        );
    }

    #[test]
    fn ac2_compound_assignment_bitwise() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: i32) -> i32 {
                let mut mask = 0xFF;
                mask &= x;
                return mask;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(
            wgsl.contains("mask &= x;"),
            "Should contain &= assignment, got:\n{wgsl}"
        );
    }

    // ===================================================================
    // AC3: Function Calls and Methods
    // ===================================================================

    #[test]
    fn ac3_direct_function_call() {
        assert_eq!(transpile_expr("clamp(x, 0.0, 1.0)"), "clamp(x, 0.0, 1.0)");
    }

    #[test]
    fn ac3_method_abs() {
        assert_eq!(transpile_expr("x.abs()"), "abs(x)");
    }

    #[test]
    fn ac3_method_sqrt() {
        assert_eq!(transpile_expr("x.sqrt()"), "sqrt(x)");
    }

    #[test]
    fn ac3_method_sin() {
        assert_eq!(transpile_expr("x.sin()"), "sin(x)");
    }

    #[test]
    fn ac3_method_cos() {
        assert_eq!(transpile_expr("x.cos()"), "cos(x)");
    }

    #[test]
    fn ac3_method_floor() {
        assert_eq!(transpile_expr("x.floor()"), "floor(x)");
    }

    #[test]
    fn ac3_method_ceil() {
        assert_eq!(transpile_expr("x.ceil()"), "ceil(x)");
    }

    #[test]
    fn ac3_method_round() {
        assert_eq!(transpile_expr("x.round()"), "round(x)");
    }

    #[test]
    fn ac3_method_fract() {
        assert_eq!(transpile_expr("x.fract()"), "fract(x)");
    }

    #[test]
    fn ac3_method_sign() {
        assert_eq!(transpile_expr("x.sign()"), "sign(x)");
    }

    #[test]
    fn ac3_method_length() {
        assert_eq!(transpile_expr("v.length()"), "length(v)");
    }

    #[test]
    fn ac3_method_normalize() {
        assert_eq!(transpile_expr("v.normalize()"), "normalize(v)");
    }

    #[test]
    fn ac3_method_dot() {
        assert_eq!(transpile_expr("a.dot(b)"), "dot(a, b)");
    }

    #[test]
    fn ac3_method_cross() {
        assert_eq!(transpile_expr("a.cross(b)"), "cross(a, b)");
    }

    #[test]
    fn ac3_method_min() {
        assert_eq!(transpile_expr("a.min(b)"), "min(a, b)");
    }

    #[test]
    fn ac3_method_max() {
        assert_eq!(transpile_expr("a.max(b)"), "max(a, b)");
    }

    #[test]
    fn ac3_method_pow() {
        assert_eq!(transpile_expr("a.pow(b)"), "pow(a, b)");
    }

    #[test]
    fn ac3_method_clamp() {
        assert_eq!(transpile_expr("x.clamp(lo, hi)"), "clamp(x, lo, hi)");
    }

    #[test]
    fn ac3_method_mix() {
        assert_eq!(transpile_expr("a.mix(b, t)"), "mix(a, b, t)");
    }

    #[test]
    fn ac3_method_smoothstep() {
        assert_eq!(
            transpile_expr("x.smoothstep(lo, hi)"),
            "smoothstep(x, lo, hi)"
        );
    }

    #[test]
    fn ac3_method_distance() {
        assert_eq!(transpile_expr("a.distance(b)"), "distance(a, b)");
    }

    #[test]
    fn ac3_method_reflect() {
        assert_eq!(transpile_expr("v.reflect(n)"), "reflect(v, n)");
    }

    #[test]
    fn ac3_method_saturate() {
        assert_eq!(transpile_expr("x.saturate()"), "saturate(x)");
    }

    #[test]
    fn ac3_method_degrees() {
        assert_eq!(transpile_expr("x.degrees()"), "degrees(x)");
    }

    #[test]
    fn ac3_method_radians() {
        assert_eq!(transpile_expr("x.radians()"), "radians(x)");
    }

    #[test]
    fn ac3_method_trig_extended() {
        assert_eq!(transpile_expr("x.sinh()"), "sinh(x)");
        assert_eq!(transpile_expr("x.cosh()"), "cosh(x)");
        assert_eq!(transpile_expr("x.tanh()"), "tanh(x)");
        assert_eq!(transpile_expr("x.asinh()"), "asinh(x)");
        assert_eq!(transpile_expr("x.acosh()"), "acosh(x)");
        assert_eq!(transpile_expr("x.atanh()"), "atanh(x)");
    }

    #[test]
    fn ac3_qualified_f32_sin() {
        assert_eq!(transpile_expr("f32::sin(x)"), "sin(x)");
    }

    #[test]
    fn ac3_qualified_f32_cos() {
        assert_eq!(transpile_expr("f32::cos(x)"), "cos(x)");
    }

    #[test]
    fn ac3_qualified_f32_sqrt() {
        assert_eq!(transpile_expr("f32::sqrt(x)"), "sqrt(x)");
    }

    #[test]
    fn ac3_qualified_f32_abs() {
        assert_eq!(transpile_expr("f32::abs(x)"), "abs(x)");
    }

    #[test]
    fn ac3_qualified_f32_min() {
        assert_eq!(transpile_expr("f32::min(a, b)"), "min(a, b)");
    }

    #[test]
    fn ac3_qualified_f32_max() {
        assert_eq!(transpile_expr("f32::max(a, b)"), "max(a, b)");
    }

    #[test]
    fn ac3_qualified_f32_clamp() {
        assert_eq!(transpile_expr("f32::clamp(x, lo, hi)"), "clamp(x, lo, hi)");
    }

    #[test]
    fn ac3_qualified_f32_pow() {
        assert_eq!(transpile_expr("f32::pow(a, b)"), "pow(a, b)");
    }

    #[test]
    fn ac3_vec2_constructor() {
        assert_eq!(transpile_expr("Vec2(1.0, 2.0)"), "vec2<f32>(1.0, 2.0)");
    }

    #[test]
    fn ac3_vec3_constructor() {
        assert_eq!(
            transpile_expr("Vec3(1.0, 2.0, 3.0)"),
            "vec3<f32>(1.0, 2.0, 3.0)"
        );
    }

    #[test]
    fn ac3_vec4_constructor() {
        assert_eq!(
            transpile_expr("Vec4(1.0, 2.0, 3.0, 4.0)"),
            "vec4<f32>(1.0, 2.0, 3.0, 4.0)"
        );
    }

    #[test]
    fn ac3_ivec_constructors() {
        assert_eq!(transpile_expr("IVec2(1, 2)"), "vec2<i32>(1, 2)");
        assert_eq!(transpile_expr("IVec3(1, 2, 3)"), "vec3<i32>(1, 2, 3)");
        assert_eq!(transpile_expr("IVec4(1, 2, 3, 4)"), "vec4<i32>(1, 2, 3, 4)");
    }

    #[test]
    fn ac3_uvec_constructors() {
        assert_eq!(transpile_expr("UVec2(1, 2)"), "vec2<u32>(1, 2)");
        assert_eq!(transpile_expr("UVec3(1, 2, 3)"), "vec3<u32>(1, 2, 3)");
        assert_eq!(transpile_expr("UVec4(1, 2, 3, 4)"), "vec4<u32>(1, 2, 3, 4)");
    }

    #[test]
    fn ac3_bvec_constructors() {
        assert_eq!(
            transpile_expr("BVec2(true, false)"),
            "vec2<bool>(true, false)"
        );
    }

    #[test]
    fn ac3_matrix_constructors() {
        assert_eq!(
            transpile_expr("Mat2(a, b, c, d)"),
            "mat2x2<f32>(a, b, c, d)"
        );
        assert_eq!(transpile_expr("Mat3(a, b, c)"), "mat3x3<f32>(a, b, c)");
        assert_eq!(
            transpile_expr("Mat4(a, b, c, d)"),
            "mat4x4<f32>(a, b, c, d)"
        );
    }

    #[test]
    fn ac3_vec3_new_static_call() {
        assert_eq!(
            transpile_expr("Vec3::new(1.0, 2.0, 3.0)"),
            "vec3<f32>(1.0, 2.0, 3.0)"
        );
    }

    #[test]
    fn ac3_vec3_splat_static_call() {
        assert_eq!(transpile_expr("Vec3::splat(1.0)"), "vec3<f32>(1.0)");
    }

    #[test]
    fn ac3_vec3_zero_static_call() {
        assert_eq!(transpile_expr("Vec3::zero()"), "vec3<f32>(0.0)");
    }

    #[test]
    fn ac3_vec3_one_static_call() {
        assert_eq!(transpile_expr("Vec3::one()"), "vec3<f32>(1.0)");
    }

    #[test]
    fn ac3_conversion_to_f32() {
        assert_eq!(transpile_expr("x.to_f32()"), "f32(x)");
    }

    #[test]
    fn ac3_conversion_to_i32() {
        assert_eq!(transpile_expr("x.to_i32()"), "i32(x)");
    }

    #[test]
    fn ac3_conversion_to_u32() {
        assert_eq!(transpile_expr("x.to_u32()"), "u32(x)");
    }

    #[test]
    fn ac3_unsupported_method_error() {
        let msg = transpile_expr_err("x.to_string()");
        assert!(
            msg.contains("to_string"),
            "Error should mention method: {msg}"
        );
    }

    // ===================================================================
    // AC4: Complex Expressions
    // ===================================================================

    #[test]
    fn ac4_nested_expressions() {
        assert_eq!(transpile_expr("(a + b) * (c - d)"), "(a + b) * (c - d)");
    }

    #[test]
    fn ac4_deeply_nested() {
        // Parentheses are preserved as they appear in Rust source
        let result = transpile_expr("((a + b) * c) / (d - (e + f))");
        assert!(
            result.contains("(a + b)") && result.contains("(d - (e + f))"),
            "Should preserve nested parens: {result}"
        );
    }

    #[test]
    fn ac4_conditional_select() {
        // if condition { value_a } else { value_b } → select(value_b, value_a, condition)
        assert_eq!(
            transpile_expr("if flag { a } else { b }"),
            "select(b, a, flag)"
        );
    }

    #[test]
    fn ac4_conditional_select_complex() {
        assert_eq!(
            transpile_expr("if x > 0.0 { x } else { -x }"),
            "select(-x, x, x > 0.0)"
        );
    }

    #[test]
    fn ac4_type_cast_f32() {
        assert_eq!(transpile_expr("x as f32"), "f32(x)");
    }

    #[test]
    fn ac4_type_cast_i32() {
        assert_eq!(transpile_expr("x as i32"), "i32(x)");
    }

    #[test]
    fn ac4_type_cast_u32() {
        assert_eq!(transpile_expr("x as u32"), "u32(x)");
    }

    #[test]
    fn ac4_chained_method_calls() {
        // (point1 - point2).length()
        assert_eq!(
            transpile_expr("(point1 - point2).length()"),
            "length((point1 - point2))"
        );
    }

    #[test]
    fn ac4_method_chain_abs_sqrt() {
        // x.abs().sqrt()
        assert_eq!(transpile_expr("x.abs().sqrt()"), "sqrt(abs(x))");
    }

    #[test]
    fn ac4_method_with_expression_arg() {
        // a.max(b * 2.0)
        assert_eq!(transpile_expr("a.max(b * 2.0)"), "max(a, b * 2.0)");
    }

    #[test]
    fn ac4_reference_stripping() {
        // &x should just be x in WGSL (no references in WGSL expressions)
        assert_eq!(transpile_expr("&x"), "x");
    }

    #[test]
    fn ac4_single_element_tuple() {
        assert_eq!(transpile_expr("(x,)"), "x");
    }

    #[test]
    fn ac4_multi_element_tuple_error() {
        let msg = transpile_expr_err("(x, y)");
        assert!(msg.contains("Tuples"), "Error should mention tuples: {msg}");
    }

    #[test]
    fn ac4_bool_literal_true() {
        assert_eq!(transpile_expr("true"), "true");
    }

    #[test]
    fn ac4_bool_literal_false() {
        assert_eq!(transpile_expr("false"), "false");
    }

    #[test]
    fn ac4_uint_literal() {
        assert_eq!(transpile_expr("42u32"), "42u");
    }

    #[test]
    fn ac4_float_literal_integer_value() {
        // Float with no decimal should still have .0
        let result = transpile_expr("42.0");
        assert!(
            result.contains('.'),
            "Float should have decimal point: {result}"
        );
    }

    // ===================================================================
    // Integration tests: full function transpilation
    // ===================================================================

    #[test]
    fn integration_vector_length_function() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn vector_magnitude(v: Vec3) -> f32 {
                v.length()
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("fn vector_magnitude(v: vec3<f32>) -> f32"));
        assert!(wgsl.contains("return length(v);"));
    }

    #[test]
    fn integration_normalize_function() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn safe_normalize(v: Vec3) -> Vec3 {
                let len = v.length();
                if len > 0.0 { v.normalize() } else { Vec3(0.0, 0.0, 0.0) }
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        // When if-else is the last expression in a function body, the
        // converter treats it as an if/else statement (each branch gets
        // an implicit return). This is valid WGSL.
        assert!(wgsl.contains("if (len > 0.0)"), "got:\n{wgsl}");
        assert!(
            wgsl.contains("normalize(v)"),
            "Should contain normalize call, got:\n{wgsl}"
        );
    }

    #[test]
    fn integration_compound_assignment_function() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn accumulate(value: f32, count: i32, factor: f32) -> f32 {
                let mut total = 0.0;
                total += value;
                total *= factor;
                return total;
            }
        };
        let wgsl = transpile(&func, ["count".to_string(), "factor".to_string()]);
        assert!(wgsl.contains("var total = 0.0;"), "got:\n{wgsl}");
        assert!(wgsl.contains("total += value;"), "got:\n{wgsl}");
        assert!(wgsl.contains("total *= uniforms.factor;"), "got:\n{wgsl}");
    }

    #[test]
    fn integration_if_else_statement() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn clamp_manual(x: f32) -> f32 {
                let mut result = x;
                if x > 1.0 {
                    result = 1.0;
                } else {
                    result = x;
                }
                return result;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("if (x > 1.0)"), "got:\n{wgsl}");
        assert!(wgsl.contains("result = 1.0;"), "got:\n{wgsl}");
        assert!(wgsl.contains("} else {"), "got:\n{wgsl}");
    }

    #[test]
    fn integration_linear_interpolation() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn lerp(a: f32, b: f32, t: f32) -> f32 {
                a * (1.0 - t) + b * t
            }
        };
        let wgsl = transpile(&func, ["b".to_string(), "t".to_string()]);
        assert!(wgsl.contains("uniforms.b"), "got:\n{wgsl}");
        assert!(wgsl.contains("uniforms.t"), "got:\n{wgsl}");
    }

    #[test]
    fn integration_distance_calculation() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn dist(point1: Vec3, point2: Vec3) -> f32 {
                let diff = point1 - point2;
                diff.length()
            }
        };
        let wgsl = transpile(&func, ["point2".to_string()]);
        assert!(
            wgsl.contains("let diff = point1 - uniforms.point2;"),
            "got:\n{wgsl}"
        );
        assert!(wgsl.contains("return length(diff);"), "got:\n{wgsl}");
    }

    #[test]
    fn integration_complex_lighting() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn lighting(position: Vec3, normal: Vec3, light_pos: Vec3) -> f32 {
                let light_dir = (light_pos - position).normalize();
                let diffuse = normal.dot(light_dir).max(0.0);
                diffuse * 0.8 + 0.2
            }
        };
        let wgsl = transpile(&func, ["normal".to_string(), "light_pos".to_string()]);
        // The parenthesised sub-expression preserves parens
        assert!(
            wgsl.contains("normalize((uniforms.light_pos - position))"),
            "got:\n{wgsl}"
        );
        assert!(
            wgsl.contains("max(dot(uniforms.normal, light_dir), 0.0)"),
            "got:\n{wgsl}"
        );
    }

    #[test]
    fn integration_qualified_path_function() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn safe_sqrt(x: f32) -> f32 {
                let clamped = f32::max(x, 0.0);
                f32::sqrt(clamped)
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("let clamped = max(x, 0.0);"), "got:\n{wgsl}");
        assert!(wgsl.contains("return sqrt(clamped);"), "got:\n{wgsl}");
    }

    #[test]
    fn integration_type_annotated_let() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test(x: f32) -> f32 {
                let y: f32 = x * 2.0;
                return y;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("let y: f32 = x * 2.0;"), "got:\n{wgsl}");
    }

    #[test]
    fn integration_vec_static_constructor() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn origin() -> Vec3 {
                Vec3::new(0.0, 0.0, 0.0)
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(
            wgsl.contains("return vec3<f32>(0.0, 0.0, 0.0);"),
            "got:\n{wgsl}"
        );
    }

    // ===================================================================
    // Error handling tests
    // ===================================================================

    #[test]
    fn error_unsupported_closure() {
        let msg = transpile_expr_err("|x| x + 1");
        assert!(msg.contains("closure"), "Expected closure error: {msg}");
    }

    #[test]
    fn error_unsupported_match() {
        let msg = transpile_expr_err("match x { 0 => 1, _ => 2 }");
        assert!(msg.contains("match"), "Expected match error: {msg}");
    }

    #[test]
    fn error_unsupported_method() {
        let msg = transpile_expr_err("x.to_string()");
        assert!(
            msg.contains("to_string"),
            "Expected method name in error: {msg}"
        );
    }

    #[test]
    fn error_if_without_else() {
        let msg = transpile_expr_err("if flag { x }");
        assert!(
            msg.contains("without else"),
            "Expected 'without else' error: {msg}"
        );
    }
}
