// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Derive macro implementation for automatic Mixable trait generation.
//!
//! This module implements the procedural macro that automatically generates
//! Mixable trait implementations for user-defined structs, making it easy
//! to integrate custom visualization types with Gup's composition system.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Error, Field, Fields, Ident, Result, Type, parse_quote};

/// Configuration parsed from #[mixable] attributes.
#[derive(Debug, Default)]
pub struct MixableConfig {
    /// The type of rendering to generate (points, lines, triangles, custom)
    pub render_type: Option<RenderType>,
    /// Path to a custom render function
    pub custom_render: Option<syn::Path>,
    /// Output type for the Mixable implementation
    pub output_type: Option<Type>,
}

/// Supported render types for automatic generation.
#[derive(Debug, Clone)]
pub enum RenderType {
    Points,
    Lines,
    Triangles,
    Custom(String),
}

/// Analysis of struct fields for render data extraction.
#[derive(Debug, Default)]
pub struct FieldAnalysis {
    /// Fields containing vertex data
    pub vertex_fields: Vec<VertexField>,
    /// Fields containing uniform data
    pub uniform_fields: Vec<UniformField>,
    /// Fields containing texture data
    pub texture_fields: Vec<TextureField>,
}

/// Information about a vertex data field.
#[derive(Debug)]
#[allow(dead_code)]
pub struct VertexField {
    pub name: Ident,
    pub field_type: Type,
    pub vertex_format: VertexFormat,
}

/// Information about a uniform data field.
#[derive(Debug)]
#[allow(dead_code)]
pub struct UniformField {
    pub name: Ident,
    pub field_type: Type,
    pub binding: Option<u32>,
}

/// Information about a texture data field.
#[derive(Debug)]
#[allow(dead_code)]
pub struct TextureField {
    pub name: Ident,
    pub field_type: Type,
    pub binding: Option<u32>,
}

/// Vertex format specification for GPU data.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum VertexFormat {
    Float32x2,
    Float32x3,
    Float32x4,
    Custom(String),
}

/// Generate the Mixable trait implementation for the input struct.
pub fn generate_mixable_impl(input: &DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Parse mixable attributes from the struct
    let config = parse_mixable_attributes(&input.attrs)?;

    // Analyze struct fields to understand data layout
    let field_analysis = analyze_fields(input)?;

    // Generate the appropriate render implementation
    let render_impl = generate_render_implementation(&config, &field_analysis)?;

    // Determine the output type
    let output_type = config.output_type.unwrap_or_else(|| parse_quote! { () });

    let expanded = quote! {
        impl #impl_generics ::gup::Mixable for #name #ty_generics #where_clause {
            type Output = #output_type;

            fn render(&mut self, context: &mut ::gup::RenderContext) -> ::gup::GupResult<()> {
                #render_impl
            }

            fn is_valid(&self) -> bool {
                // Generated validation based on field analysis
                self.validate_fields()
            }

            fn description(&self) -> String {
                format!("{}(auto-derived)", stringify!(#name))
            }
        }

        impl #impl_generics #name #ty_generics #where_clause {
            /// Validate all fields marked for GPU rendering.
            fn validate_fields(&self) -> bool {
                // Basic validation - can be overridden by users
                true
            }

            /// Extract vertex data from fields marked with #[mixable(vertex_data)]
            fn extract_vertex_data(&self) -> Vec<f32> {
                let mut vertex_data = Vec::new();
                // Implementation will be generated based on actual vertex fields
                vertex_data
            }
        }
    };

    Ok(expanded)
}

/// Parse #[mixable] attributes from the struct.
fn parse_mixable_attributes(attrs: &[Attribute]) -> Result<MixableConfig> {
    let mut config = MixableConfig::default();

    for attr in attrs {
        if attr.path().is_ident("mixable") {
            // Use parse_nested_meta for syn 2.0 compatibility
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("render_type") {
                    if let Ok(lit_str) = meta.value()?.parse::<syn::LitStr>() {
                        let render_type_str = lit_str.value();
                        config.render_type = Some(match render_type_str.as_str() {
                            "points" => RenderType::Points,
                            "lines" => RenderType::Lines,
                            "triangles" => RenderType::Triangles,
                            custom => RenderType::Custom(custom.to_string()),
                        });
                    }
                } else if meta.path.is_ident("output_type") {
                    if let Ok(lit_str) = meta.value()?.parse::<syn::LitStr>() {
                        config.output_type = Some(syn::parse_str(&lit_str.value())?);
                    }
                } else if meta.path.is_ident("custom_render") {
                    if let Ok(lit_str) = meta.value()?.parse::<syn::LitStr>() {
                        config.custom_render = Some(syn::parse_str(&lit_str.value())?);
                    }
                }
                Ok(())
            })?;
        }
    }

    Ok(config)
}

/// Analyze struct fields to understand their roles in rendering.
fn analyze_fields(input: &DeriveInput) -> Result<FieldAnalysis> {
    let mut analysis = FieldAnalysis::default();

    if let Data::Struct(data_struct) = &input.data {
        if let Fields::Named(fields) = &data_struct.fields {
            for field in &fields.named {
                analyze_field(field, &mut analysis)?;
            }
        } else {
            return Err(Error::new_spanned(
                input,
                "Mixable can only be derived for structs with named fields",
            ));
        }
    } else {
        return Err(Error::new_spanned(
            input,
            "Mixable can only be derived for structs",
        ));
    }

    Ok(analysis)
}

