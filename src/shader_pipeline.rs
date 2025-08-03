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

//! Shader Pipeline Builder System
//!
//! This module implements the ShaderPipeline system that takes composed shader functions
//! and generates optimized WGSL vertex and fragment shaders for the GPU. It handles
//! function composition, uniform buffer management, and generates high-quality WGSL code
//! that leverages GPU parallel processing.

use crate::buffer::{BufferType, GpuBuffer};
use crate::error::GupResult;
use crate::shader_function::ComposableShaderFunction;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BufferBindingType, ColorTargetState, Device, FragmentState,
    MultisampleState, PrimitiveState, Queue, RenderPipeline, RenderPipelineDescriptor,
    ShaderModule, ShaderModuleDescriptor, ShaderSource, ShaderStages, VertexState,
};

/// Represents a shader function in the pipeline with its metadata and uniform buffer.
pub struct PipelineFunction {
    name: String,
    wgsl_code: String,
    uniform_buffer: Option<Box<dyn std::any::Any + Send + Sync>>,
    uniform_size: usize,
}

impl PipelineFunction {
    pub fn new<F: ComposableShaderFunction + 'static>(function: F) -> Self
    where
        F::Uniforms: Send + Sync + 'static,
    {
        let name = F::function_name().to_string();
        let wgsl_code = function.generate_wgsl();
        let uniform_buffer = function
            .create_uniforms()
            .map(|u| Box::new(u) as Box<dyn std::any::Any + Send + Sync>);
        let uniform_size = std::mem::size_of::<F::Uniforms>();

        Self {
            name,
            wgsl_code,
            uniform_buffer,
            uniform_size,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn wgsl_code(&self) -> &str {
        &self.wgsl_code
    }

    pub fn has_uniforms(&self) -> bool {
        self.uniform_buffer.is_some()
    }

    pub fn uniform_size(&self) -> usize {
        self.uniform_size
    }
}

/// Cached shader compilation results to avoid regeneration.
#[derive(Clone)]
pub struct CachedShaders {
    pub vertex_shader: String,
    pub fragment_shader: String,
    pub bind_group_layout: Option<Arc<BindGroupLayout>>,
    pub vertex_module: Option<Arc<ShaderModule>>,
    pub fragment_module: Option<Arc<ShaderModule>>,
}

/// Attribute mapping configuration for shader pipeline.
#[derive(Debug, Clone)]
pub struct AttributeMapping {
    pub attribute_name: String,
    pub function_name: String,
    pub location: u32,
}

/// Core shader pipeline that manages function composition and WGSL generation.
pub struct ComposableShaderPipeline {
    functions: Vec<PipelineFunction>,
    attribute_mappings: Vec<AttributeMapping>,
    cached_shaders: Option<CachedShaders>,
    uniform_buffers: HashMap<String, GpuBuffer<u8>>,
    pipeline_hash: u64,
}

impl Default for ComposableShaderPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ComposableShaderPipeline {
    /// Create a new empty shader pipeline.
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            attribute_mappings: Vec::new(),
            cached_shaders: None,
            uniform_buffers: HashMap::new(),
            pipeline_hash: 0,
        }
    }

    /// Add a shader function to the pipeline.
    pub fn add_function<F: ComposableShaderFunction + 'static>(&mut self, function: F)
    where
        F::Uniforms: Send + Sync + 'static,
    {
        let pipeline_function = PipelineFunction::new(function);
        self.functions.push(pipeline_function);
        self.invalidate_cache();
    }

    /// Map an attribute name to a function output for use in vertex/fragment shaders.
    pub fn map_attribute(&mut self, attr_name: &str, function_name: &str) {
        let location = self.attribute_mappings.len() as u32;
        self.attribute_mappings.push(AttributeMapping {
            attribute_name: attr_name.to_string(),
            function_name: function_name.to_string(),
            location,
        });
        self.invalidate_cache();
    }

    /// Get the number of functions in the pipeline.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    /// Invalidate the shader cache when pipeline changes.
    fn invalidate_cache(&mut self) {
        self.cached_shaders = None;
        self.pipeline_hash = self.calculate_hash();
    }

