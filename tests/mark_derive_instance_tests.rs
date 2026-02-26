// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the Mark derive macro's GPU instance buffer generation.
//!
//! Verifies that `#[mark(position)]`, `#[mark(color)]`, `#[mark(size)]`, and
//! other field annotations correctly generate `{Name}Instance` structs with
//! proper WGSL alignment, `bytemuck` compatibility, and `From` conversions.

use gup::mark::Mark;
use gup::shader_function::{Vec2, Vec3, Vec4};

// ============================================================
// Test marks with various field annotation combinations
// ============================================================

/// Mark with all three standard annotations: position, color, size.
#[derive(Debug, Clone, gup_macros::Mark)]
#[mark(primitive = "quad")]
pub struct AnnotatedDiamond {
    #[mark(position)]
    pub center: Vec2,
    #[mark(size)]
    pub size: f32,
    #[mark(color)]
    pub color: Vec4,
}

/// Mark with only a position annotation.
#[derive(Debug, Clone, gup_macros::Mark)]
pub struct PositionOnly {
    #[mark(position)]
    pub pos: Vec2,
    pub extra: f32, // not annotated — excluded from instance
}

/// Mark with position and color (tests padding between vec2 and vec4).
#[derive(Debug, Clone, gup_macros::Mark)]
pub struct PosColor {
    #[mark(position)]
    pub position: Vec2,
    #[mark(color)]
    pub fill: Vec4,
}

/// Mark with all scalar fields (no padding needed between them).
#[derive(Debug, Clone, gup_macros::Mark)]
pub struct AllScalars {
    #[mark(size)]
    pub width: f32,
    #[mark(size)]
    pub height: f32,
    #[mark(attribute)]
    pub opacity: f32,
}

/// Mark with a Vec3 field (16-byte alignment, 12-byte size).
#[derive(Debug, Clone, gup_macros::Mark)]
pub struct WithVec3 {
    #[mark(position)]
    pub pos: Vec3,
    #[mark(size)]
    pub scale: f32,
}

/// Mark with custom role names (not just position/color/size).
#[derive(Debug, Clone, gup_macros::Mark)]
pub struct CustomRoles {
    #[mark(position)]
    pub center: Vec2,
    #[mark(rotation)]
    pub angle: f32,
    #[mark(color)]
    pub tint: Vec4,
}

/// Mark with no annotations (should NOT generate instance struct).
#[derive(Debug, Clone, gup_macros::Mark)]
pub struct NoAnnotations {
    pub center: Vec2,
    pub size: f32,
    pub color: Vec4,
}

/// Mark with triangle primitive and annotations.
#[derive(Debug, Clone, gup_macros::Mark)]
#[mark(primitive = "triangle")]
pub struct AnnotatedArrow {
    #[mark(position)]
    pub tip: Vec2,
    #[mark(size)]
    pub length: f32,
    #[mark(color)]
    pub color: Vec4,
}

/// Mark with multiple vec4 fields (no inter-field padding needed).
#[derive(Debug, Clone, gup_macros::Mark)]
pub struct MultiVec4 {
    #[mark(color)]
    pub fill_color: Vec4,
    #[mark(color)]
    pub stroke_color: Vec4,
}

/// Mark with integer fields.
#[derive(Debug, Clone, gup_macros::Mark)]
pub struct WithIntegers {
    #[mark(position)]
    pub pos: Vec2,
    #[mark(attribute)]
    pub count: u32,
    #[mark(attribute)]
    pub index: i32,
}

// ============================================================
// Instance struct existence and type checks
// ============================================================