/// Analyze a single field to determine its role in rendering.
fn analyze_field(field: &Field, analysis: &mut FieldAnalysis) -> Result<()> {
    let field_name = field.ident.as_ref().unwrap().clone();
    let field_type = field.ty.clone();

    // Check field attributes to determine how it should be used
    for attr in &field.attrs {
        if attr.path().is_ident("mixable") {
            // Use parse_nested_meta for syn 2.0 compatibility
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("vertex_data") {
                    let vertex_format = infer_vertex_format(&field_type)?;
                    analysis.vertex_fields.push(VertexField {
                        name: field_name.clone(),
                        field_type: field_type.clone(),
                        vertex_format,
                    });
                } else if meta.path.is_ident("uniform_data") {
                    analysis.uniform_fields.push(UniformField {
                        name: field_name.clone(),
                        field_type: field_type.clone(),
                        binding: None,
                    });
                } else if meta.path.is_ident("texture_data") {
                    analysis.texture_fields.push(TextureField {
                        name: field_name.clone(),
                        field_type: field_type.clone(),
                        binding: None,
                    });
                } else if meta.path.is_ident("binding") {
                    if let Ok(lit_int) = meta.value()?.parse::<syn::LitInt>() {
                        let binding = lit_int.base10_parse::<u32>()?;
                        // Apply binding to the most recently added field
                        if let Some(uniform_field) = analysis.uniform_fields.last_mut() {
                            if uniform_field.name == field_name {
                                uniform_field.binding = Some(binding);
                            }
                        }
                        if let Some(texture_field) = analysis.texture_fields.last_mut() {
                            if texture_field.name == field_name {
                                texture_field.binding = Some(binding);
                            }
                        }
                    }
                }
                Ok(())
            })?;
        }
    }

    Ok(())
}

/// Infer the vertex format from the field type.
fn infer_vertex_format(field_type: &Type) -> Result<VertexFormat> {
    // This is a simplified implementation - in practice, you might want more
    // sophisticated type analysis
    match field_type {
        Type::Path(type_path) => {
            let type_name = type_path.path.segments.last().unwrap().ident.to_string();
            match type_name.as_str() {
                "Vec" => {
                    // Assume Vec<[f32; 2]> for points
                    Ok(VertexFormat::Float32x2)
                }
                _ => Ok(VertexFormat::Custom(type_name)),
            }
        }
        _ => Ok(VertexFormat::Custom("unknown".to_string())),
    }
}

/// Generate the render implementation based on configuration and field analysis.
fn generate_render_implementation(
    config: &MixableConfig,
    _analysis: &FieldAnalysis,
) -> Result<TokenStream> {
    match &config.render_type {
        Some(RenderType::Points) => generate_points_render(),
        Some(RenderType::Lines) => generate_lines_render(),
        Some(RenderType::Triangles) => generate_triangles_render(),
        Some(RenderType::Custom(custom_type)) => generate_custom_render(custom_type),
        None => generate_default_render(),
    }
}

/// Generate point-based rendering implementation.
fn generate_points_render() -> Result<TokenStream> {
    Ok(quote! {
        // Extract vertex data from fields marked as vertex_data
        let vertex_data = self.extract_vertex_data();

        if vertex_data.is_empty() {
            return Ok(());
        }

        // Convert to point format expected by render_points
        let points: Vec<[f32; 2]> = vertex_data
            .chunks_exact(2)
            .map(|chunk| [chunk[0], chunk[1]])
            .collect();

        // Render using the context's point rendering functionality
        context.render_points(&points)
    })
}

/// Generate line-based rendering implementation.
fn generate_lines_render() -> Result<TokenStream> {
    Ok(quote! {
        // Extract vertex data for line rendering
        let vertex_data = self.extract_vertex_data();

        if vertex_data.len() < 4 {
            return Ok(()); // Need at least 2 points for a line
        }

        // Convert to line segments
        let points: Vec<[f32; 2]> = vertex_data
            .chunks_exact(2)
            .map(|chunk| [chunk[0], chunk[1]])
            .collect();

        // Use line rendering - for now, fall back to point rendering
        context.render_points(&points)
    })
}

/// Generate triangle-based rendering implementation.
fn generate_triangles_render() -> Result<TokenStream> {
    Ok(quote! {
        // Extract vertex data for triangle rendering
        let vertex_data = self.extract_vertex_data();

        if vertex_data.len() < 6 {
            return Ok(()); // Need at least 3 points for a triangle
        }

        // Convert to triangles
        let points: Vec<[f32; 2]> = vertex_data
            .chunks_exact(2)
            .map(|chunk| [chunk[0], chunk[1]])
            .collect();

        // Use triangle rendering - for now, fall back to point rendering
        context.render_points(&points)
    })
}

/// Generate custom rendering implementation.
fn generate_custom_render(custom_type: &str) -> Result<TokenStream> {
    let error_msg = format!("Custom render type '{custom_type}' not implemented");
    Ok(quote! {
        return Err(::gup::GupError::render_error(#error_msg.to_string()));
    })
}

/// Generate default (no-op) rendering implementation.
fn generate_default_render() -> Result<TokenStream> {
    Ok(quote! {
        // Default implementation - no rendering
        Ok(())
    })
}