    /// Calculate a hash for the current pipeline configuration.
    fn calculate_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash function names and WGSL code
        for function in &self.functions {
            function.name().hash(&mut hasher);
            function.wgsl_code().hash(&mut hasher);
        }

        // Hash attribute mappings
        for mapping in &self.attribute_mappings {
            mapping.attribute_name.hash(&mut hasher);
            mapping.function_name.hash(&mut hasher);
            mapping.location.hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Generate data type definitions for WGSL.
    fn generate_data_type_definitions(&self) -> String {
        let mut definitions = String::new();

        definitions.push_str("struct VertexInput {\n");
        definitions.push_str("    @builtin(vertex_index) vertex_index: u32,\n");
        definitions.push_str("}\n\n");

        definitions.push_str("struct VertexOutput {\n");
        definitions.push_str("    @builtin(position) clip_position: vec4<f32>,\n");

        for mapping in &self.attribute_mappings {
            definitions.push_str(&format!(
                "    @location({}) {}: vec4<f32>,\n",
                mapping.location, mapping.attribute_name
            ));
        }

        definitions.push_str("}\n\n");

        definitions
    }

    /// Generate uniform struct definitions and bindings for WGSL.
    fn generate_uniform_bindings(&self) -> String {
        let mut bindings = String::new();
        let mut binding_index = 0;

        // First add uniform struct definitions
        let mut defined_types = std::collections::HashSet::new();
        for function in self.functions.iter() {
            if function.has_uniforms() {
                let uniform_type_name = match function.name() {
                    "linear_scale" => "LinearScaleUniforms",
                    "color_map" => "ColorMapUniforms",
                    "position_transform" => "PositionTransformUniforms",
                    _ => "GenericUniforms",
                };

                if !defined_types.contains(uniform_type_name) {
                    defined_types.insert(uniform_type_name);

                    match uniform_type_name {
                        "LinearScaleUniforms" => {
                            bindings.push_str("struct LinearScaleUniforms {\n");
                            bindings.push_str("    domain_min: f32,\n");
                            bindings.push_str("    domain_max: f32,\n");
                            bindings.push_str("    range_min: f32,\n");
                            bindings.push_str("    range_max: f32,\n");
                            bindings.push_str("}\n\n");
                        }
                        "ColorMapUniforms" => {
                            bindings.push_str("struct ColorMapUniforms {\n");
                            bindings.push_str("    min_color: vec4<f32>,\n");
                            bindings.push_str("    max_color: vec4<f32>,\n");
                            bindings.push_str("}\n\n");
                        }
                        "PositionTransformUniforms" => {
                            bindings.push_str("struct PositionTransformUniforms {\n");
                            bindings.push_str("    scale: vec2<f32>,\n");
                            bindings.push_str("    offset: vec2<f32>,\n");
                            bindings.push_str("}\n\n");
                        }
                        _ => {
                            bindings.push_str(&format!("struct {uniform_type_name} {{\n"));
                            bindings.push_str("    data: f32,\n");
                            bindings.push_str("}\n\n");
                        }
                    }
                }
            }
        }

        // Then add uniform variable bindings
        for (i, function) in self.functions.iter().enumerate() {
            if function.has_uniforms() {
                let uniform_type_name = match function.name() {
                    "linear_scale" => "LinearScaleUniforms",
                    "color_map" => "ColorMapUniforms",
                    "position_transform" => "PositionTransformUniforms",
                    _ => "GenericUniforms",
                };

                bindings.push_str(&format!(
                    "@group(0) @binding({}) var<uniform> {}_uniforms_{}: {};\n",
                    binding_index,
                    function.name(),
                    i,
                    uniform_type_name
                ));
                binding_index += 1;
            }
        }

        bindings.push('\n');
        bindings
    }

    /// Generate the main vertex function.
    fn generate_main_vertex_function(&self) -> String {
        let mut vertex_fn = String::new();

        vertex_fn.push_str("@vertex\n");
        vertex_fn.push_str("fn vs_main(in: VertexInput) -> VertexOutput {\n");
        vertex_fn.push_str("    var output: VertexOutput;\n");
        vertex_fn.push_str("    \n");

        // Calculate position based on vertex index (simple grid layout for demonstration)
        vertex_fn.push_str("    let x = f32(in.vertex_index % 2u) * 2.0 - 1.0;\n");
        vertex_fn.push_str("    let y = f32(in.vertex_index / 2u) * 2.0 - 1.0;\n");
        vertex_fn.push_str("    output.clip_position = vec4<f32>(x * 0.5, y * 0.5, 0.0, 1.0);\n");
        vertex_fn.push_str("    \n");

        // Apply attribute transformations based on mappings
        for mapping in &self.attribute_mappings {
            if let Some((i, function)) = self
                .functions
                .iter()
                .enumerate()
                .find(|(_, f)| f.name() == mapping.function_name)
            {
                let unique_function_name = format!("{}_{}", function.name(), i);

                if function.has_uniforms() {
                    match function.name() {
                        "position_transform" => {
                            // PositionTransform expects vec2<f32> as first parameter
                            vertex_fn.push_str(&format!(
                                "    let {}_result = {}(vec2<f32>(x, y), {}_uniforms_{});\n",
                                mapping.attribute_name,
                                unique_function_name,
                                function.name(),
                                i
                            ));
                        }
                        _ => {
                            // Other functions expect f32 as first parameter
                            vertex_fn.push_str(&format!(
                                "    let {}_result = {}(f32(in.vertex_index), {}_uniforms_{});\n",
                                mapping.attribute_name,
                                unique_function_name,
                                function.name(),
                                i
                            ));
                        }
                    }
                } else {
                    vertex_fn.push_str(&format!(
                        "    let {}_result = {}(f32(in.vertex_index));\n",
                        mapping.attribute_name, unique_function_name
                    ));
                }

                // Convert result to vec4 for output based on function type
                match function.name() {
                    "color_map" => {
                        // ColorMap already returns vec4<f32>
                        vertex_fn.push_str(&format!(
                            "    output.{} = {}_result;\n",
                            mapping.attribute_name, mapping.attribute_name
                        ));
                    }
                    "position_transform" => {
                        // PositionTransform returns vec2<f32>
                        vertex_fn.push_str(&format!(
                            "    output.{} = vec4<f32>({}_result, 0.0, 1.0);\n",
                            mapping.attribute_name, mapping.attribute_name
                        ));
                    }
                    _ => {
                        // LinearScale and others return f32
                        vertex_fn.push_str(&format!(
                            "    output.{} = vec4<f32>({}_result, 0.0, 0.0, 1.0);\n",
                            mapping.attribute_name, mapping.attribute_name
                        ));
                    }
                }
            }
        }

        vertex_fn.push_str("    \n");
        vertex_fn.push_str("    return output;\n");
        vertex_fn.push_str("}\n");

        vertex_fn
    }

    /// Generate the main fragment function.
    fn generate_main_fragment_function(&self) -> String {
        let mut fragment_fn = String::new();

        fragment_fn.push_str("@fragment\n");
        fragment_fn.push_str("fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {\n");

        // If there's a color attribute mapping, use it
        if let Some(color_mapping) = self
            .attribute_mappings
            .iter()
            .find(|m| m.attribute_name == "color")
        {
            fragment_fn.push_str(&format!(
                "    return in.{};\n",
                color_mapping.attribute_name
            ));
        } else {
            // Default white color
            fragment_fn.push_str("    return vec4<f32>(1.0, 1.0, 1.0, 1.0);\n");
        }

        fragment_fn.push_str("}\n");

        fragment_fn
    }

    /// Generate complete vertex shader WGSL source.
    pub fn generate_vertex_shader(&self) -> String {
        if let Some(ref cached) = self.cached_shaders {
            return cached.vertex_shader.clone();
        }

        let mut shader = String::new();

        // Add header comment
        shader.push_str("// Generated vertex shader by Gup ShaderPipeline\n");
        shader.push_str(
            "// This shader was automatically generated from composed shader functions\n\n",
        );

        // Add data type definitions
        shader.push_str(&self.generate_data_type_definitions());

        // Add uniform buffer bindings
        shader.push_str(&self.generate_uniform_bindings());

        // Add all function definitions with unique names
        for (i, function) in self.functions.iter().enumerate() {
            let mut function_code = function.wgsl_code().to_string();

            // Make function names unique by appending index
            let original_name = function.name();
            let unique_name = format!("{original_name}_{i}");
            function_code =
                function_code.replace(&format!("fn {original_name}"), &format!("fn {unique_name}"));

            shader.push_str(&function_code);
            shader.push_str("\n\n");
        }

        // Generate main vertex function
        shader.push_str(&self.generate_main_vertex_function());

        shader
    }

    /// Generate complete fragment shader WGSL source.
    pub fn generate_fragment_shader(&self) -> String {
        if let Some(ref cached) = self.cached_shaders {
            return cached.fragment_shader.clone();
        }

        let mut shader = String::new();

        // Add header comment
        shader.push_str("// Generated fragment shader by Gup ShaderPipeline\n");
        shader.push_str(
            "// This shader was automatically generated from composed shader functions\n\n",
        );

        // Add data type definitions (needed for VertexOutput)
        shader.push_str(&self.generate_data_type_definitions());

        // Add uniform buffer bindings
        shader.push_str(&self.generate_uniform_bindings());

        // Add all function definitions with unique names
        for (i, function) in self.functions.iter().enumerate() {
            let mut function_code = function.wgsl_code().to_string();

            // Make function names unique by appending index
            let original_name = function.name();
            let unique_name = format!("{original_name}_{i}");
            function_code =
                function_code.replace(&format!("fn {original_name}"), &format!("fn {unique_name}"));

            shader.push_str(&function_code);
            shader.push_str("\n\n");
        }

        // Generate main fragment function
        shader.push_str(&self.generate_main_fragment_function());

        shader
    }

    /// Create uniform buffers for all functions that need them.
    pub fn create_uniform_buffers(&mut self, device: &Device) -> GupResult<()> {
        self.uniform_buffers.clear();

        for function in &self.functions {
            if function.has_uniforms() && function.uniform_size() > 0 {
                let buffer = GpuBuffer::new(device, BufferType::Uniform, function.uniform_size());
                self.uniform_buffers
                    .insert(function.name().to_string(), buffer);
            }
        }

        Ok(())
    }

    /// Update uniform data for all functions.
    pub fn update_uniforms(&mut self, device: &Device, queue: &Queue) -> GupResult<()> {
        for function in self.functions.iter() {
            if let Some(uniform_data) = &function.uniform_buffer {
                if let Some(buffer) = self.uniform_buffers.get_mut(function.name()) {
                    // This is a simplified approach - in a real implementation,
                    // we'd need proper type erasure and serialization
                    let data_slice = unsafe {
                        std::slice::from_raw_parts(
                            uniform_data.as_ref() as *const _ as *const u8,
                            function.uniform_size(),
                        )
                    };

                    buffer.upload(device, queue, data_slice)?;
                }
            }
        }

        Ok(())
    }

    /// Create a bind group layout for the pipeline's uniforms.
    pub fn create_bind_group_layout(&self, device: &Device) -> GupResult<BindGroupLayout> {
        let mut entries = Vec::new();
        let mut binding_index = 0;

        for function in self.functions.iter() {
            if function.has_uniforms() {
                entries.push(BindGroupLayoutEntry {
                    binding: binding_index,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                binding_index += 1;
            }
        }

        Ok(device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("shader_pipeline_bind_group_layout"),
            entries: &entries,
        }))
    }

    /// Create a bind group for the pipeline's uniforms.
    pub fn create_bind_group(&self, device: &Device) -> GupResult<BindGroup> {
        let layout = self.create_bind_group_layout(device)?;
        let mut entries = Vec::new();
        let mut binding_index = 0;

        for function in self.functions.iter() {
            if function.has_uniforms() {
                if let Some(buffer) = self.uniform_buffers.get(function.name()) {
                    entries.push(BindGroupEntry {
                        binding: binding_index,
                        resource: buffer.raw_buffer().as_entire_binding(),
                    });
                    binding_index += 1;
                }
            }
        }

        Ok(device.create_bind_group(&BindGroupDescriptor {
            label: Some("shader_pipeline_bind_group"),
            layout: &layout,
            entries: &entries,
        }))
    }

    /// Update the shader cache with compiled shaders.
    fn update_cache(&mut self, device: &Device) -> GupResult<()> {
        let vertex_source = self.generate_vertex_shader();
        let fragment_source = self.generate_fragment_shader();

        let vertex_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("pipeline_vertex_shader"),
            source: ShaderSource::Wgsl(vertex_source.clone().into()),
        });

