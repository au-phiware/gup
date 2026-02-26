// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Procedural macro for generating shader functions via Rust-to-WGSL transpilation.
//!
//! The `#[shader_fn]` attribute transpiles a Rust function into WGSL using the
//! transpilation pipeline (GUP-055 through GUP-059) and generates a struct that
//! implements `ComposableShaderFunction`. This allows shader logic to be written
//! in idiomatic Rust rather than embedded WGSL strings.
//!
//! The generated output is fully compatible with `#[wgsl_function]` — both
//! approaches produce types implementing the same trait, so they can be mixed
//! freely in a `ShaderPipeline`.

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Error, FnArg, ItemFn, Pat, PatType, Result, Type};

use crate::transpile::{RustToWgsl, WgslCodeGen};
use crate::wgsl_function::{UniformParam, WgslFunctionInfo};
use crate::wgsl_keywords::{validate_function_name, validate_param_name};

/// Parse a `syn::ItemFn` and transpile its body to WGSL, producing a
/// [`WgslFunctionInfo`] that can be rendered to tokens identically to
/// `#[wgsl_function]`.
pub fn expand_shader_fn(function: ItemFn) -> Result<TokenStream> {
    // --- Validate function signature (same constraints as #[wgsl_function]) ---
    let function_name = function.sig.ident.to_string();

    if function_name.is_empty() {
        return Err(Error::new_spanned(
            &function.sig.ident,
            "Function name cannot be empty",
        ));
    }

    if function_name.contains("__") {
        return Err(Error::new_spanned(
            &function.sig.ident,
            "Function names with double underscores are reserved. Use single underscores instead.",
        ));
    }

    // Validate function name is not a WGSL reserved keyword
    validate_function_name(&function.sig.ident)?;

    if !function.sig.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &function.sig.generics,
            "Generic functions are not supported in #[shader_fn].",
        ));
    }

    if function.sig.asyncness.is_some() {
        return Err(Error::new_spanned(
            function.sig.asyncness,
            "Async functions are not supported in #[shader_fn].",
        ));
    }

    if function.sig.unsafety.is_some() {
        return Err(Error::new_spanned(
            function.sig.unsafety,
            "Unsafe functions are not supported in #[shader_fn].",
        ));
    }

    let inputs = &function.sig.inputs;

    if inputs.is_empty() {
        return Err(Error::new_spanned(
            &function.sig,
            "Shader function must have at least one input parameter.",
        ));
    }

    // --- Extract input type, output type, and uniform parameters ---
    let input_type = match inputs.first().unwrap() {
        FnArg::Typed(PatType { pat, ty, .. }) => {
            // Validate the first parameter name is not a WGSL reserved keyword
            if let Pat::Ident(pat_ident) = &**pat {
                validate_param_name(&pat_ident.ident)?;
            }
            (**ty).clone()
        }
        FnArg::Receiver(_) => {
            return Err(Error::new_spanned(
                inputs.first().unwrap(),
                "Shader functions cannot have 'self' parameters.",
            ));
        }
    };

    let output_type = match &function.sig.output {
        syn::ReturnType::Type(_, ty) => (**ty).clone(),
        syn::ReturnType::Default => {
            return Err(Error::new_spanned(
                &function.sig,
                "Shader functions must have an explicit return type.",
            ));
        }
    };

    let mut uniform_param_names = Vec::new();
    let mut uniform_params = Vec::new();

    for input in inputs.iter().skip(1) {
        match input {
            FnArg::Typed(PatType { pat, ty, .. }) => {
                if let Pat::Ident(pat_ident) = &**pat {
                    let name = pat_ident.ident.clone();

                    // Validate parameter name is not a WGSL reserved keyword
                    validate_param_name(&name)?;

                    let wgsl_type =
                        rust_type_to_wgsl_string(ty).map_err(|msg| Error::new_spanned(ty, msg))?;

                    uniform_param_names.push(name.to_string());
                    uniform_params.push(UniformParam {
                        name,
                        rust_type: (**ty).clone(),
                        wgsl_type,
                    });
                } else {
                    return Err(Error::new_spanned(
                        pat,
                        "Only simple identifiers are allowed as parameter names.",
                    ));
                }
            }
            FnArg::Receiver(_) => {
                return Err(Error::new_spanned(input, "Unexpected 'self' parameter."));
            }
        }
    }

    // --- Transpile the Rust function body to WGSL ---
    let mut converter = RustToWgsl::new(uniform_param_names);
    let wgsl_function_ast = converter
        .convert_function(&function)
        .map_err(|e| e.into_syn_error())?;

    let mut codegen = WgslCodeGen::new();
    let wgsl_fn_code = codegen.generate_function(&wgsl_function_ast);

    // Build the complete WGSL body including the uniform struct definition
    let wgsl_body = if !uniform_params.is_empty() {
        let uniforms_struct_name = format!("{}Uniforms", pascal_case(&function_name));
        let struct_fields: Vec<String> = uniform_params
            .iter()
            .map(|p| format!("    {}: {}", p.name, p.wgsl_type))
            .collect();

        format!(
            "struct {} {{\n{},\n}}\n\n{}",
            uniforms_struct_name,
            struct_fields.join(",\n"),
            wgsl_fn_code
        )
    } else {
        wgsl_fn_code
    };

    // --- Build the WgslFunctionInfo and render to tokens ---
    let struct_name_str = pascal_case(&function_name);
    let struct_name = format_ident!("{}", struct_name_str);
    let uniforms_name = format_ident!("{}Uniforms", struct_name_str);

    // Collect custom types
    let mut custom_types = Vec::new();
    if is_custom_type(&input_type) {
        custom_types.push(input_type.clone());
    }
    if is_custom_type(&output_type) {
        custom_types.push(output_type.clone());
    }
    for param in &uniform_params {
        if is_custom_type(&param.rust_type) {
            custom_types.push(param.rust_type.clone());
        }
    }

    let info = WgslFunctionInfo {
        function_name,
        struct_name,
        uniforms_name,
        input_type,
        output_type,
        uniform_params,
        wgsl_body,
        custom_types,
    };

    let mut tokens = TokenStream::new();
    info.to_tokens(&mut tokens);
    Ok(tokens)
}

