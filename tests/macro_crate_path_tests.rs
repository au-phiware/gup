// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Crate path resolution tests for the procedural macros (GUP-377).
//!
//! Integration tests are compiled as a separate crate, so this file exercises
//! the macros exactly as a downstream crate would. It deliberately avoids
//! `use gup::*` and instead creates a hostile environment: crate-root modules
//! named `gup`, `bytemuck`, `shader_function` and `mark` that would capture
//! any generated path that is not absolute (`crate::shader_function::…`,
//! bare `gup::…`, bare `bytemuck::…`). Every macro must therefore resolve the
//! library through `::gup` (or an explicit `crate = "…"` override).

#![allow(dead_code)]

// ---- Hostile crate-root items ----------------------------------------------

/// Captures bare `gup::…` paths (which would become ambiguous with the extern
/// crate) — generated code must use `::gup::…`.
mod gup {}

/// Captures bare `bytemuck::…` paths — generated code must use the
/// `gup::__private::bytemuck` re-export.
mod bytemuck {}

/// Captures `crate::shader_function::…` paths emitted by an unfixed macro.
mod shader_function {}

/// Captures `crate::mark::…` paths emitted by an unfixed macro.
mod mark {}

/// A re-export of the library under a different name, used to exercise the
/// `crate = "…"` overrides.
pub mod engine {
    pub use ::gup as core_lib;
}

use ::gup::shader_function::{
    ComposableShaderFunction, ShaderType, ShaderUniform, Vec2, Vec3, Vec4, WgslStructType,
};

// ---- #[wgsl_function] ------------------------------------------------------

#[::gup::proc_macros::wgsl_function]
fn scale_value(value: f32, factor: f32, offset: f32) -> f32 {
    return value * factor + offset;
}

#[::gup::proc_macros::wgsl_function]
fn identity_value(value: f32) -> f32 {
    return value;
}

#[::gup::proc_macros::wgsl_function(crate = "crate::engine::core_lib")]
fn reexported_scale(value: f32, factor: f32) -> f32 {
    return value * factor;
}

#[test]
fn wgsl_function_resolves_without_glob_import() {
    let f = ScaleValue::new(2.0, 1.0);
    let uniforms = f.create_uniforms().expect("uniforms");
    assert_eq!(uniforms.factor, 2.0);
    assert_eq!(uniforms.offset, 1.0);
    assert_eq!(ScaleValue::function_name(), "scale_value");
    assert!(ScaleValue::wgsl_function().contains("fn scale_value"));
    assert_eq!(ScaleValueUniforms::wgsl_type_name(), "ScaleValueUniforms");

    let id = IdentityValue;
    assert!(id.generate_wgsl().contains("fn identity_value"));
    assert!(IdentityValueUniforms::wgsl_struct_definition().contains("IdentityValueUniforms"));
}

#[test]
fn wgsl_function_honours_crate_override() {
    let f = ReexportedScale::new(3.0);
    assert_eq!(ReexportedScale::function_name(), "reexported_scale");
    assert_eq!(f.create_uniforms().expect("uniforms").factor, 3.0);
}

// ---- #[derive(WgslStruct)] with #[wgsl_function] custom types ---------------

#[derive(::gup_macros::WgslStruct, Clone, Copy, Debug, ::bytemuck::Pod, ::bytemuck::Zeroable)]
#[repr(C)]
struct Material {
    albedo: Vec3,
    metallic: f32,
}

#[::gup::proc_macros::wgsl_function]
fn shade(color: Vec3, props: Material) -> Vec3 {
    return color * props.albedo * props.metallic;
}

#[derive(::gup_macros::WgslStruct, Clone, Copy, Debug, ::bytemuck::Pod, ::bytemuck::Zeroable)]
#[repr(C)]
#[gup(crate = "crate::engine::core_lib")]
struct Tint {
    strength: f32,
}

#[test]
fn wgsl_struct_resolves_without_glob_import() {
    assert_eq!(Material::struct_name(), "Material");
    assert!(Material::wgsl_struct_definition().contains("albedo: vec3<f32>"));
    assert_eq!(<Material as ShaderType>::wgsl_type_name(), "Material");
    assert_eq!(Tint::struct_name(), "Tint");

    let props = Material {
        albedo: Vec3::new(1.0, 0.5, 0.0),
        metallic: 0.8,
    };
    let wgsl = Shade::new(props).generate_wgsl();
    assert!(wgsl.contains("struct Material"), "{wgsl}");
    assert!(wgsl.contains("fn shade"), "{wgsl}");
}

// ---- #[derive(ShaderType)] -------------------------------------------------

#[derive(::gup_macros::ShaderType, Clone)]
struct Reading {
    value: f32,
    weight: f32,
}

#[derive(::gup_macros::ShaderType, Clone)]
#[gup(crate = "crate::engine::core_lib")]
struct Sample {
    value: f32,
}

#[test]
fn shader_type_derive_resolves_without_import() {
    assert_eq!(<Reading as ShaderType>::wgsl_type_name(), "Reading");
    assert_eq!(<Reading as ShaderType>::size_bytes(), 8);
    assert_eq!(<Sample as ShaderType>::wgsl_type_name(), "Sample");
}

// ---- #[derive(MarkTypeId)] -------------------------------------------------

#[derive(Debug, Clone, ::gup_macros::MarkTypeId)]
#[mark_type_id = 240]
struct ExternalMark;

#[derive(Debug, Clone, ::gup_macros::MarkTypeId)]
#[mark_type_id = 241]
#[gup(crate = "crate::engine::core_lib")]
struct ReexportedMark;

#[test]
fn mark_type_id_resolves_without_import() {
    use ::gup::mark::MarkTypeIdProvider;
    assert_eq!(ExternalMark::mark_type_id(), 240);
    assert_eq!(ReexportedMark::mark_type_id(), 241);
}

// ---- #[derive(Mark)] -------------------------------------------------------

#[derive(Debug, Clone, ::gup::Mark)]
#[mark(primitive = "triangle")]
struct Arrow {
    #[mark(position)]
    position: Vec2,
    #[mark(size)]
    size: f32,
    #[mark(color)]
    color: Vec4,
}

#[derive(Debug, Clone, ::gup::Mark)]
#[gup(crate = "crate::engine::core_lib")]
struct Badge {
    #[mark(position)]
    position: Vec2,
}

#[test]
fn mark_derive_resolves_without_import() {
    use ::gup::mark::Mark;
    assert_eq!(Arrow::vertex_count(), 3);
    assert_eq!(Arrow::get_attribute_type("size").unwrap(), "f32");
    assert_eq!(Badge::vertex_count(), 4);

    let arrow = Arrow {
        position: Vec2 { x: 1.0, y: 2.0 },
        size: 3.0,
        color: Vec4 {
            x: 0.1,
            y: 0.2,
            z: 0.3,
            w: 1.0,
        },
    };
    let instance = ArrowInstance::from(&arrow);
    assert_eq!(instance.position, [1.0, 2.0]);
    assert_eq!(instance.size, 3.0);
    let bytes: &[u8] = ::gup::__private::bytemuck::bytes_of(&instance);
    assert_eq!(bytes.len(), std::mem::size_of::<ArrowInstance>());
}