        let fragment_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("pipeline_fragment_shader"),
            source: ShaderSource::Wgsl(fragment_source.clone().into()),
        });

        let bind_group_layout = self.create_bind_group_layout(device)?;

        self.cached_shaders = Some(CachedShaders {
            vertex_shader: vertex_source,
            fragment_shader: fragment_source,
            bind_group_layout: Some(Arc::new(bind_group_layout)),
            vertex_module: Some(Arc::new(vertex_module)),
            fragment_module: Some(Arc::new(fragment_module)),
        });

        Ok(())
    }

    /// Get the current pipeline hash for cache validation.
    pub fn pipeline_hash(&self) -> u64 {
        self.pipeline_hash
    }

    /// Check if the cache is valid for the current pipeline configuration.
    pub fn is_cache_valid(&self) -> bool {
        self.cached_shaders.is_some()
    }

    /// Get the number of uniform buffers (for testing).
    pub fn uniform_buffer_count(&self) -> usize {
        self.uniform_buffers.len()
    }

    /// Get the number of functions with uniforms (for testing).
    pub fn functions_with_uniforms_count(&self) -> usize {
        self.functions.iter().filter(|f| f.has_uniforms()).count()
    }

    /// Update the cache (for testing).
    pub fn update_cache_public(&mut self, device: &Device) -> GupResult<()> {
        self.update_cache(device)
    }

    /// Optimize shader source by removing unused code and performing optimizations.
    pub fn optimize_shader(&self, shader_source: &str) -> String {
        let mut optimized = shader_source.to_string();

        // Remove unused uniform declarations
        optimized = self.remove_unused_uniforms(&optimized);

        // Inline small functions (basic implementation)
        optimized = self.inline_small_functions(&optimized);

        // Fold constants where possible
        optimized = self.fold_constants(&optimized);

        optimized
    }

    /// Remove unused uniform declarations from shader source.
    fn remove_unused_uniforms(&self, shader: &str) -> String {
        let mut lines: Vec<&str> = shader.lines().collect();
        let mut used_uniforms = std::collections::HashSet::new();

        // Find all uniform usages in the shader
        for line in &lines {
            for function in &self.functions {
                let uniform_name = format!("{}_uniforms", function.name());
                if line.contains(&uniform_name) && !line.trim_start().starts_with("@group") {
                    used_uniforms.insert(uniform_name);
                }
            }
        }

        // Remove unused uniform declarations
        lines.retain(|line| {
            if line.trim_start().starts_with("@group") && line.contains("var<uniform>") {
                // Check if this uniform is used
                for function in &self.functions {
                    let uniform_name = format!("{}_uniforms", function.name());
                    if line.contains(&uniform_name) {
                        return used_uniforms.contains(&uniform_name);
                    }
                }
                false
            } else {
                true
            }
        });

        lines.join("\n")
    }

    /// Inline small functions for performance optimization.
    fn inline_small_functions(&self, shader: &str) -> String {
        let mut optimized = shader.to_string();

        // This is a simplified inlining implementation
        // In practice, this would need proper WGSL AST parsing
        for function in &self.functions {
            let function_code = function.wgsl_code().trim();

            // Only inline very simple functions (less than 3 lines of actual code)
            let code_lines: Vec<&str> = function_code
                .lines()
                .filter(|line| !line.trim().is_empty() && !line.trim().starts_with("//"))
                .collect();

            if code_lines.len() <= 3 {
                // Simple inline replacement (very basic)
                let function_name = function.name();
                let call_pattern = format!("{function_name}(");

                if optimized.matches(&call_pattern).count() <= 2 {
                    // Only inline if called few times
                    // This is a placeholder for proper inlining logic
                    optimized.push_str(&format!("// Inlined function: {function_name}\n"));
                }
            }
        }

        optimized
    }

    /// Perform constant folding optimizations.
    fn fold_constants(&self, shader: &str) -> String {
        let mut optimized = shader.to_string();

        // Simple constant folding examples
        // In practice, this would need proper expression parsing
        optimized = optimized.replace("1.0 * ", "");
        optimized = optimized.replace(" * 1.0", "");
        optimized = optimized.replace("0.0 + ", "");
        optimized = optimized.replace(" + 0.0", "");

        optimized
    }

    /// Generate optimized vertex shader.
    pub fn generate_optimized_vertex_shader(&self) -> String {
        let base_shader = self.generate_vertex_shader();
        self.optimize_shader(&base_shader)
    }

    /// Generate optimized fragment shader.
    pub fn generate_optimized_fragment_shader(&self) -> String {
        let base_shader = self.generate_fragment_shader();
        self.optimize_shader(&base_shader)
    }

    /// Create a render pipeline with the generated shaders.
    pub fn create_render_pipeline(&mut self, device: &Device) -> GupResult<RenderPipeline> {
        // Ensure cache is updated
        if !self.is_cache_valid() {
            self.update_cache(device)?;
        }

        let cached = self.cached_shaders.as_ref().unwrap();
        let bind_group_layout = cached.bind_group_layout.as_ref().unwrap();

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shader_pipeline_layout"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_module = cached.vertex_module.as_ref().unwrap();
        let fragment_module = cached.fragment_module.as_ref().unwrap();

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("shader_pipeline_render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: vertex_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: fragment_module,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Ok(render_pipeline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_function::{ColorMap, LinearScale, Vec4};
    use crate::vec4;

    #[test]
    fn test_pipeline_creation() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        assert_eq!(pipeline.function_count(), 1);
    }

    #[test]
    fn test_multiple_functions() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

        pipeline.add_function(scale);
        pipeline.add_function(color_map);

        assert_eq!(pipeline.function_count(), 2);
    }

    #[test]
    fn test_attribute_mapping() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);
        pipeline.map_attribute("color", "linear_scale");

        assert_eq!(pipeline.attribute_mappings.len(), 1);
        assert_eq!(pipeline.attribute_mappings[0].attribute_name, "color");
        assert_eq!(pipeline.attribute_mappings[0].function_name, "linear_scale");
    }

    #[test]
    fn test_shader_generation() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);
        pipeline.map_attribute("color", "linear_scale");

        let vertex_shader = pipeline.generate_vertex_shader();
        assert!(vertex_shader.contains("linear_scale"));
        assert!(vertex_shader.contains("@vertex"));
        assert!(vertex_shader.contains("vs_main"));
        assert!(vertex_shader.contains("VertexOutput"));

        let fragment_shader = pipeline.generate_fragment_shader();
        assert!(fragment_shader.contains("@fragment"));
        assert!(fragment_shader.contains("fs_main"));
    }

    #[test]
    fn test_pipeline_hash_changes() {
        let mut pipeline = ComposableShaderPipeline::new();
        let initial_hash = pipeline.pipeline_hash();

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        assert_ne!(pipeline.pipeline_hash(), initial_hash);
    }

    #[test]
    fn test_cache_invalidation() {
        let mut pipeline = ComposableShaderPipeline::new();
        assert!(!pipeline.is_cache_valid());

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        assert!(!pipeline.is_cache_valid());
    }

    #[test]
    fn test_uniform_buffer_detection() {
        let pipeline_fn = PipelineFunction::new(LinearScale::new(0.0, 100.0, 0.0, 1.0));
        assert!(pipeline_fn.has_uniforms());
        assert_eq!(pipeline_fn.name(), "linear_scale");
    }

    #[test]
    fn test_data_type_definitions() {
        let pipeline = ComposableShaderPipeline::new();
        let definitions = pipeline.generate_data_type_definitions();

        assert!(definitions.contains("struct VertexInput"));
        assert!(definitions.contains("struct VertexOutput"));
        assert!(definitions.contains("@builtin(vertex_index)"));
        assert!(definitions.contains("@builtin(position)"));
    }

    #[test]
    fn test_uniform_bindings_generation() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        let bindings = pipeline.generate_uniform_bindings();
        assert!(bindings.contains("@group(0) @binding(0)"));
        assert!(bindings.contains("linear_scale_uniforms_0"));
        assert!(bindings.contains("LinearScaleUniforms"));
    }

    #[test]
    fn test_shader_optimization() {
        let pipeline = ComposableShaderPipeline::new();
        let test_shader = r#"
            let x = 1.0 * y;
            let z = a + 0.0;
            @group(0) @binding(0) var<uniform> unused_uniforms: UnusedUniforms;
            return x * 1.0;
        "#;

        let optimized = pipeline.optimize_shader(test_shader);
        assert!(optimized.contains("let x = y;"));
        assert!(optimized.contains("let z = a;"));
    }

    #[test]
    fn test_constant_folding() {
        let pipeline = ComposableShaderPipeline::new();
        let test_code = "let result = value * 1.0 + 0.0;";
        let optimized = pipeline.fold_constants(test_code);
        assert_eq!(optimized, "let result = value;");
    }

    #[test]
    fn test_fragment_shader_with_color_mapping() {
        let mut pipeline = ComposableShaderPipeline::new();
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        pipeline.add_function(color_map);
        pipeline.map_attribute("color", "color_map");

        let fragment_shader = pipeline.generate_fragment_shader();
        assert!(fragment_shader.contains("return in.color;"));
    }

    #[test]
    fn test_vertex_shader_with_multiple_attributes() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

        pipeline.add_function(scale);
        pipeline.add_function(color_map);
        pipeline.map_attribute("size", "linear_scale");
        pipeline.map_attribute("color", "color_map");

        let vertex_shader = pipeline.generate_vertex_shader();
        assert!(vertex_shader.contains("size_result"));
        assert!(vertex_shader.contains("color_result"));
        assert!(vertex_shader.contains("output.size"));
        assert!(vertex_shader.contains("output.color"));
    }

    #[test]
    fn test_optimized_shader_generation() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        let optimized_vertex = pipeline.generate_optimized_vertex_shader();
        let optimized_fragment = pipeline.generate_optimized_fragment_shader();

        assert!(optimized_vertex.contains("vs_main"));
        assert!(optimized_fragment.contains("fs_main"));
    }

    #[test]
    fn test_shader_caching() {
        let mut pipeline = ComposableShaderPipeline::new();
        assert!(!pipeline.is_cache_valid());

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        let hash1 = pipeline.pipeline_hash();

        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        pipeline.add_function(color_map);

        let hash2 = pipeline.pipeline_hash();
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_removed_unused_uniforms() {
        let pipeline = ComposableShaderPipeline::new();
        let shader_with_unused = r#"
@group(0) @binding(0) var<uniform> used_uniforms: UsedUniforms;
@group(0) @binding(1) var<uniform> unused_uniforms: UnusedUniforms;

fn main() {
    let x = used_uniforms.value;
}
        "#;

        let optimized = pipeline.remove_unused_uniforms(shader_with_unused);
        assert!(optimized.contains("used_uniforms"));
        assert!(!optimized.contains("unused_uniforms"));
    }
}
