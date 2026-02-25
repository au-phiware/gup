// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end integration tests for the type mapping system.
//!
//! Tests the full pipeline: TypeMapper → RustToWgsl → WgslCodeGen → WGSL text.

#[cfg(test)]
mod tests {
    use crate::transpile::type_map::{MemoryLayout, TypeMapper, TypeMappingErrorKind};
    use crate::transpile::{RustToWgsl, WgslCodeGen};
    use syn::parse_quote;

    // ---------------------------------------------------------------------------
    // Comprehensive type mapping coverage
    // ---------------------------------------------------------------------------

    #[test]
    fn all_primitive_types_map_correctly() {
        let mut mapper = TypeMapper::new();
        let cases: Vec<(syn::Type, &str)> = vec![
            (parse_quote!(f32), "f32"),
            (parse_quote!(i32), "i32"),
            (parse_quote!(u32), "u32"),
            (parse_quote!(bool), "bool"),
        ];
        for (ty, expected) in cases {
            let info = mapper.map_rust_type(&ty).unwrap();
            assert_eq!(info.wgsl_type.to_string(), expected);
            assert_eq!(info.layout, MemoryLayout::new(4, 4));
        }
    }

    #[test]
    fn all_float_vector_types_map_correctly() {
        let mut mapper = TypeMapper::new();
        let cases: Vec<(syn::Type, &str)> = vec![
            (parse_quote!(Vec2), "vec2<f32>"),
            (parse_quote!(Vec3), "vec3<f32>"),
            (parse_quote!(Vec4), "vec4<f32>"),
        ];
        for (ty, expected) in cases {
            let info = mapper.map_rust_type(&ty).unwrap();
            assert_eq!(info.wgsl_type.to_string(), expected);
        }
    }

    #[test]
    fn all_integer_vector_types_map_correctly() {
        let mut mapper = TypeMapper::new();
        let cases: Vec<(syn::Type, &str)> = vec![
            (parse_quote!(IVec2), "vec2<i32>"),
            (parse_quote!(IVec3), "vec3<i32>"),
            (parse_quote!(IVec4), "vec4<i32>"),
            (parse_quote!(UVec2), "vec2<u32>"),
            (parse_quote!(UVec3), "vec3<u32>"),
            (parse_quote!(UVec4), "vec4<u32>"),
            (parse_quote!(BVec2), "vec2<bool>"),
            (parse_quote!(BVec3), "vec3<bool>"),
            (parse_quote!(BVec4), "vec4<bool>"),
        ];
        for (ty, expected) in cases {
            let info = mapper.map_rust_type(&ty).unwrap();
            assert_eq!(
                info.wgsl_type.to_string(),
                expected,
                "Failed for expected: {expected}"
            );
        }
    }

    #[test]
    fn all_matrix_types_map_correctly() {
        let mut mapper = TypeMapper::new();
        let cases: Vec<(syn::Type, &str)> = vec![
            (parse_quote!(Mat2), "mat2x2<f32>"),
            (parse_quote!(Mat3), "mat3x3<f32>"),
            (parse_quote!(Mat4), "mat4x4<f32>"),
            (parse_quote!(Mat2x3), "mat2x3<f32>"),
            (parse_quote!(Mat2x4), "mat2x4<f32>"),
            (parse_quote!(Mat3x2), "mat3x2<f32>"),
            (parse_quote!(Mat3x4), "mat3x4<f32>"),
            (parse_quote!(Mat4x2), "mat4x2<f32>"),
            (parse_quote!(Mat4x3), "mat4x3<f32>"),
        ];
        for (ty, expected) in cases {
            let info = mapper.map_rust_type(&ty).unwrap();
            assert_eq!(
                info.wgsl_type.to_string(),
                expected,
                "Failed for expected: {expected}"
            );
        }
    }

    // ---------------------------------------------------------------------------
    // Array types
    // ---------------------------------------------------------------------------

