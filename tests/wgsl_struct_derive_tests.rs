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

//! Tests for the WgslStruct derive macro

use gup::shader_function::{ShaderType, WgslStructType};
use gup::*;
use gup_macros::WgslStruct;

#[test]
fn test_simple_struct_derivation() {
    #[derive(WgslStruct, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct SimpleStruct {
        x: f32,
        y: f32,
    }

    let wgsl = SimpleStruct::wgsl_struct_definition();
    assert!(wgsl.contains("struct SimpleStruct"));
    assert!(wgsl.contains("x: f32"));
    assert!(wgsl.contains("y: f32"));

    assert_eq!(SimpleStruct::struct_name(), "SimpleStruct");
    assert_eq!(SimpleStruct::wgsl_type_name(), "SimpleStruct");
}

#[test]
fn test_struct_with_vectors() {
    #[derive(WgslStruct, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct VectorStruct {
        position: Vec3,
        color: Vec4,
    }

    let wgsl = VectorStruct::wgsl_struct_definition();
    assert!(wgsl.contains("struct VectorStruct"));
    assert!(wgsl.contains("position: vec3<f32>"));
    assert!(wgsl.contains("color: vec4<f32>"));
}

#[test]
fn test_struct_with_padding() {
    #[derive(WgslStruct, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct PaddedStruct {
        albedo: Vec3,
        metallic: f32,
        _padding: [f32; 3], // Should be skipped in WGSL
    }

    let wgsl = PaddedStruct::wgsl_struct_definition();
    assert!(wgsl.contains("struct PaddedStruct"));
    assert!(wgsl.contains("albedo: vec3<f32>"));
    assert!(wgsl.contains("metallic: f32"));
    // Padding should NOT appear in WGSL
    assert!(!wgsl.contains("_padding"));
    assert!(!wgsl.contains("padding"));
}

#[test]
fn test_struct_with_matrices() {
    #[derive(WgslStruct, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct MatrixStruct {
        transform: Mat4,
        scale: Mat2,
    }

    let wgsl = MatrixStruct::wgsl_struct_definition();
    assert!(wgsl.contains("struct MatrixStruct"));
    assert!(wgsl.contains("transform: mat4x4<f32>"));
    assert!(wgsl.contains("scale: mat2x2<f32>"));
}

#[test]
fn test_struct_with_arrays() {
    #[derive(WgslStruct, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct ArrayStruct {
        values: [f32; 4],
        flags: [u32; 2],
    }

    let wgsl = ArrayStruct::wgsl_struct_definition();
    assert!(wgsl.contains("struct ArrayStruct"));
    assert!(wgsl.contains("values: array<f32, 4>"));
    assert!(wgsl.contains("flags: array<u32, 2>"));
}

#[test]
fn test_size_and_alignment() {
    #[derive(WgslStruct, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct SizedStruct {
        a: f32,
        b: f32,
        c: u32,
    }

    // Size should be 3 * 4 = 12 bytes
    assert_eq!(SizedStruct::size_bytes(), 12);

    // Alignment should be 4 (max of f32 and u32)
    assert_eq!(SizedStruct::alignment(), 4);
}
