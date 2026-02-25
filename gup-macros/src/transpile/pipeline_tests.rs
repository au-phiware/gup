// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end pipeline tests for the transpilation prototype.
//!
//! These tests exercise the full pipeline: parse Rust → convert to
//! WGSL AST → generate WGSL text.

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

    #[test]
    fn pipeline_identity() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn identity(value: f32) -> f32 {
                return value;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("fn identity(value: f32) -> f32"));
        assert!(wgsl.contains("return value;"));
    }

    #[test]
    fn pipeline_arithmetic_with_uniforms() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn scale_offset(value: f32, scale: f32, offset: f32) -> f32 {
                return value * scale + offset;
            }
        };
        let wgsl = transpile(&func, ["scale".to_string(), "offset".to_string()]);
        assert!(
            wgsl.contains("uniforms: ScaleOffsetUniforms"),
            "Should have uniforms param, got:\n{wgsl}"
        );
        assert!(wgsl.contains("uniforms.scale"));
        assert!(wgsl.contains("uniforms.offset"));
    }

    #[test]
    fn pipeline_let_bindings() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn normalise(value: f32, lo: f32, hi: f32) -> f32 {
                let range = hi - lo;
                let shifted = value - lo;
                return shifted / range;
            }
        };
        let wgsl = transpile(&func, ["lo".to_string(), "hi".to_string()]);
        assert!(wgsl.contains("let range = uniforms.hi - uniforms.lo;"));
        assert!(wgsl.contains("let shifted = value - uniforms.lo;"));
        assert!(wgsl.contains("return shifted / range;"));
    }

    #[test]
    fn pipeline_function_calls() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn safe_sqrt(value: f32) -> f32 {
                let clamped = clamp(value, 0.0, 100.0);
                return sqrt(clamped);
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("clamp(value, 0.0, 100.0)"));
        assert!(wgsl.contains("sqrt(clamped)"));
    }

    #[test]
    fn pipeline_method_translation() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn process(value: f32) -> f32 {
                return value.abs();
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("abs(value)"));
    }

    #[test]
    fn pipeline_vec_constructor() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn make_position(x: f32) -> Vec2 {
                return Vec2(x, 0.0);
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("-> vec2<f32>"));
        assert!(wgsl.contains("vec2<f32>(x, 0.0)"));
    }

    #[test]
    fn pipeline_type_cast() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn index_to_float(idx: i32) -> f32 {
                return idx as f32;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("f32(idx)"));
    }

    #[test]
    fn pipeline_unary_negation() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn negate(value: f32) -> f32 {
                return -value;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("-value"));
    }

    #[test]
    fn pipeline_complex_expression() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn linear_scale(value: f32, domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> f32 {
                let normalised = (value - domain_min) / (domain_max - domain_min);
                return range_min + normalised * (range_max - range_min);
            }
        };
        let wgsl = transpile(
            &func,
            [
                "domain_min".to_string(),
                "domain_max".to_string(),
                "range_min".to_string(),
                "range_max".to_string(),
            ],
        );
        assert!(wgsl.contains("uniforms.domain_min"));
        assert!(wgsl.contains("uniforms.domain_max"));
        assert!(wgsl.contains("uniforms.range_min"));
        assert!(wgsl.contains("uniforms.range_max"));
        assert!(wgsl.contains("let normalised"));
    }

    #[test]
    fn pipeline_index_access() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn get_element(data: f32) -> f32 {
                return data;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("fn get_element"));
    }

    #[test]
    fn pipeline_multiple_methods() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn chain(a: f32) -> f32 {
                let b = a.abs();
                let c = b.sqrt();
                return clamp(c, 0.0, 1.0);
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("abs(a)"));
        assert!(wgsl.contains("sqrt(b)"));
        assert!(wgsl.contains("clamp(c, 0.0, 1.0)"));
    }

    #[test]
    fn pipeline_preserves_parentheses() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn order_of_ops(a: f32) -> f32 {
                return (a + 1.0) * (a - 1.0);
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("(a + 1.0) * (a - 1.0)"));
    }

    #[test]
    fn pipeline_comparison_operators() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn compare(a: f32) -> bool {
                return a > 0.0;
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("a > 0.0"));
    }

    #[test]
    fn pipeline_error_on_closure() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: syn::Expr = syn::parse_quote!(|x| x + 1);
        assert!(converter.convert_expr(&expr).is_err());
    }

    #[test]
    fn pipeline_error_on_match() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: syn::Expr = syn::parse_quote!(match x {
            0 => 1,
            _ => 2,
        });
        assert!(converter.convert_expr(&expr).is_err());
    }

    #[test]
    fn pipeline_error_on_unsupported_method() {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let expr: syn::Expr = syn::parse_quote!(x.to_string());
        let err = converter.convert_expr(&expr).unwrap_err();
        assert!(err.message.contains("to_string"));
    }
}