#[test]
fn annotated_diamond_generates_instance_struct() {
    // Verify the instance type exists and is constructible via From
    let diamond = AnnotatedDiamond {
        center: Vec2 { x: 1.0, y: 2.0 },
        size: 5.0,
        color: Vec4 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
    };
    let instance = AnnotatedDiamondInstance::from(&diamond);
    assert_eq!(instance.center, [1.0, 2.0]);
    assert_eq!(instance.size, 5.0);
    assert_eq!(instance.color, [1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn owned_from_conversion_works() {
    let diamond = AnnotatedDiamond {
        center: Vec2 { x: 3.0, y: 4.0 },
        size: 10.0,
        color: Vec4 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
            w: 0.5,
        },
    };
    // From<AnnotatedDiamond> (owned) should also work
    let instance: AnnotatedDiamondInstance = diamond.into();
    assert_eq!(instance.center, [3.0, 4.0]);
    assert_eq!(instance.size, 10.0);
    assert_eq!(instance.color, [0.0, 1.0, 0.0, 0.5]);
}

#[test]
fn position_only_generates_instance() {
    let mark = PositionOnly {
        pos: Vec2 { x: 5.0, y: 6.0 },
        extra: 99.0,
    };
    let instance = PositionOnlyInstance::from(&mark);
    assert_eq!(instance.pos, [5.0, 6.0]);
    // extra is NOT in the instance (no annotation)
}

#[test]
fn pos_color_generates_instance_with_padding() {
    let mark = PosColor {
        position: Vec2 { x: 1.0, y: 2.0 },
        fill: Vec4 {
            x: 0.5,
            y: 0.5,
            z: 0.5,
            w: 1.0,
        },
    };
    let instance = PosColorInstance::from(&mark);
    assert_eq!(instance.position, [1.0, 2.0]);
    assert_eq!(instance.fill, [0.5, 0.5, 0.5, 1.0]);
}

#[test]
fn all_scalars_no_inter_padding() {
    let mark = AllScalars {
        width: 10.0,
        height: 20.0,
        opacity: 0.8,
    };
    let instance = AllScalarsInstance::from(&mark);
    assert_eq!(instance.width, 10.0);
    assert_eq!(instance.height, 20.0);
    assert_eq!(instance.opacity, 0.8);
}

#[test]
fn vec3_field_alignment() {
    let mark = WithVec3 {
        pos: Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            _padding: 0.0,
        },
        scale: 4.0,
    };
    let instance = WithVec3Instance::from(&mark);
    assert_eq!(instance.pos, [1.0, 2.0, 3.0]);
    assert_eq!(instance.scale, 4.0);
}

#[test]
fn custom_role_names_work() {
    let mark = CustomRoles {
        center: Vec2 { x: 0.0, y: 0.0 },
        angle: 1.57,
        tint: Vec4 {
            x: 1.0,
            y: 1.0,
            z: 0.0,
            w: 1.0,
        },
    };
    let instance = CustomRolesInstance::from(&mark);
    assert_eq!(instance.center, [0.0, 0.0]);
    assert!((instance.angle - 1.57).abs() < f32::EPSILON);
    assert_eq!(instance.tint, [1.0, 1.0, 0.0, 1.0]);
}

#[test]
fn triangle_primitive_with_annotations() {
    let mark = AnnotatedArrow {
        tip: Vec2 { x: 5.0, y: 10.0 },
        length: 3.0,
        color: Vec4 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
            w: 1.0,
        },
    };
    let instance = AnnotatedArrowInstance::from(&mark);
    assert_eq!(instance.tip, [5.0, 10.0]);
    assert_eq!(instance.length, 3.0);
    assert_eq!(instance.color, [0.0, 0.0, 1.0, 1.0]);

    // Also verify the Mark trait still works correctly for triangle
    assert_eq!(AnnotatedArrow::vertex_count(), 3);
    assert_eq!(AnnotatedArrow::index_count(), None);
}

