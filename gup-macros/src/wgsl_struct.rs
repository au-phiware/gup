// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Derive macro for `WgslStruct` trait
//!
//! This module implements the `#[derive(WgslStruct)]` macro that automatically
//! generates WGSL struct definitions from Rust structs with proper GPU alignment.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, Result, Type};

/// Implementation of the `#[derive(WgslStruct)]` macro
pub fn derive_wgsl_struct_impl(input: DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let name_str = name.to_string();

    // Verify #[repr(C)] attribute is present
    let has_repr_c = input.attrs.iter().any(|attr| {
        if attr.path().is_ident("repr") {
            // Parse the repr attribute to check for C
            if let Ok(list) = attr.parse_args::<syn::Ident>() {
                return list == "C";
            }
        }
        false
    });

    if !has_repr_c {
        return Err(Error::new_spanned(
            &input,
            "WgslStruct requires #[repr(C)] for proper GPU memory layout. Add #[repr(C)] above your struct.",
        ));
    }

    // Only support structs with named fields
    let fields = match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => &fields_named.named,
            _ => {
                return Err(Error::new_spanned(
                    &input,
                    "WgslStruct can only be derived for structs with named fields. Tuple structs are not supported.",
                ));
            }
        },
        _ => {
            return Err(Error::new_spanned(
                &input,
                "WgslStruct can only be derived for structs. Enums and unions are not supported.",
            ));
        }
    };

    if fields.is_empty() {
        return Err(Error::new_spanned(
            &input,
            "WgslStruct cannot be derived for empty structs. Add at least one field.",
        ));
    }

    // Generate WGSL struct definition
    let mut wgsl_fields = Vec::new();
    let mut size_calculation = quote! { 0 };
    let mut alignment_calculation = quote! { 1 };

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        // Skip padding fields (conventionally named with underscore prefix or "padding")
        let field_name_str = field_name.to_string();
        if field_name_str.starts_with('_') || field_name_str.contains("padding") {
            continue;
        }

        // Map Rust type to WGSL type
        let wgsl_type = rust_type_to_wgsl(&field_type)?;

        wgsl_fields.push(format!("    {}: {},", field_name_str, wgsl_type));

        // Add to size and alignment calculations
        size_calculation = quote! {
            #size_calculation + std::mem::size_of::<#field_type>()
        };
        alignment_calculation = quote! {
            std::cmp::max(#alignment_calculation, std::mem::align_of::<#field_type>())
        };
    }

    // Build complete WGSL struct definition
    let wgsl_definition = format!("struct {} {{\n{}\n}}", name_str, wgsl_fields.join("\n"));

    // Generate the trait implementation
    let generated = quote! {
        impl gup::shader_function::WgslStructType for #name {
            fn wgsl_struct_definition() -> &'static str {
                #wgsl_definition
            }

            fn struct_name() -> &'static str {
                #name_str
            }
        }

        // Also implement ShaderType for completeness
        impl gup::shader_function::ShaderType for #name {
            fn wgsl_type_name() -> &'static str {
                #name_str
            }

            fn wgsl_type_definition() -> Option<&'static str> {
                Some(#wgsl_definition)
            }

            fn size_bytes() -> usize {
                #size_calculation
            }

            fn alignment() -> usize {
                #alignment_calculation
            }
        }
    };

    Ok(generated)
}

/// Convert Rust type to WGSL type string
fn rust_type_to_wgsl(ty: &Type) -> Result<String> {
    match ty {
        Type::Path(type_path) => {
            let path = &type_path.path;
            if path.segments.len() == 1 {
                let segment = &path.segments[0];
                let type_name = segment.ident.to_string();

                match type_name.as_str() {
                    // Scalar types
                    "f32" => Ok("f32".to_string()),
                    "i32" => Ok("i32".to_string()),
                    "u32" => Ok("u32".to_string()),
                    "bool" => Ok("bool".to_string()),

                    // Vector types
                    "Vec2" => Ok("vec2<f32>".to_string()),
                    "Vec3" => Ok("vec3<f32>".to_string()),
                    "Vec4" => Ok("vec4<f32>".to_string()),

                    // Matrix types - square
                    "Mat2" => Ok("mat2x2<f32>".to_string()),
                    "Mat3" => Ok("mat3x3<f32>".to_string()),
                    "Mat4" => Ok("mat4x4<f32>".to_string()),

                    // Matrix types - non-square
                    "Mat2x3" => Ok("mat2x3<f32>".to_string()),
                    "Mat2x4" => Ok("mat2x4<f32>".to_string()),
                    "Mat3x2" => Ok("mat3x2<f32>".to_string()),
                    "Mat3x4" => Ok("mat3x4<f32>".to_string()),
                    "Mat4x2" => Ok("mat4x2<f32>".to_string()),
                    "Mat4x3" => Ok("mat4x3<f32>".to_string()),

                    // Custom types - assume they implement WgslStructType
                    other => Ok(other.to_string()),
                }
            } else {
                Err(Error::new_spanned(
                    ty,
                    "Complex type paths are not supported in WgslStruct. Use simple type names.",
                ))
            }
        }
        Type::Array(type_array) => {
            // Handle array types like [f32; 4]
            let elem_type = rust_type_to_wgsl(&type_array.elem)?;
            match &type_array.len {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(lit_int),
                    ..
                }) => {
                    let len = lit_int.base10_parse::<usize>().map_err(|_| {
                        Error::new_spanned(
                            lit_int,
                            "Invalid array length. Must be a positive integer.",
                        )
                    })?;
                    Ok(format!("array<{}, {}>", elem_type, len))
                }
                _ => Err(Error::new_spanned(
                    &type_array.len,
                    "Only literal array lengths are supported in WgslStruct. Use a constant like [f32; 4].",
                )),
            }
        }
        _ => Err(Error::new_spanned(
            ty,
            "This type is not supported in WgslStruct. Supported types: f32, i32, u32, bool, Vec2, Vec3, Vec4, Mat2/3/4, arrays, and custom WgslStruct types.",
        )),
    }
}