// ---- Helpers (mirrors wgsl_function.rs helpers) ----

/// Convert a Rust type to a WGSL type string.
fn rust_type_to_wgsl_string(ty: &Type) -> std::result::Result<String, String> {
    match ty {
        Type::Path(type_path) => {
            let path = &type_path.path;
            if path.segments.len() == 1 {
                let segment = &path.segments[0];
                match segment.ident.to_string().as_str() {
                    "f32" => Ok("f32".into()),
                    "i32" => Ok("i32".into()),
                    "u32" => Ok("u32".into()),
                    "bool" => Ok("bool".into()),
                    "Vec2" => Ok("vec2<f32>".into()),
                    "Vec3" => Ok("vec3<f32>".into()),
                    "Vec4" => Ok("vec4<f32>".into()),
                    "IVec2" => Ok("vec2<i32>".into()),
                    "IVec3" => Ok("vec3<i32>".into()),
                    "IVec4" => Ok("vec4<i32>".into()),
                    "UVec2" => Ok("vec2<u32>".into()),
                    "UVec3" => Ok("vec3<u32>".into()),
                    "UVec4" => Ok("vec4<u32>".into()),
                    "Mat2" => Ok("mat2x2<f32>".into()),
                    "Mat3" => Ok("mat3x3<f32>".into()),
                    "Mat4" => Ok("mat4x4<f32>".into()),
                    other => Ok(other.into()),
                }
            } else {
                Err(format!("Unsupported type: {}", quote!(#ty)))
            }
        }
        Type::Array(type_array) => {
            let elem = rust_type_to_wgsl_string(&type_array.elem)?;
            let len = &type_array.len;
            Ok(format!("array<{}, {}>", elem, quote!(#len)))
        }
        _ => Err(format!("Unsupported type: {}", quote!(#ty))),
    }
}

/// Convert snake_case to PascalCase.
fn pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect()
}

/// Check if a type is a custom (non-primitive) type that might have a WgslStruct definition.
fn is_custom_type(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => {
            let path = &type_path.path;
            if path.segments.len() == 1 {
                let type_name = path.segments[0].ident.to_string();
                !matches!(
                    type_name.as_str(),
                    "f32"
                        | "i32"
                        | "u32"
                        | "bool"
                        | "Vec2"
                        | "Vec3"
                        | "Vec4"
                        | "IVec2"
                        | "IVec3"
                        | "IVec4"
                        | "UVec2"
                        | "UVec3"
                        | "UVec4"
                        | "Mat2"
                        | "Mat3"
                        | "Mat4"
                        | "Mat2x3"
                        | "Mat2x4"
                        | "Mat3x2"
                        | "Mat3x4"
                        | "Mat4x2"
                        | "Mat4x3"
                )
            } else {
                false
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: parse a function and expand it, returning the token stream string.
    fn expand(input: proc_macro2::TokenStream) -> std::result::Result<String, String> {
        let func: ItemFn = syn::parse2(input).map_err(|e| e.to_string())?;
        let tokens = expand_shader_fn(func).map_err(|e| e.to_string())?;
        Ok(tokens.to_string())
    }

    #[test]
    fn simple_function_expands() {
        let tokens = expand(quote! {
            fn my_func(value: f32) -> f32 {
                return value * 2.0;
            }
        })
        .unwrap();

        assert!(tokens.contains("MyFunc"), "Should generate MyFunc struct");
        assert!(
            tokens.contains("MyFuncUniforms"),
            "Should generate uniforms struct"
        );
        assert!(
            tokens.contains("ComposableShaderFunction"),
            "Should implement trait"
        );
        assert!(tokens.contains("my_func"), "Should contain function name");
    }

    #[test]
    fn function_with_uniforms_expands() {
        let tokens = expand(quote! {
            fn scale_offset(value: f32, scale: f32, offset: f32) -> f32 {
                return value * scale + offset;
            }
        })
        .unwrap();

        assert!(
            tokens.contains("ScaleOffset"),
            "Should generate ScaleOffset struct, got: {tokens}"
        );
        assert!(
            tokens.contains("ScaleOffsetUniforms"),
            "Should generate uniforms struct"
        );
    }

    #[test]
    fn error_on_no_params() {
        let result = expand(quote! {
            fn bad() -> f32 {
                return 1.0;
            }
        });
        assert!(result.is_err(), "Should fail for function with no params");
    }

    #[test]
    fn error_on_no_return_type() {
        let result = expand(quote! {
            fn bad(value: f32) {
                return;
            }
        });
        assert!(
            result.is_err(),
            "Should fail for function with no return type"
        );
    }

    #[test]
    fn error_on_generic_function() {
        let result = expand(quote! {
            fn bad<T>(value: T) -> T {
                return value;
            }
        });
        assert!(result.is_err(), "Should fail for generic functions");
    }

    #[test]
    fn error_on_async_function() {
        let result = expand(quote! {
            async fn bad(value: f32) -> f32 {
                return value;
            }
        });
        assert!(result.is_err(), "Should fail for async functions");
    }

    #[test]
    fn pascal_case_conversion() {
        assert_eq!(pascal_case("linear_scale"), "LinearScale");
        assert_eq!(pascal_case("my_func"), "MyFunc");
        assert_eq!(pascal_case("simple"), "Simple");
    }
}
