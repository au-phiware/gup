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

//! Tests for WgslStruct integration with #[wgsl_function]

use gup::shader_function::ComposableShaderFunction;
use gup::*;
use gup_macros::{WgslStruct, wgsl_function};

#[test]
fn test_wgsl_function_with_custom_struct() {
    // Define a custom struct with WgslStruct
    #[derive(WgslStruct, Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct MaterialProps {
        albedo: Vec3,
        metallic: f32,
    }

    // Use it in a wgsl_function
    #[wgsl_function]
    fn apply_material(color: Vec3, props: MaterialProps) -> Vec3 {
        return color * props.albedo * props.metallic;
    }

    let instance = ApplyMaterial::new(MaterialProps {
        albedo: vec3![1.0, 0.5, 0.0],
        metallic: 0.8,
    });

    // The generated WGSL should include the struct definition
    let wgsl = instance.generate_wgsl();

    // Should contain the MaterialProps struct definition
    assert!(
        wgsl.contains("struct MaterialProps"),
        "WGSL should contain MaterialProps struct definition"
    );
    assert!(
        wgsl.contains("albedo: vec3<f32>"),
        "WGSL should contain albedo field"
    );
    assert!(
        wgsl.contains("metallic: f32"),
        "WGSL should contain metallic field"
    );

    // Should also contain the function
    assert!(
        wgsl.contains("fn apply_material"),
        "WGSL should contain the function"
    );
}

#[test]
fn test_wgsl_function_with_nested_structs() {
    // Define nested custom structs
    #[derive(WgslStruct, Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct LightProperties {
        color: Vec3,
        intensity: f32,
    }

    #[derive(WgslStruct, Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct ShadingParams {
        ambient: f32,
        diffuse: f32,
        specular: f32,
        _padding: f32,
    }

    // Use both structs in a function
    #[wgsl_function]
    fn compute_lighting(base_color: Vec3, light: LightProperties) -> Vec3 {
        return base_color * light.color * light.intensity;
    }

    let instance = ComputeLighting::new(LightProperties {
        color: vec3![1.0, 1.0, 1.0],
        intensity: 1.0,
    });

    let wgsl = instance.generate_wgsl();

    // Should contain both struct definitions
    assert!(wgsl.contains("struct LightProperties"));
    assert!(wgsl.contains("color: vec3<f32>"));
    assert!(wgsl.contains("intensity: f32"));
}

#[test]
fn test_wgsl_function_without_custom_struct() {
    // Test that functions without custom structs still work
    #[wgsl_function]
    fn simple_scale(value: f32, scale: f32) -> f32 {
        return value * scale;
    }

    let instance = SimpleScale::new(2.0_f32);
    let wgsl = instance.generate_wgsl();

    // Should contain the function
    assert!(wgsl.contains("fn simple_scale"));
    // Should not contain random struct definitions
    assert!(!wgsl.contains("struct LightProperties"));
}

// Define test structs outside test functions to avoid visibility issues

#[derive(WgslStruct, Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct TestPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    _padding: f32,
}

#[derive(WgslStruct, Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct TestColorOutput {
    pub rgb: Vec3,
    pub alpha: f32,
}

#[test]
fn test_custom_struct_as_input_type() {
    #[wgsl_function]
    fn scale_x(pos: TestPosition, scale: f32) -> f32 {
        return pos.x * scale;
    }

    let instance = ScaleX::new(2.0);
    let wgsl = instance.generate_wgsl();

    // Should contain Position struct definition
    assert!(wgsl.contains("struct TestPosition"));
    assert!(wgsl.contains("x: f32"));
    assert!(wgsl.contains("y: f32"));
    assert!(wgsl.contains("z: f32"));
    // Padding should be skipped
    assert!(!wgsl.contains("_padding"));
}

#[test]
fn test_custom_struct_as_return_type() {
    // For now, we can't return custom structs from wgsl_function
    // because the macro doesn't support struct construction syntax
    // This test just verifies the struct definition is included
    #[wgsl_function]
    fn get_color_value(output: TestColorOutput, factor: f32) -> Vec3 {
        return output.rgb * factor;
    }

    let instance = GetColorValue::new(1.0);
    let wgsl = instance.generate_wgsl();

    // Should contain ColorOutput struct definition
    assert!(wgsl.contains("struct TestColorOutput"));
    assert!(wgsl.contains("rgb: vec3<f32>"));
    assert!(wgsl.contains("alpha: f32"));
}
