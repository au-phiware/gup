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

//! Procedural macros for the Gup GPU visualization library

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{Data, DeriveInput, Fields, parse_macro_input};

mod mark_derive;
mod mark_type_id;
mod mixable_derive;
mod wgsl_function;
mod wgsl_struct;

use wgsl_function::WgslFunctionInfo;

/// Procedural macro for generating shader functions from WGSL syntax.
///
/// This macro allows you to write WGSL functions directly in Rust and automatically
/// generates the corresponding `ComposableShaderFunction` trait implementation.
///
/// # Example
///
/// ```rust,ignore
/// use gup::*;
///
/// #[wgsl_function]
/// fn linear_scale(value: f32, scale: f32) -> f32 {
///     return value * scale;
/// }
/// ```
///
/// This generates:
/// - A `LinearScale` struct with configuration fields
/// - A `LinearScaleUniforms` struct for GPU uniforms
/// - An implementation of `ComposableShaderFunction` for `LinearScale`
/// - WGSL code that can be compiled and run on the GPU
#[proc_macro_attribute]
pub fn wgsl_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as WgslFunctionInfo);
    let mut tokens = proc_macro2::TokenStream::new();
    input.to_tokens(&mut tokens);
    TokenStream::from(tokens)
}

/// Derive macro for automatically implementing the `ShaderType` trait.
///
/// This macro generates a `ShaderType` implementation for custom structs,
/// including WGSL type definitions and proper memory layout calculations.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(ShaderType)]
/// struct WeatherData {
///     longitude: f32,
///     latitude: f32,
///     temperature: f32,
///     wind_vector: Vec3,
/// }
/// ```
///
/// This generates:
/// - `ShaderType` trait implementation with correct WGSL type name
/// - WGSL struct definition with proper field types
/// - Memory layout calculations for GPU compatibility
#[proc_macro_derive(ShaderType)]
pub fn derive_shader_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let name_str = name.to_string();

    // For now, only support structs with named fields
    let fields = match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => &fields_named.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "ShaderType can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "ShaderType can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    // Generate WGSL struct definition
    let mut wgsl_definition = format!("struct {name_str} {{\n");
    let mut size_calculation = quote! { 0 };
    let mut max_alignment = quote! { 1 };

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        // Basic type mapping - this could be extended for more complex types
        let wgsl_type = match field_type {
            syn::Type::Path(type_path) if type_path.path.is_ident("f32") => "f32",
            syn::Type::Path(type_path) if type_path.path.is_ident("i32") => "i32",
            syn::Type::Path(type_path) if type_path.path.is_ident("u32") => "u32",
            syn::Type::Path(type_path) if type_path.path.is_ident("bool") => "bool",
            syn::Type::Path(type_path) if type_path.path.is_ident("Vec2") => "vec2<f32>",
            syn::Type::Path(type_path) if type_path.path.is_ident("Vec3") => "vec3<f32>",
            syn::Type::Path(type_path) if type_path.path.is_ident("Vec4") => "vec4<f32>",
            syn::Type::Path(type_path) if type_path.path.is_ident("Mat2") => "mat2x2<f32>",
            syn::Type::Path(type_path) if type_path.path.is_ident("Mat3") => "mat3x3<f32>",
            syn::Type::Path(type_path) if type_path.path.is_ident("Mat4") => "mat4x4<f32>",
            _ => {
                return syn::Error::new_spanned(
                    field_type,
                    "Unsupported field type for ShaderType derivation. Supported types: f32, i32, u32, bool, Vec2, Vec3, Vec4, Mat2, Mat3, Mat4"
                ).to_compile_error().into();
            }
        };

        wgsl_definition.push_str(&format!("    {field_name}: {wgsl_type},\n"));

        // Add to size and alignment calculations
        size_calculation = quote! {
            #size_calculation + <#field_type as ShaderType>::size_bytes()
        };
        max_alignment = quote! {
            std::cmp::max(#max_alignment, <#field_type as ShaderType>::alignment())
        };
    }

    wgsl_definition.push('}');

    let generated = quote! {
        impl ShaderType for #name {
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
                #max_alignment
            }
        }
    };

    generated.into()
}

