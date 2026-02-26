// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mark derive macro implementation for automatic Mark trait generation.
//!
//! This module generates Mark trait implementations from annotated structs,
//! creating the vertex type, attribute type, and core Mark methods automatically
//! for common mark patterns (quads, triangles).
//!
//! ## Instance Buffer Generation
//!
//! Fields annotated with `#[mark(position)]`, `#[mark(color)]`, `#[mark(size)]`,
//! or any other `#[mark(role)]` attribute will be included in an auto-generated
//! `{Name}Instance` struct with:
//! - `#[repr(C)]` layout for GPU compatibility
//! - `bytemuck::Pod` and `bytemuck::Zeroable` derives
//! - Automatic WGSL-compatible alignment padding
//! - `From<&{Name}>` and `From<{Name}>` conversion implementations

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

    // Generate optional instance struct from field annotations
    let instance_output = generate_instance_struct(name, fields)?;

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

        impl ::gup::mark::Mark for #name {
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
            ) -> ::gup::error::GupResult<&'static str> {
                match attribute_name {
                    #(#attr_type_arms)*
                    // Fall back to common defaults
                    "position" => Ok("vec2<f32>"),
                    "color" => Ok("vec4<f32>"),
                    "size" | "radius" => Ok("f32"),
                    _ => Err(::gup::error::GupError::validation_error(
                        format!("Unknown attribute for {}: {}", stringify!(#name), attribute_name),
                    )),
                }
            }
        }

        #instance_output
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
    let type_name = extract_type_name(field_type).ok_or_else(|| {
        syn::Error::new_spanned(field, "Mark derive requires simple path types for fields.")
    })?;

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
}

/// Extract the simple type name from a `syn::Type`.
fn extract_type_name(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(type_path) = ty {
        type_path.path.segments.last().map(|s| s.ident.to_string())
    } else {
        None
    }
}

// ============================================================
// Instance buffer generation
// ============================================================

/// GPU data type for instance buffer layout calculations.
///
/// Maps Rust types to their GPU-compatible representations with proper
/// WGSL alignment and size information.
#[derive(Debug, Clone, Copy)]
enum GpuType {
    F32,
    I32,
    U32,
    Vec2,
    Vec3,
    Vec4,
}

impl GpuType {
    /// WGSL alignment requirement in bytes.
    fn alignment(self) -> usize {
        match self {
            Self::F32 | Self::I32 | Self::U32 => 4,
            Self::Vec2 => 8,
            Self::Vec3 | Self::Vec4 => 16,
        }
    }

    /// Size in bytes.
    fn size(self) -> usize {
        match self {
            Self::F32 | Self::I32 | Self::U32 => 4,
            Self::Vec2 => 8,
            Self::Vec3 => 12,
            Self::Vec4 => 16,
        }
    }

    /// Rust type token for use in `#[repr(C)]` struct fields.
    fn instance_type(self) -> TokenStream {
        match self {
            Self::F32 => quote! { f32 },
            Self::I32 => quote! { i32 },
            Self::U32 => quote! { u32 },
            Self::Vec2 => quote! { [f32; 2] },
            Self::Vec3 => quote! { [f32; 3] },
            Self::Vec4 => quote! { [f32; 4] },
        }
    }