    #[test]
    fn array_of_scalars() {
        let mut mapper = TypeMapper::new();
        let ty: syn::Type = parse_quote!([f32; 4]);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type.to_string(), "array<f32, 4>");
    }

    #[test]
    fn array_of_vectors() {
        let mut mapper = TypeMapper::new();
        let ty: syn::Type = parse_quote!([Vec3; 8]);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type.to_string(), "array<vec3<f32>, 8>");
    }

    #[test]
    fn nested_arrays() {
        let mut mapper = TypeMapper::new();
        let ty: syn::Type = parse_quote!([[f32; 3]; 2]);
        let info = mapper.map_rust_type(&ty).unwrap();
        assert_eq!(info.wgsl_type.to_string(), "array<array<f32, 3>, 2>");
    }

    // ---------------------------------------------------------------------------
    // Struct registration and WGSL generation
    // ---------------------------------------------------------------------------

    #[test]
    fn register_and_generate_struct() {
        let mut mapper = TypeMapper::new();
        let fields: Vec<(String, syn::Type)> = vec![
            ("position".to_string(), parse_quote!(Vec3)),
            ("scale".to_string(), parse_quote!(f32)),
            ("color".to_string(), parse_quote!(Vec4)),
            ("flags".to_string(), parse_quote!(u32)),
        ];
        let info = mapper
            .register_struct("TransformUniforms", &fields)
            .unwrap();
        assert_eq!(info.wgsl_type.to_string(), "TransformUniforms");

        let wgsl = mapper.generate_struct_definitions();
        assert!(wgsl.contains("struct TransformUniforms {"));
        assert!(wgsl.contains("position: vec3<f32>,"));
        assert!(wgsl.contains("scale: f32,"));
        assert!(wgsl.contains("color: vec4<f32>,"));
        assert!(wgsl.contains("flags: u32,"));
    }

    #[test]
    fn register_nested_structs() {
        let mut mapper = TypeMapper::new();

        // Register inner struct
        let inner_fields: Vec<(String, syn::Type)> = vec![
            ("min_val".to_string(), parse_quote!(f32)),
            ("max_val".to_string(), parse_quote!(f32)),
        ];
        mapper.register_struct("Range", &inner_fields).unwrap();

        // Register outer struct referencing inner
        let outer_fields: Vec<(String, syn::Type)> = vec![
            ("domain".to_string(), parse_quote!(Range)),
            ("value".to_string(), parse_quote!(f32)),
        ];
        mapper
            .register_struct("ScaleConfig", &outer_fields)
            .unwrap();

        let defs = mapper.struct_definitions();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].name, "Range");
        assert_eq!(defs[1].name, "ScaleConfig");

        let wgsl = mapper.generate_struct_definitions();
        assert!(wgsl.contains("struct Range {"));
        assert!(wgsl.contains("struct ScaleConfig {"));
        assert!(wgsl.contains("domain: Range,"));
    }

    // ---------------------------------------------------------------------------
    // Memory layout
    // ---------------------------------------------------------------------------

    #[test]
    fn layout_vec3_alignment_quirk() {
        let mut mapper = TypeMapper::new();
        let ty: syn::Type = parse_quote!(Vec3);
        let info = mapper.map_rust_type(&ty).unwrap();
        // vec3 size = 12, but alignment = 16 (rounded up to vec4)
        assert_eq!(info.layout.size, 12);
        assert_eq!(info.layout.align, 16);
    }

    #[test]
    fn layout_mat4x4() {
        let mut mapper = TypeMapper::new();
        let ty: syn::Type = parse_quote!(Mat4);
        let info = mapper.map_rust_type(&ty).unwrap();
        // 4 columns × vec4(16 bytes) = 64 bytes
        assert_eq!(info.layout.size, 64);
        assert_eq!(info.layout.align, 16);
    }

    #[test]
    fn struct_layout_with_vec3_and_scalar() {
        let mut mapper = TypeMapper::new();
        let fields: Vec<(String, syn::Type)> = vec![
            ("position".to_string(), parse_quote!(Vec3)),
            ("w".to_string(), parse_quote!(f32)),
        ];
        let layout = mapper.compute_struct_layout(&fields).unwrap();
        // Vec3: offset 0, 12 bytes (16-byte align)
        // f32: offset 12 (4-byte align OK), 4 bytes
        // Total: 16 bytes, struct align: 16
        assert_eq!(layout.size, 16);
        assert_eq!(layout.align, 16);
    }

    // ---------------------------------------------------------------------------
    // Error diagnostics
    // ---------------------------------------------------------------------------

    #[test]
    fn error_all_unsupported_types() {
        let mut mapper = TypeMapper::new();
        let unsupported: Vec<syn::Type> = vec![
            parse_quote!(f64),
            parse_quote!(i8),
            parse_quote!(i16),
            parse_quote!(i64),
            parse_quote!(u8),
            parse_quote!(u16),
            parse_quote!(u64),
            parse_quote!(usize),
            parse_quote!(isize),
            parse_quote!(String),
        ];
        for ty in &unsupported {
            let err = mapper.map_rust_type(ty).unwrap_err();
            assert_eq!(err.kind, TypeMappingErrorKind::IncompatibleType);
        }
    }

    #[test]
    fn error_complex_path_with_suggestion() {
        let mut mapper = TypeMapper::new();
        let ty: syn::Type = parse_quote!(std::f32);
        let err = mapper.map_rust_type(&ty).unwrap_err();
        assert_eq!(err.kind, TypeMappingErrorKind::ComplexPath);
        assert!(err.suggestion.is_some());
        assert!(err.suggestion.unwrap().contains("type name"));
    }

    #[test]
    fn error_struct_field_reports_field_name() {
        let mut mapper = TypeMapper::new();
        let fields: Vec<(String, syn::Type)> = vec![
            ("good".to_string(), parse_quote!(f32)),
            ("bad".to_string(), parse_quote!(f64)),
        ];
        let err = mapper.register_struct("MyStruct", &fields).unwrap_err();
        assert_eq!(err.kind, TypeMappingErrorKind::InvalidStructField);
        assert!(err.message.contains("bad"));
        assert!(err.message.contains("MyStruct"));
    }

    // ---------------------------------------------------------------------------
    // Function signature validation
    // ---------------------------------------------------------------------------

    #[test]
    fn validate_valid_function_signature() {
        let mut mapper = TypeMapper::new();
        let sig: syn::Signature = parse_quote! {
            fn transform(value: Vec3, scale: f32) -> Vec4
        };
        assert!(mapper.validate_function_signature(&sig).is_ok());
    }

    #[test]
    fn validate_invalid_function_signature() {
        let mut mapper = TypeMapper::new();
        let sig: syn::Signature = parse_quote! {
            fn bad(value: f64) -> f64
        };
        let errors = mapper.validate_function_signature(&sig).unwrap_err();
        assert_eq!(errors.len(), 2); // param + return type
    }

    // ---------------------------------------------------------------------------
    // Full pipeline: Rust → convert → codegen → WGSL text
    // ---------------------------------------------------------------------------

    fn transpile(func: &syn::ItemFn, uniform_params: impl IntoIterator<Item = String>) -> String {
        let mut converter = RustToWgsl::new(uniform_params);
        let wgsl_func = converter.convert_function(func).unwrap();
        let mut codegen = WgslCodeGen::new();
        codegen.generate_function(&wgsl_func)
    }

    #[test]
    fn pipeline_vec3_return_type() {
        let func: syn::ItemFn = parse_quote! {
            fn make_pos(x: f32) -> Vec3 {
                return Vec3(x, 0.0, 1.0);
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("-> vec3<f32>"));
        assert!(wgsl.contains("vec3<f32>(x, 0.0, 1.0)"));
    }

    #[test]
    fn pipeline_ivec2_constructor() {
        let func: syn::ItemFn = parse_quote! {
            fn make_coord(x: i32, y: i32) -> IVec2 {
                return IVec2(x, y);
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("-> vec2<i32>"));
        assert!(wgsl.contains("vec2<i32>(x, y)"));
    }

    #[test]
    fn pipeline_uvec3_constructor() {
        let func: syn::ItemFn = parse_quote! {
            fn make_index(x: u32) -> UVec3 {
                return UVec3(x, 0u32, 1u32);
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("-> vec3<u32>"));
        assert!(wgsl.contains("vec3<u32>(x, 0u, 1u)"));
    }

    #[test]
    fn pipeline_mat4_uniform_param() {
        let func: syn::ItemFn = parse_quote! {
            fn apply(pos: Vec4, transform: Mat4) -> Vec4 {
                return pos;
            }
        };
        let wgsl = transpile(&func, ["transform".to_string()]);
        assert!(wgsl.contains("pos: vec4<f32>"));
        assert!(wgsl.contains("-> vec4<f32>"));
    }

    #[test]
    fn pipeline_array_parameter() {
        let func: syn::ItemFn = parse_quote! {
            fn get_first(data: [f32; 4]) -> f32 {
                return data[0];
            }
        };
        let wgsl = transpile(&func, std::iter::empty::<String>());
        assert!(wgsl.contains("data: array<f32, 4>"));
        assert!(wgsl.contains("data[0]"));
    }

    #[test]
    fn pipeline_reference_parameter_transparent() {
        // References should be stripped — WGSL has no reference syntax
        let mut converter = RustToWgsl::new(std::iter::empty::<String>());
        let ty: syn::Type = parse_quote!(&f32);
        let wgsl_type = converter.convert_type(&ty).unwrap();
        assert_eq!(wgsl_type.to_string(), "f32");
    }

    #[test]
    fn pipeline_struct_with_all_field_types() {
        let mut mapper = TypeMapper::new();
        let fields: Vec<(String, syn::Type)> = vec![
            ("a".to_string(), parse_quote!(f32)),
            ("b".to_string(), parse_quote!(i32)),
            ("c".to_string(), parse_quote!(u32)),
            ("d".to_string(), parse_quote!(bool)),
            ("e".to_string(), parse_quote!(Vec2)),
            ("f".to_string(), parse_quote!(Vec3)),
            ("g".to_string(), parse_quote!(Vec4)),
            ("h".to_string(), parse_quote!(Mat4)),
            ("i".to_string(), parse_quote!([f32; 4])),
        ];
        let info = mapper.register_struct("AllTypes", &fields).unwrap();
        assert!(info.requires_definition);

        let wgsl = mapper.generate_struct_definitions();
        assert!(wgsl.contains("struct AllTypes {"));
        assert!(wgsl.contains("a: f32,"));
        assert!(wgsl.contains("b: i32,"));
        assert!(wgsl.contains("c: u32,"));
        assert!(wgsl.contains("d: bool,"));
        assert!(wgsl.contains("e: vec2<f32>,"));
        assert!(wgsl.contains("f: vec3<f32>,"));
        assert!(wgsl.contains("g: vec4<f32>,"));
        assert!(wgsl.contains("h: mat4x4<f32>,"));
        assert!(wgsl.contains("i: array<f32, 4>,"));
    }
}