/// Derive macro for automatically implementing the `Mixable` trait.
///
/// This macro generates a `Mixable` implementation for custom structs,
/// enabling them to be used in Gup's composition system with minimal boilerplate.
///
/// # Attributes
///
/// - `#[mixable(render_type = "points")]` - Specify the rendering type (points, lines, triangles)
/// - `#[mixable(vertex_data)]` - Mark a field as containing vertex data for GPU rendering
/// - `#[mixable(uniform_data)]` - Mark a field as containing uniform data
/// - `#[mixable(texture_data)]` - Mark a field as containing texture data
/// - `#[mixable(binding = N)]` - Specify the binding index for uniform/texture fields
///
/// # Examples
///
/// ```rust,ignore
/// use gup_macros::Mixable;
///
/// #[derive(Mixable)]
/// #[mixable(render_type = "points")]
/// struct ScatterPlot {
///     #[mixable(vertex_data)]
///     points: Vec<[f32; 2]>,
///     
///     #[mixable(uniform_data, binding = 0)]
///     color: [f32; 4],
/// }
/// ```
///
/// This generates:
/// - A `Mixable` trait implementation with point-based rendering
/// - Proper vertex data extraction from the `points` field
/// - Uniform binding setup for the `color` field
/// - Validation methods to ensure data integrity
#[proc_macro_derive(Mixable, attributes(mixable))]
pub fn derive_mixable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match mixable_derive::generate_mixable_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Derive macro for automatically implementing the `WgslStructType` trait.
///
/// This macro generates WGSL struct definitions from Rust structs with proper GPU
/// alignment validation. It automatically maps Rust types to WGSL types and generates
/// the complete struct definition string.
///
/// # Requirements
///
/// - Struct must have `#[repr(C)]` for GPU memory layout compatibility
/// - Struct must have named fields (no tuple structs)
/// - All field types must be WGSL-compatible or implement `WgslStructType`
/// - Padding fields (starting with `_` or containing "padding") are automatically skipped
///
/// # Supported Types
///
/// - Scalar types: `f32`, `i32`, `u32`, `bool`
/// - Vector types: `Vec2`, `Vec3`, `Vec4`
/// - Matrix types: `Mat2`, `Mat3`, `Mat4`, `Mat2x3`, `Mat2x4`, `Mat3x2`, `Mat3x4`, `Mat4x2`, `Mat4x3`
/// - Array types: `[T; N]` where T is a supported type
/// - Custom types that implement `WgslStructType`
///
/// # Example
///
/// ```rust,ignore
/// use gup_macros::WgslStruct;
///
/// #[derive(WgslStruct, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// #[repr(C)]
/// struct Material {
///     albedo: Vec3,
///     metallic: f32,
///     roughness: f32,
///     _padding: [f32; 3],  // Automatically skipped in WGSL
/// }
/// ```
///
/// This generates:
/// - `WgslStructType` trait implementation with WGSL struct definition
/// - `ShaderType` trait implementation for full integration
/// - Proper size and alignment calculations
#[proc_macro_derive(WgslStruct)]
pub fn derive_wgsl_struct(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match wgsl_struct::derive_wgsl_struct_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Derive macro for automatically generating stable mark type IDs.
///
/// This macro generates a compile-time constant `MARK_TYPE_ID` and implements
/// the `MarkTypeIdProvider` trait for mark types. This provides stable, GPU-compatible
/// IDs that are validated at compile time.
///
/// # Requirements
///
/// - Must be used with the `#[mark_type_id = N]` attribute
/// - ID must be in the range 0-255 (u8 range for GPU compatibility)
/// - IDs must be unique across all mark types (validated at runtime in tests)
///
/// # Example
///
/// ```rust,ignore
/// use gup_macros::MarkTypeId;
/// use gup::mark::Mark;
///
/// #[derive(Clone, MarkTypeId)]
/// #[mark_type_id = 0]
/// pub struct Circle;
///
/// #[derive(Clone, MarkTypeId)]
/// #[mark_type_id = 1]
/// pub struct Rectangle;
///
/// // Access the compile-time constant
/// assert_eq!(Circle::MARK_TYPE_ID, 0);
/// assert_eq!(Rectangle::MARK_TYPE_ID, 1);
/// ```
///
/// This generates:
/// - `pub const MARK_TYPE_ID: u32` - The stable type ID
/// - `MarkTypeIdProvider` trait implementation
/// - Documentation explaining the ID value
///
/// # GPU Shader Integration
///
/// The generated IDs must match the enum values in GPU shaders. Document
/// the mapping in your shader code:
///
/// ```wgsl
/// // Mark type IDs must match Rust MarkTypeId assignments:
/// // 0 = Circle
/// // 1 = Rectangle
/// // 2 = Line
/// ```
#[proc_macro_derive(MarkTypeId, attributes(mark_type_id))]
pub fn derive_mark_type_id(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match mark_type_id::derive_mark_type_id_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Derive macro for automatically implementing the `Mark` trait.
///
/// This macro generates a complete `Mark` trait implementation from an annotated
/// struct, including a GPU-compatible vertex type, base geometry generation, and
/// attribute type validation. It supports common mark patterns with minimal
/// boilerplate.
///
/// # Container Attributes
///
/// - `#[mark(primitive = "quad")]` - Generate quad geometry (4 vertices, 6 indices).
///   This is the default.
/// - `#[mark(primitive = "triangle")]` - Generate triangle geometry (3 vertices).
///
/// # Field Types
///
/// Fields are mapped to WGSL types for attribute validation:
/// - `f32` → `f32`
/// - `Vec2` → `vec2<f32>`
/// - `Vec3` → `vec3<f32>`
/// - `Vec4` → `vec4<f32>`
///
/// # Example
///
/// ```rust,ignore
/// use gup_macros::Mark;
/// use gup::shader_function::{Vec2, Vec4};
///
/// #[derive(Debug, Clone, Mark)]
/// #[mark(primitive = "quad")]
/// pub struct Diamond {
///     pub center: Vec2,
///     pub size: f32,
///     pub color: Vec4,
///     pub angle: f32,
/// }
///
/// // This generates:
/// // - `DiamondVertex` struct with `position: [f32; 2]`
/// // - `impl Mark for Diamond` with quad geometry
/// // - Attribute type validation for all fields
/// ```
///
/// # Generated Items
///
/// For a struct named `Foo`, the macro generates:
/// - `FooVertex` - A `#[repr(C)]` GPU vertex type with a `position: [f32; 2]` field
/// - `impl Mark for Foo` with:
///   - `type Vertex = FooVertex`
///   - `type AttributeValue = Foo`
///   - Geometry generation matching the selected primitive
///   - `get_attribute_type()` returning correct WGSL types for all fields
#[proc_macro_derive(Mark, attributes(mark))]
pub fn derive_mark(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match mark_derive::derive_mark_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}