    /// Expression to convert from the source struct field to the instance field.
    fn conversion_expr(self, field_name: &syn::Ident) -> TokenStream {
        match self {
            Self::F32 | Self::I32 | Self::U32 => quote! { value.#field_name },
            Self::Vec2 => quote! { [value.#field_name.x, value.#field_name.y] },
            Self::Vec3 => {
                quote! { [value.#field_name.x, value.#field_name.y, value.#field_name.z] }
            }
            Self::Vec4 => {
                quote! { [value.#field_name.x, value.#field_name.y, value.#field_name.z, value.#field_name.w] }
            }
        }
    }

    /// Determine GPU type from a Rust type name.
    fn from_type_name(type_name: &str) -> Option<Self> {
        match type_name {
            "f32" => Some(Self::F32),
            "i32" => Some(Self::I32),
            "u32" => Some(Self::U32),
            "Vec2" => Some(Self::Vec2),
            "Vec3" => Some(Self::Vec3),
            "Vec4" => Some(Self::Vec4),
            _ => None,
        }
    }
}

/// Parsed information about a field annotated with `#[mark(role)]`.
struct AnnotatedField {
    name: syn::Ident,
    gpu_type: GpuType,
}

/// Parse a field-level `#[mark(role)]` attribute, returning the role name.
///
/// Recognises simple identifiers like `#[mark(position)]`, `#[mark(color)]`,
/// `#[mark(size)]`, and any other single-identifier role. Key-value pairs
/// (e.g., `primitive = "quad"`) are consumed but ignored since they belong
/// to the container-level attribute.
fn parse_field_mark_role(field: &syn::Field) -> Result<Option<String>> {
    for attr in &field.attrs {
        if !attr.path().is_ident("mark") {
            continue;
        }
        let mut role = None;
        attr.parse_nested_meta(|meta| {
            if meta.input.peek(syn::Token![=]) {
                // Key-value pair (e.g., primitive = "quad") — consume and skip
                let value = meta.value()?;
                let _: Lit = value.parse()?;
            } else if let Some(ident) = meta.path.get_ident() {
                role = Some(ident.to_string());
            }
            Ok(())
        })?;
        if role.is_some() {
            return Ok(role);
        }
    }
    Ok(None)
}

/// Generate `{Name}Instance` struct and `From` impls for annotated fields.
///
/// Only fields with `#[mark(role)]` annotations are included. If no fields
/// are annotated, no instance struct is generated (backward-compatible).
///
/// The generated struct uses `#[repr(C)]` layout with explicit padding fields
/// to satisfy WGSL storage buffer alignment requirements:
/// - `f32`/`i32`/`u32`: 4-byte alignment
/// - `vec2<f32>`: 8-byte alignment
/// - `vec3<f32>`/`vec4<f32>`: 16-byte alignment
/// - Struct size is padded to a multiple of its maximum alignment
fn generate_instance_struct(
    name: &syn::Ident,
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
) -> Result<TokenStream> {
    // Collect fields that have #[mark(role)] annotations
    let mut annotated: Vec<AnnotatedField> = Vec::new();
    for field in fields {
        if parse_field_mark_role(field)?.is_some() {
            let field_name = field.ident.as_ref().unwrap().clone();
            let type_name = extract_type_name(&field.ty).ok_or_else(|| {
                syn::Error::new_spanned(field, "Expected a path type for instance buffer field")
            })?;
            let gpu_type = GpuType::from_type_name(&type_name).ok_or_else(|| {
                syn::Error::new_spanned(
                    field,
                    format!(
                        "Unsupported type '{type_name}' for GPU instance buffer. \
                         Supported: f32, i32, u32, Vec2, Vec3, Vec4"
                    ),
                )
            })?;
            annotated.push(AnnotatedField {
                name: field_name,
                gpu_type,
            });
        }
    }

    if annotated.is_empty() {
        return Ok(TokenStream::new());
    }

    let instance_name = format_ident!("{}Instance", name);

    // Build struct fields and From conversion with WGSL-compatible alignment
    let mut struct_field_tokens = Vec::new();
    let mut from_field_tokens = Vec::new();
    let mut current_offset: usize = 0;
    let mut max_alignment: usize = 4;
    let mut pad_counter: usize = 0;

    for af in &annotated {
        let alignment = af.gpu_type.alignment();
        let size = af.gpu_type.size();
        max_alignment = max_alignment.max(alignment);

        // Insert padding if the current offset is not aligned
        let misalignment = current_offset % alignment;
        if misalignment != 0 {
            let padding_bytes = alignment - misalignment;
            let padding_count = padding_bytes / 4;
            let pad_name = format_ident!("_pad{}", pad_counter);
            pad_counter += 1;

            if padding_count == 1 {
                struct_field_tokens.push(quote! {
                    /// Alignment padding.
                    pub #pad_name: f32,
                });
                from_field_tokens.push(quote! { #pad_name: 0.0, });
            } else {
                struct_field_tokens.push(quote! {
                    /// Alignment padding.
                    pub #pad_name: [f32; #padding_count],
                });
                from_field_tokens.push(quote! { #pad_name: [0.0; #padding_count], });
            }
            current_offset += padding_bytes;
        }

        // Add the data field
        let inst_type = af.gpu_type.instance_type();
        let conv_expr = af.gpu_type.conversion_expr(&af.name);
        let field_name = &af.name;

        struct_field_tokens.push(quote! {
            pub #field_name: #inst_type,
        });
        from_field_tokens.push(quote! { #field_name: #conv_expr, });
        current_offset += size;
    }

    // Struct tail padding — size must be a multiple of max alignment
    let tail_misalignment = current_offset % max_alignment;
    if tail_misalignment != 0 {
        let padding_bytes = max_alignment - tail_misalignment;
        let padding_count = padding_bytes / 4;
        let pad_name = format_ident!("_pad{}", pad_counter);

        if padding_count == 1 {
            struct_field_tokens.push(quote! {
                /// Struct tail padding for WGSL alignment.
                pub #pad_name: f32,
            });
            from_field_tokens.push(quote! { #pad_name: 0.0, });
        } else {
            struct_field_tokens.push(quote! {
                /// Struct tail padding for WGSL alignment.
                pub #pad_name: [f32; #padding_count],
            });
            from_field_tokens.push(quote! { #pad_name: [0.0; #padding_count], });
        }
    }

    Ok(quote! {
        /// Auto-generated GPU-compatible instance data for storage buffer upload.
        ///
        /// This struct has `#[repr(C)]` layout with WGSL-compatible alignment
        /// padding inserted automatically. Fields correspond to
        /// `#[mark(...)]`-annotated fields on [`#name`].
        #[repr(C)]
        #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        pub struct #instance_name {
            #(#struct_field_tokens)*
        }

        impl From<&#name> for #instance_name {
            fn from(value: &#name) -> Self {
                Self {
                    #(#from_field_tokens)*
                }
            }
        }

        impl From<#name> for #instance_name {
            fn from(value: #name) -> Self {
                Self::from(&value)
            }
        }
    })
}
