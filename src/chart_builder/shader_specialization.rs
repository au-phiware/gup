// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shader specialization system for optimized GPU pipeline generation.
//!
//! This module generates specialized WGSL shaders based on chart configuration,
//! eliminating redundant operations and enabling pipeline caching.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Describes the data layout for shader generation.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum DataLayout {
    /// Simple f32 x, y coordinates
    SimpleFloat2,
    /// f32 x, y, plus single f32 attribute
    Float2WithScalar,
    /// f32 x, y, plus vec4 color
    Float2WithColor,
    /// f32 x, y, plus vec4 color and f32 size
    Float2WithColorAndSize,
}

impl DataLayout {
    /// Infer data layout from type T.
    ///
    /// This is a simplified version - in a complete implementation,
    /// this would use const generics or type introspection.
    pub fn infer<T>() -> Self {
        // For now, return most common layout
        // A complete implementation would analyze T's structure
        DataLayout::SimpleFloat2
    }

    /// Generate WGSL struct definition for this layout.
    pub fn generate_vertex_input(&self) -> String {
        match self {
            DataLayout::SimpleFloat2 => {
                "struct VertexInput {\n    @location(0) position: vec2<f32>,\n}\n".to_string()
            }
            DataLayout::Float2WithScalar => {
                "struct VertexInput {\n    @location(0) position: vec2<f32>,\n    @location(1) value: f32,\n}\n".to_string()
            }
            DataLayout::Float2WithColor => {
                "struct VertexInput {\n    @location(0) position: vec2<f32>,\n    @location(1) color: vec4<f32>,\n}\n".to_string()
            }
            DataLayout::Float2WithColorAndSize => {
                "struct VertexInput {\n    @location(0) position: vec2<f32>,\n    @location(1) color: vec4<f32>,\n    @location(2) size: f32,\n}\n".to_string()
            }
        }
    }
}

/// Type of accessor function for shader generation.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum AccessorType {
    /// Direct field access (can be inlined)
    DirectField,
    /// Computed value (requires function call)
    Computed,
    /// Constant value (can be uniform)
    Constant,
}

/// Type of mark being rendered.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum MarkType {
    Circle,
    Rectangle,
    Line,
}

impl MarkType {
    /// Generate specialized fragment shader for this mark type.
    pub fn generate_fragment_shader(&self) -> String {
        match self {
            MarkType::Circle => {
                r#"
@fragment
fn fs_main(@builtin(position) coord: vec4<f32>, @location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    // Optimized circle fragment shader
    let center = vec2<f32>(0.5, 0.5);
    let uv = fract(coord.xy);
    let dist = distance(uv, center);
    if (dist > 0.5) {
        discard;
    }
    return color;
}
"#.to_string()
            }
            MarkType::Rectangle => {
                r#"
@fragment
fn fs_main(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    // Simple passthrough for rectangles
    return color;
}
"#.to_string()
            }
            MarkType::Line => {
                r#"
@fragment
fn fs_main(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    // Simple passthrough for lines
    return color;
}
"#.to_string()
            }
        }
    }
}

/// Shader specialization configuration.
///
/// This struct captures all the information needed to generate a specialized
/// shader pipeline for a specific chart configuration.
#[derive(Debug, Clone)]
pub struct ShaderSpecialization {
    /// Data memory layout
    pub data_layout: DataLayout,
    /// Types of accessor functions used
    pub accessor_types: Vec<AccessorType>,
    /// Type of mark being rendered
    pub mark_type: MarkType,
}

impl ShaderSpecialization {
    /// Create a new shader specialization.
    pub fn new(
        data_layout: DataLayout,
        accessor_types: Vec<AccessorType>,
        mark_type: MarkType,
    ) -> Self {
        Self {
            data_layout,
            accessor_types,
            mark_type,
        }
    }

    /// Generate a complete specialized WGSL shader for this configuration.
    pub fn generate_specialized_shader(&self) -> String {
        let mut shader = String::new();

        // Add vertex input structure
        shader.push_str(&self.data_layout.generate_vertex_input());
        shader.push_str("\n");

        // Add vertex output structure
        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) position: vec4<f32>,\n");
        shader.push_str("    @location(0) color: vec4<f32>,\n");
        shader.push_str("}\n\n");

        // Add optimized accessor functions based on types
        shader.push_str(&self.generate_accessor_functions());
        shader.push_str("\n");

        // Add specialized vertex shader
        shader.push_str(&self.generate_vertex_shader());
        shader.push_str("\n");

        // Add specialized fragment shader
        shader.push_str(&self.mark_type.generate_fragment_shader());