#[test]
fn multi_vec4_no_padding() {
    let mark = MultiVec4 {
        fill_color: Vec4 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
        stroke_color: Vec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
    };
    let instance = MultiVec4Instance::from(&mark);
    assert_eq!(instance.fill_color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(instance.stroke_color, [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn integer_fields() {
    let mark = WithIntegers {
        pos: Vec2 { x: 1.0, y: 2.0 },
        count: 42,
        index: -1,
    };
    let instance = WithIntegersInstance::from(&mark);
    assert_eq!(instance.pos, [1.0, 2.0]);
    assert_eq!(instance.count, 42);
    assert_eq!(instance.index, -1);
}

// ============================================================
// WGSL alignment and size validation
// ============================================================

#[test]
fn annotated_diamond_instance_size_and_alignment() {
    // AnnotatedDiamond has: center (Vec2, 8), size (f32, 4), color (Vec4, 16)
    // Layout:
    //   offset 0:  center [f32; 2] = 8 bytes
    //   offset 8:  size f32 = 4 bytes
    //   offset 12: _pad0 f32 = 4 bytes (pad for vec4 at 16)
    //   offset 16: color [f32; 4] = 16 bytes
    //   Total: 32 bytes (multiple of 16 ✓)
    assert_eq!(std::mem::size_of::<AnnotatedDiamondInstance>(), 32);
}

#[test]
fn pos_color_instance_alignment() {
    // PosColor has: position (Vec2, 8), fill (Vec4, 16)
    // Layout:
    //   offset 0:  position [f32; 2] = 8 bytes
    //   offset 8:  _pad0 [f32; 2] = 8 bytes (pad for vec4 at 16)
    //   offset 16: fill [f32; 4] = 16 bytes
    //   Total: 32 bytes (multiple of 16 ✓)
    assert_eq!(std::mem::size_of::<PosColorInstance>(), 32);
}

#[test]
fn all_scalars_instance_size() {
    // AllScalars has: width (f32, 4), height (f32, 4), opacity (f32, 4)
    // Layout:
    //   offset 0: width f32 = 4 bytes
    //   offset 4: height f32 = 4 bytes
    //   offset 8: opacity f32 = 4 bytes
    //   Total: 12 bytes (multiple of 4 ✓)
    assert_eq!(std::mem::size_of::<AllScalarsInstance>(), 12);
}

#[test]
fn vec3_instance_size_and_padding() {
    // WithVec3 has: pos (Vec3, 12 bytes, 16-byte align), scale (f32, 4)
    // Layout:
    //   offset 0:  pos [f32; 3] = 12 bytes
    //   offset 12: scale f32 = 4 bytes
    //   Total: 16 bytes (multiple of 16 ✓)
    assert_eq!(std::mem::size_of::<WithVec3Instance>(), 16);
}

#[test]
fn custom_roles_instance_size() {
    // CustomRoles has: center (Vec2, 8), angle (f32, 4), tint (Vec4, 16)
    // Layout:
    //   offset 0:  center [f32; 2] = 8 bytes
    //   offset 8:  angle f32 = 4 bytes
    //   offset 12: _pad0 f32 = 4 bytes (pad for vec4 at 16)
    //   offset 16: tint [f32; 4] = 16 bytes
    //   Total: 32 bytes (multiple of 16 ✓)
    assert_eq!(std::mem::size_of::<CustomRolesInstance>(), 32);
}

#[test]
fn multi_vec4_instance_size() {
    // MultiVec4 has: fill_color (Vec4, 16), stroke_color (Vec4, 16)
    // Layout:
    //   offset 0:  fill_color [f32; 4] = 16 bytes
    //   offset 16: stroke_color [f32; 4] = 16 bytes
    //   Total: 32 bytes (multiple of 16 ✓)
    assert_eq!(std::mem::size_of::<MultiVec4Instance>(), 32);
}

#[test]
fn position_only_instance_size() {
    // PositionOnly has: pos (Vec2, 8)
    // Layout:
    //   offset 0: pos [f32; 2] = 8 bytes
    //   Total: 8 bytes (multiple of 8 ✓)
    assert_eq!(std::mem::size_of::<PositionOnlyInstance>(), 8);
}

#[test]
fn integers_instance_size() {
    // WithIntegers has: pos (Vec2, 8), count (u32, 4), index (i32, 4)
    // Layout:
    //   offset 0: pos [f32; 2] = 8 bytes
    //   offset 8: count u32 = 4 bytes
    //   offset 12: index i32 = 4 bytes
    //   Total: 16 bytes (multiple of 8 ✓)
    assert_eq!(std::mem::size_of::<WithIntegersInstance>(), 16);
}

#[test]
fn struct_size_is_multiple_of_max_alignment() {
    // For every generated instance struct, size should be a multiple of its
    // maximum field alignment. This ensures correct array stride for storage buffers.

    let size = std::mem::size_of::<AnnotatedDiamondInstance>();
    assert_eq!(
        size % 16,
        0,
        "AnnotatedDiamondInstance size {size} not multiple of 16"
    );

    let size = std::mem::size_of::<PosColorInstance>();
    assert_eq!(
        size % 16,
        0,
        "PosColorInstance size {size} not multiple of 16"
    );

    let size = std::mem::size_of::<AllScalarsInstance>();
    assert_eq!(
        size % 4,
        0,
        "AllScalarsInstance size {size} not multiple of 4"
    );

    let size = std::mem::size_of::<WithVec3Instance>();
    assert_eq!(
        size % 16,
        0,
        "WithVec3Instance size {size} not multiple of 16"
    );

    let size = std::mem::size_of::<MultiVec4Instance>();
    assert_eq!(
        size % 16,
        0,
        "MultiVec4Instance size {size} not multiple of 16"
    );
}

// ============================================================
// Bytemuck compatibility
// ============================================================

#[test]
fn instance_structs_are_bytemuck_compatible() {
    // Verify that casting to bytes and back works (proves Pod + Zeroable)
    let instance = AnnotatedDiamondInstance::from(&AnnotatedDiamond {
        center: Vec2 { x: 1.0, y: 2.0 },
        size: 3.0,
        color: Vec4 {
            x: 0.5,
            y: 0.5,
            z: 0.5,
            w: 1.0,
        },
    });

    let bytes: &[u8] = bytemuck::bytes_of(&instance);
    let round_tripped: &AnnotatedDiamondInstance = bytemuck::from_bytes(bytes);
    assert_eq!(round_tripped.center, [1.0, 2.0]);
    assert_eq!(round_tripped.size, 3.0);
    assert_eq!(round_tripped.color, [0.5, 0.5, 0.5, 1.0]);
}

#[test]
fn instance_array_bytemuck_cast() {
    // Verify that arrays of instances can be cast to byte slices (storage buffer upload)
    let instances = vec![
        AnnotatedDiamondInstance::from(&AnnotatedDiamond {
            center: Vec2 { x: 0.0, y: 0.0 },
            size: 1.0,
            color: Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        }),
        AnnotatedDiamondInstance::from(&AnnotatedDiamond {
            center: Vec2 { x: 1.0, y: 1.0 },
            size: 2.0,
            color: Vec4 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
                w: 1.0,
            },
        }),
    ];

    let bytes: &[u8] = bytemuck::cast_slice(&instances);
    let expected_bytes = 2 * std::mem::size_of::<AnnotatedDiamondInstance>();
    assert_eq!(bytes.len(), expected_bytes);
}

#[test]
fn zeroed_instance_is_valid() {
    // bytemuck::Zeroable means zeroed memory is a valid value
    let zeroed: AnnotatedDiamondInstance = bytemuck::Zeroable::zeroed();
    assert_eq!(zeroed.center, [0.0, 0.0]);
    assert_eq!(zeroed.size, 0.0);
    assert_eq!(zeroed.color, [0.0, 0.0, 0.0, 0.0]);
}

// ============================================================
// Mark trait still works correctly with annotations
// ============================================================

#[test]
fn annotated_mark_still_implements_mark_trait() {
    // Verify the Mark trait is still generated correctly alongside the instance struct
    assert_eq!(AnnotatedDiamond::vertex_count(), 4);
    assert_eq!(AnnotatedDiamond::index_count(), Some(6));
    assert_eq!(AnnotatedDiamond::generate_vertices().len(), 4);
    assert_eq!(
        AnnotatedDiamond::get_attribute_type("center").unwrap(),
        "vec2<f32>"
    );
    assert_eq!(AnnotatedDiamond::get_attribute_type("size").unwrap(), "f32");
    assert_eq!(
        AnnotatedDiamond::get_attribute_type("color").unwrap(),
        "vec4<f32>"
    );
}

#[test]
fn no_annotations_mark_still_valid() {
    // Mark with no annotations should still have a valid Mark impl
    assert_eq!(NoAnnotations::vertex_count(), 4);
    assert_eq!(NoAnnotations::index_count(), Some(6));
    assert_eq!(
        NoAnnotations::get_attribute_type("center").unwrap(),
        "vec2<f32>"
    );
}

// ============================================================
// Debug derive works
// ============================================================

#[test]
fn instance_implements_debug() {
    let instance = AnnotatedDiamondInstance::from(&AnnotatedDiamond {
        center: Vec2 { x: 1.0, y: 2.0 },
        size: 3.0,
        color: Vec4 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
    });
    let debug_str = format!("{instance:?}");
    assert!(debug_str.contains("AnnotatedDiamondInstance"));
}

#[test]
fn instance_implements_clone_copy() {
    let instance = AnnotatedDiamondInstance::from(&AnnotatedDiamond {
        center: Vec2 { x: 1.0, y: 2.0 },
        size: 3.0,
        color: Vec4 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
    });
    let cloned = instance;
    let copied = instance;
    assert_eq!(cloned.center, copied.center);
}

// ============================================================
// MarkValidator compatibility
// ============================================================

#[test]
fn annotated_marks_pass_mark_validator() {
    use gup::mark::validation::assert_mark_valid;

    // Marks with field annotations should still pass all MarkValidator checks
    assert_mark_valid::<AnnotatedDiamond>().expect("AnnotatedDiamond should be valid");
    assert_mark_valid::<PositionOnly>().expect("PositionOnly should be valid");
    assert_mark_valid::<PosColor>().expect("PosColor should be valid");
    assert_mark_valid::<AllScalars>().expect("AllScalars should be valid");
    assert_mark_valid::<WithVec3>().expect("WithVec3 should be valid");
    assert_mark_valid::<CustomRoles>().expect("CustomRoles should be valid");
    assert_mark_valid::<AnnotatedArrow>().expect("AnnotatedArrow should be valid");
    assert_mark_valid::<MultiVec4>().expect("MultiVec4 should be valid");
    assert_mark_valid::<WithIntegers>().expect("WithIntegers should be valid");
    assert_mark_valid::<NoAnnotations>().expect("NoAnnotations should be valid");
}

#[test]
fn annotated_marks_produce_passing_validation_reports() {
    use gup::mark::validation::MarkValidator;

    let report = MarkValidator::<AnnotatedDiamond>::validate();
    assert!(
        report.is_passing(),
        "AnnotatedDiamond validation failed: {}",
        report.summary()
    );

    let report = MarkValidator::<AnnotatedArrow>::validate();
    assert!(
        report.is_passing(),
        "AnnotatedArrow validation failed: {}",
        report.summary()
    );

    let report = MarkValidator::<CustomRoles>::validate();
    assert!(
        report.is_passing(),
        "CustomRoles validation failed: {}",
        report.summary()
    );
}
