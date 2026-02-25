// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mark derive macro implementation for automatic Mark trait generation.
//!
//! This module generates Mark trait implementations from annotated structs,
//! creating the vertex type, attribute type, and core Mark methods automatically
//! for common mark patterns (quads, triangles).

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Fields, Lit, Result};

/// Supported primitive types for mark geometry.
#[derive(Debug, Clone, Copy)]
enum MarkPrimitive {
    /// A quad (4 vertices, 6 indices)
    Quad,
    /// A single triangle (3 vertices, no indices needed)
    Triangle,
}

impl MarkPrimitive {
    fn vertex_count(&self) -> usize {
        match self {
            Self::Quad => 4,
            Self::Triangle => 3,
        }
    }

    fn index_count(&self) -> Option<usize> {
        match self {
            Self::Quad => Some(6),
            Self::Triangle => None,
        }
    }
}

/// Generate the Mark trait implementation for a derive-annotated struct.
pub fn derive_mark_impl(input: DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;

    // Validate we have a struct with named fields
    let fields = match &input.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => &fields_named.named,
            Fields::Unit => {
                return Err(syn::Error::new_spanned(
                    &input,
                    "Mark derive requires a struct with named fields for attribute definition. \
                     Use #[derive(Mark)] on a struct with position, color, and size fields.",
                ));
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    &input,
                    "Mark derive requires a struct with named fields, not a tuple struct.",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &input,
                "Mark derive can only be used on structs.",
            ));
        }
    };

    // Parse the #[mark(...)] attribute
    let primitive = parse_mark_attrs(&input)?;
    let vertex_count = primitive.vertex_count();
    let index_count_expr = match primitive.index_count() {
        Some(n) => quote! { Some(#n) },
        None => quote! { None },
    };

    // Generate vertex type name
    let vertex_name = format_ident!("{}Vertex", name);

    // Generate vertices and indices based on primitive type
    let (generate_vertices, generate_indices) = match primitive {
        MarkPrimitive::Quad => (
            quote! {
                vec![
                    #vertex_name { position: [-1.0, -1.0] },
                    #vertex_name { position: [ 1.0, -1.0] },
                    #vertex_name { position: [ 1.0,  1.0] },
                    #vertex_name { position: [-1.0,  1.0] },
                ]
            },
            quote! {
                Some(vec![0, 1, 2, 0, 2, 3])
            },
        ),
        MarkPrimitive::Triangle => (
            quote! {
                vec![
                    #vertex_name { position: [ 0.0,  1.0] },
                    #vertex_name { position: [-1.0, -1.0] },
                    #vertex_name { position: [ 1.0, -1.0] },
                ]
            },
            quote! {
                None
            },
        ),
    };

    // Collect attribute types for get_attribute_type
    let mut attr_type_arms = Vec::new();
    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let field_type = &field.ty;

        // Determine WGSL type from field attributes or type
        let wgsl_type = get_wgsl_type_for_field(field_type, field)?;

        attr_type_arms.push(quote! {
            #field_name_str => Ok(#wgsl_type),
        });
    }

    let expanded = quote! {
        /// Auto-generated GPU vertex type for mark rendering.
        ///
        /// Each vertex represents a corner of the base geometry.
        /// Instance-specific data is handled via storage buffers.
        #[repr(C)]
        #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        pub struct #vertex_name {
            /// Local position within the base geometry
            pub position: [f32; 2],
        }

        impl crate::mark::Mark for #name {
            type Vertex = #vertex_name;
            type AttributeValue = #name;

            fn vertex_count() -> usize {
                #vertex_count
            }

            fn index_count() -> Option<usize> {
                #index_count_expr
            }

            fn generate_vertices() -> Vec<Self::Vertex> {
                #generate_vertices
            }

            fn generate_indices() -> Option<Vec<u32>> {
                #generate_indices
            }

            fn get_attribute_type(
                attribute_name: &str,
            ) -> crate::error::GupResult<&'static str> {
                match attribute_name {
                    #(#attr_type_arms)*
                    // Fall back to common defaults
                    "position" => Ok("vec2<f32>"),
                    "color" => Ok("vec4<f32>"),
                    "size" | "radius" => Ok("f32"),
                    _ => Err(crate::error::GupError::validation_error(
                        format!("Unknown attribute for {}: {}", stringify!(#name), attribute_name),
                    )),
                }
            }
        }
    };

    Ok(expanded)
}

/// Parse the `#[mark(...)]` container attribute.
fn parse_mark_attrs(input: &DeriveInput) -> Result<MarkPrimitive> {
    let mut primitive = MarkPrimitive::Quad; // default

    for attr in &input.attrs {
        if !attr.path().is_ident("mark") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("primitive") {
                let value = meta.value()?;
                let lit: Lit = value.parse()?;
                if let Lit::Str(lit_str) = lit {
                    primitive = match lit_str.value().as_str() {
                        "quad" => MarkPrimitive::Quad,
                        "triangle" => MarkPrimitive::Triangle,
                        other => {
                            return Err(syn::Error::new_spanned(
                                &lit_str,
                                format!(
                                    "Unknown primitive type '{}'. \
                                     Supported: \"quad\", \"triangle\"",
                                    other
                                ),
                            ));
                        }
                    };
                } else {
                    return Err(syn::Error::new_spanned(
                        &lit,
                        "primitive must be a string literal",
                    ));
                }
            }
            Ok(())
        })?;
    }

    Ok(primitive)
}

/// Determine the WGSL type from a Rust field type.
fn get_wgsl_type_for_field(field_type: &syn::Type, field: &syn::Field) -> Result<&'static str> {
    if let syn::Type::Path(type_path) = field_type {
        let type_name = type_path
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();

        match type_name.as_str() {
            "f32" => Ok("f32"),
            "i32" => Ok("i32"),
            "u32" => Ok("u32"),
            "Vec2" => Ok("vec2<f32>"),
            "Vec3" => Ok("vec3<f32>"),
            "Vec4" => Ok("vec4<f32>"),
            "Mat2" => Ok("mat2x2<f32>"),
            "Mat3" => Ok("mat3x3<f32>"),
            "Mat4" => Ok("mat4x4<f32>"),
            _ => Err(syn::Error::new_spanned(
                field,
                format!(
                    "Unsupported field type '{}' for Mark derive. \
                     Supported types: f32, i32, u32, Vec2, Vec3, Vec4, Mat2, Mat3, Mat4",
                    type_name
                ),
            )),
        }
    } else {
        Err(syn::Error::new_spanned(
            field,
            "Mark derive requires simple path types for fields.",
        ))
    }
}