        shader
    }

    /// Generate optimized accessor functions.
    fn generate_accessor_functions(&self) -> String {
        let mut functions = String::new();

        for (i, accessor_type) in self.accessor_types.iter().enumerate() {
            match accessor_type {
                AccessorType::DirectField => {
                    // Direct field access can be inlined in vertex shader
                    functions.push_str(&format!(
                        "// Accessor {} - direct field (inlined)\n",
                        i
                    ));
                }
                AccessorType::Computed => {
                    functions.push_str(&format!(
                        "fn computed_accessor_{}(input: VertexInput) -> f32 {{\n    // Computed value\n    return input.position.x + input.position.y;\n}}\n\n",
                        i
                    ));
                }
                AccessorType::Constant => {
                    // Constants should be uniforms
                    functions.push_str(&format!("// Accessor {} - constant (uniform)\n", i));
                }
            }
        }

        functions
    }

    /// Generate specialized vertex shader.
    fn generate_vertex_shader(&self) -> String {
        match self.data_layout {
            DataLayout::SimpleFloat2 => {
                r#"@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.color = vec4<f32>(0.5, 0.5, 0.5, 1.0); // Default color
    return output;
}
"#.to_string()
            }
            DataLayout::Float2WithColor => {
                r#"@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.color = input.color;
    return output;
}
"#.to_string()
            }
            _ => {
                // Generic version for other layouts
                r#"@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.color = vec4<f32>(0.7, 0.7, 0.7, 1.0); // Default gray
    return output;
}
"#.to_string()
            }
        }
    }

    /// Generate a cache key for this specialization.
    ///
    /// Identical configurations produce identical cache keys,
    /// enabling pipeline reuse.
    pub fn cache_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.data_layout.hash(&mut hasher);
        for accessor_type in &self.accessor_types {
            accessor_type.hash(&mut hasher);
        }
        self.mark_type.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_layout_vertex_input() {
        let layout = DataLayout::SimpleFloat2;
        let input = layout.generate_vertex_input();
        assert!(input.contains("struct VertexInput"));
        assert!(input.contains("position: vec2<f32>"));
    }

    #[test]
    fn test_data_layout_with_color() {
        let layout = DataLayout::Float2WithColor;
        let input = layout.generate_vertex_input();
        assert!(input.contains("position: vec2<f32>"));
        assert!(input.contains("color: vec4<f32>"));
    }

    #[test]
    fn test_mark_type_fragment_shader() {
        let circle_shader = MarkType::Circle.generate_fragment_shader();
        assert!(circle_shader.contains("@fragment"));
        assert!(circle_shader.contains("distance"));
        assert!(circle_shader.contains("discard"));

        let rect_shader = MarkType::Rectangle.generate_fragment_shader();
        assert!(rect_shader.contains("@fragment"));
        assert!(rect_shader.contains("return color"));
    }

    #[test]
    fn test_shader_specialization() {
        let spec = ShaderSpecialization::new(
            DataLayout::SimpleFloat2,
            vec![AccessorType::DirectField, AccessorType::DirectField],
            MarkType::Circle,
        );

        let shader = spec.generate_specialized_shader();
        assert!(shader.contains("struct VertexInput"));
        assert!(shader.contains("struct VertexOutput"));
        assert!(shader.contains("@vertex"));
        assert!(shader.contains("@fragment"));
    }

    #[test]
    fn test_cache_key_consistency() {
        let spec1 = ShaderSpecialization::new(
            DataLayout::SimpleFloat2,
            vec![AccessorType::DirectField, AccessorType::DirectField],
            MarkType::Circle,
        );

        let spec2 = ShaderSpecialization::new(
            DataLayout::SimpleFloat2,
            vec![AccessorType::DirectField, AccessorType::DirectField],
            MarkType::Circle,
        );

        // Identical configurations should produce same cache key
        assert_eq!(spec1.cache_key(), spec2.cache_key());
    }

    #[test]
    fn test_cache_key_uniqueness() {
        let spec1 = ShaderSpecialization::new(
            DataLayout::SimpleFloat2,
            vec![AccessorType::DirectField],
            MarkType::Circle,
        );

        let spec2 = ShaderSpecialization::new(
            DataLayout::Float2WithColor,
            vec![AccessorType::DirectField],
            MarkType::Circle,
        );

        let spec3 = ShaderSpecialization::new(
            DataLayout::SimpleFloat2,
            vec![AccessorType::DirectField],
            MarkType::Rectangle,
        );

        // Different configurations should produce different keys
        assert_ne!(spec1.cache_key(), spec2.cache_key());
        assert_ne!(spec1.cache_key(), spec3.cache_key());
        assert_ne!(spec2.cache_key(), spec3.cache_key());
    }

    #[test]
    fn test_accessor_type_generation() {
        let spec = ShaderSpecialization::new(
            DataLayout::SimpleFloat2,
            vec![
                AccessorType::DirectField,
                AccessorType::Computed,
                AccessorType::Constant,
            ],
            MarkType::Circle,
        );

        let shader = spec.generate_specialized_shader();
        assert!(shader.contains("direct field"));
        assert!(shader.contains("computed_accessor_1"));
        assert!(shader.contains("constant"));
    }

    #[test]
    fn test_specialized_vertex_shaders() {
        let spec_simple = ShaderSpecialization::new(
            DataLayout::SimpleFloat2,
            vec![],
            MarkType::Circle,
        );
        let shader_simple = spec_simple.generate_specialized_shader();
        assert!(shader_simple.contains("Default color"));

        let spec_color = ShaderSpecialization::new(
            DataLayout::Float2WithColor,
            vec![],
            MarkType::Circle,
        );
        let shader_color = spec_color.generate_specialized_shader();
        assert!(shader_color.contains("input.color"));
    }
}
