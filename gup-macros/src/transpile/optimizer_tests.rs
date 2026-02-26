// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the transpile optimizer.

#[cfg(test)]
mod tests {
    use crate::transpile::optimizer::*;
    use crate::transpile::{RustToWgsl, WgslCodeGen};

    /// Helper: transpile a Rust function and optimise it.
    fn transpile_and_optimise(func: &syn::ItemFn, config: &OptimizationConfig) -> String {
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let wgsl_func = converter.convert_function(func).unwrap();
        let mut module = crate::transpile::WgslModule {
            structs: vec![],
            functions: vec![wgsl_func],
        };
        let _results = optimize_module(&mut module, config);
        let mut codegen = WgslCodeGen::new();
        codegen.generate_module(&module)
    }

    #[test]
    fn dead_variable_removed_end_to_end() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test_fn(x: f32) -> f32 {
                let unused = 42.0;
                return x;
            }
        };
        let wgsl = transpile_and_optimise(&func, &OptimizationConfig::default());
        assert!(
            !wgsl.contains("unused"),
            "unused variable should be removed, got:\n{wgsl}"
        );
        assert!(wgsl.contains("return x;"));
    }

    #[test]
    fn constant_expression_folded_end_to_end() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test_fn(x: f32) -> f32 {
                let a = 2.0 + 3.0;
                return x + a;
            }
        };
        let wgsl = transpile_and_optimise(&func, &OptimizationConfig::default());
        assert!(
            wgsl.contains("5.0"),
            "2.0 + 3.0 should be folded to 5.0, got:\n{wgsl}"
        );
    }

    #[test]
    fn identity_multiplication_removed_end_to_end() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test_fn(x: f32) -> f32 {
                return x * 1.0;
            }
        };
        let wgsl = transpile_and_optimise(&func, &OptimizationConfig::default());
        assert!(
            wgsl.contains("return x;"),
            "x * 1.0 should simplify to x, got:\n{wgsl}"
        );
    }

    #[test]
    fn no_optimisation_preserves_original() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test_fn(x: f32) -> f32 {
                let unused = 42.0;
                return x * 1.0;
            }
        };
        let wgsl = transpile_and_optimise(&func, &OptimizationConfig::none());
        assert!(
            wgsl.contains("unused"),
            "no-opt should preserve unused vars, got:\n{wgsl}"
        );
        assert!(
            wgsl.contains("x * 1.0"),
            "no-opt should preserve identity mul, got:\n{wgsl}"
        );
    }

    #[test]
    fn combined_optimisations() {
        let func: syn::ItemFn = syn::parse_quote! {
            fn test_fn(x: f32) -> f32 {
                let unused = 99.0;
                let a = 2.0 + 3.0;
                let b = x * 1.0;
                return b + a;
            }
        };
        let wgsl = transpile_and_optimise(&func, &OptimizationConfig::aggressive());
        // unused should be removed
        assert!(
            !wgsl.contains("unused"),
            "unused should be removed, got:\n{wgsl}"
        );
        // 2.0 + 3.0 should be folded
        assert!(wgsl.contains("5.0"), "should fold 2+3, got:\n{wgsl}");
        // x * 1.0 should simplify
        // (b is just x after folding, so return should be x + a or b + a)
    }
}
