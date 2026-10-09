// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Build-time flattening and run-time linking, end to end, on a small
//! library shaped like gup-core's: a view module, a scale with `Params`,
//! and a mark that imports the view.

use gup_wgsl::WgslModule;
use gup_wgsl::compose::{Library, Source, parse_wgsl, struct_layouts};

fn source(file: &str, text: &str) -> Source {
    Source {
        file: file.into(),
        text: text.into(),
    }
}

const VIEW: &str = "#define_import_path gup::view\n\
                    struct View {\n    size: vec2<f32>,\n    dpr: f32,\n    padding: f32,\n}\n\
                    fn px_to_clip(p: vec2<f32>, view: View) -> vec4<f32> {\n    \
                    return vec4<f32>(p / view.size * 2.0 - 1.0, 0.0, 1.0);\n}\n";
const SCALE: &str = "#define_import_path gup::scale::linear\n\
                     struct Params {\n    k: f32,\n    lo: f32,\n    pad_a: u32,\n    pad_b: u32,\n}\n\
                     fn map(v: f32, p: Params) -> f32 {\n    return p.lo + v * p.k;\n}\n";
const MARK: &str = "#define_import_path gup::marks::dot\n\
                    #import gup::view::{View, px_to_clip}\n\
                    struct Varyings {\n    @builtin(position) clip: vec4<f32>,\n    \
                    @location(0) @interpolate(flat) row: u32,\n}\n\
                    fn vertex(x: f32, row: u32, view: View) -> Varyings {\n    \
                    var out: Varyings;\n    out.clip = px_to_clip(vec2<f32>(x, 0.0), view);\n    \
                    out.row = row;\n    return out;\n}\n";
const GLUE: &str = "// generated\n\
                    #import gup::view::{View}\n\
                    #import gup::marks::dot as dot\n\
                    #import gup::scale::linear as linear\n\
                    struct Encodings {\n    x: linear::Params,\n    size: f32,\n    \
                    size_pad_a: u32,\n    size_pad_b: u32,\n    size_pad_c: u32,\n}\n\
                    @group(0) @binding(0) var<uniform> u_view: View;\n\
                    @group(1) @binding(0) var<uniform> enc: Encodings;\n\
                    @vertex\nfn vs_main(@builtin(instance_index) i: u32, @location(0) x: f32) \
                    -> dot::Varyings {\n    \
                    return dot::vertex(linear::map(x, enc.x), i, u_view);\n}\n\
                    @fragment\nfn fs_main(v: dot::Varyings) -> @location(0) vec4<f32> {\n    \
                    return vec4<f32>(f32(v.row));\n}\n";

fn library() -> Library {
    // Out of dependency order on purpose: the library sorts them.
    Library::new(&[
        source("src/dot.wgsl", MARK),
        source("src/view.wgsl", VIEW),
        source("src/linear.wgsl", SCALE),
    ])
    .unwrap_or_else(|e| panic!("{e}"))
}

/// The run-time modules for `lib`, as a build script would generate them.
fn leak(lib: &Library) -> Vec<&'static WgslModule> {
    let mut out: Vec<&'static WgslModule> = Vec::new();
    for m in lib.modules() {
        let imports: Vec<&'static WgslModule> = m
            .imports
            .iter()
            .map(|p| *out.iter().find(|s| s.import_path == p).unwrap())
            .collect();
        out.push(Box::leak(Box::new(WgslModule {
            import_path: Box::leak(m.import_path.clone().into_boxed_str()),
            wgsl: Box::leak(m.wgsl.clone().into_boxed_str()),
            imports: Box::leak(imports.into_boxed_slice()),
        })));
    }
    out
}

fn sorted_layouts(m: &naga::Module) -> Vec<(String, gup_wgsl::compose::StructLayout)> {
    let mut v = struct_layouts(m);
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

#[test]
fn flattened_modules_are_plain_wgsl_with_flat_names() {
    let lib = library();
    let paths: Vec<_> = lib
        .modules()
        .iter()
        .map(|m| m.import_path.as_str())
        .collect();
    // Dependencies first.
    assert_eq!(
        paths,
        ["gup::view", "gup::marks::dot", "gup::scale::linear"]
    );
    let dot = lib.module("gup::marks::dot").unwrap();
    assert_eq!(dot.imports, ["gup::view"]);
    assert_eq!(dot.items, ["Varyings", "vertex"]);
    assert!(!dot.wgsl.contains('#'), "{}", dot.wgsl);
    assert!(dot.wgsl.contains("struct gup_marks_dot_Varyings {"));
    assert!(dot.wgsl.contains("fn gup_marks_dot_vertex("));
    assert!(dot.wgsl.contains("gup_view_px_to_clip("));
    // Its imports' items are not repeated in its own text.
    assert!(!dot.wgsl.contains("struct gup_view_View"), "{}", dot.wgsl);
    // Each module, with its imports, is valid standalone WGSL.
    for m in lib.modules() {
        parse_wgsl(&m.import_path, &lib.closure_wgsl(&m.import_path))
            .unwrap_or_else(|e| panic!("{e}"));
    }
}

#[test]
fn linked_glue_matches_naga_oil_composition() {
    let mut lib = library();
    let modules = leak(&lib);
    let linked = gup_wgsl::link("glue", GLUE, &modules).unwrap();
    // Line numbers match the glue: imports become comments.
    assert_eq!(
        linked.lines().take(GLUE.lines().count()).count(),
        GLUE.lines().count()
    );
    assert!(linked.starts_with("// generated\n// #import gup::view::{View}\n"));
    let linked = parse_wgsl("linked", &linked).unwrap_or_else(|e| panic!("{e}"));
    let composed = lib.compose("glue", GLUE).unwrap_or_else(|e| panic!("{e}"));
    let entry_points = |m: &naga::Module| {
        m.entry_points
            .iter()
            .map(|e| (e.name.clone(), e.stage))
            .collect::<Vec<_>>()
    };
    assert_eq!(entry_points(&linked), entry_points(&composed));
    assert_eq!(entry_points(&linked).len(), 2);
    assert_eq!(sorted_layouts(&linked), sorted_layouts(&composed));
    assert_eq!(sorted_layouts(&linked).len(), 4);
}

#[test]
fn top_level_shaders_flatten_completely() {
    let mut lib = library();
    let text = lib
        .flatten_shader(
            "src/guide.wgsl",
            "#import gup::view::{View, px_to_clip}\n\
             @group(0) @binding(0) var<uniform> u_view: View;\n\
             @vertex fn vs_main(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {\n    \
             return px_to_clip(p, u_view);\n}\n",
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.contains('#'));
    assert!(text.contains("struct gup_view_View"));
    let module = parse_wgsl("guide", &text).unwrap();
    assert_eq!(module.entry_points.len(), 1);
}

fn error(sources: &[(&str, &str)]) -> String {
    let sources: Vec<_> = sources.iter().map(|(f, t)| source(f, t)).collect();
    Library::new(&sources)
        .expect_err("composition must fail")
        .to_string()
}

#[test]
fn wrong_arity_fails_naming_the_module_and_line() {
    let text = error(&[
        ("src/view.wgsl", VIEW),
        (
            "src/broken.wgsl",
            "#define_import_path gup::broken\n#import gup::view::{View, px_to_clip}\n\n\
             fn f(view: View) -> vec4<f32> {\n    return px_to_clip(vec2<f32>(0.0));\n}\n",
        ),
    ]);
    assert!(text.contains("src/broken.wgsl:4:1"), "{text}");
    assert!(
        text.contains("Requires 2 arguments, but 1 are provided"),
        "{text}"
    );
}

#[test]
fn trailing_digit_members_fail() {
    let text = error(&[(
        "src/digit.wgsl",
        "#define_import_path gup::digit\n\nstruct Params {\n    r0: f32,\n    k: f32,\n    \
         a: f32,\n    b: f32,\n}\n\nfn f(p: Params) -> f32 {\n    return p.r0;\n}\n",
    )]);
    assert!(
        text.contains("must not require substitution according to naga writeback rules: `r0`"),
        "{text}"
    );
    assert!(text.contains("src/digit.wgsl:3:1"), "{text}");
}

#[test]
fn params_must_span_a_multiple_of_16_bytes() {
    let text = error(&[(
        "src/pad.wgsl",
        "#define_import_path gup::scale::pad\nstruct Params {\n    k: f32,\n}\n\
         fn map(v: f32, p: Params) -> f32 {\n    return v * p.k;\n}\n",
    )]);
    assert!(
        text.contains(
            "struct `Params` of `gup::scale::pad` spans 4 bytes; uniform `Params` structs must \
             span a multiple of 16 bytes"
        ),
        "{text}"
    );
}

#[test]
fn layout_attributes_lost_in_the_round_trip_fail() {
    // naga's WGSL writer drops `@align`: the composed module puts `b` at
    // 16, the printed text at 4 (GUP-401).
    let text = error(&[(
        "src/align.wgsl",
        "#define_import_path gup::align\nstruct S {\n    a: f32,\n    @align(16) b: f32,\n}\n\
         fn f(s: S) -> f32 {\n    return s.a + s.b;\n}\n",
    )]);
    assert!(text.contains("struct `gup_align_S` lays out as"), "{text}");
    assert!(text.contains("drops `@align`"), "{text}");
}

#[test]
fn members_the_writer_renames_fail_in_top_level_shaders() {
    let mut lib = library();
    let err = lib
        .flatten_shader(
            "src/rule.wgsl",
            "struct RuleIn {\n    @location(0) p0: vec2<f32>,\n}\n\
             @vertex fn vs_main(r: RuleIn) -> @builtin(position) vec4<f32> {\n    \
             return vec4<f32>(r.p0, 0.0, 1.0);\n}\n",
        )
        .expect_err("p0 is renamed")
        .to_string();
    assert!(err.contains("renamed member `RuleIn.p0` to `p0_`"), "{err}");
}

#[test]
fn only_import_directives_are_allowed() {
    let text = error(&[(
        "src/defs.wgsl",
        "#define_import_path gup::defs\n#ifdef FOO\nfn f() {}\n#endif\n",
    )]);
    assert!(text.contains("src/defs.wgsl:2"), "{text}");
    assert!(
        text.contains("may use only `#define_import_path` and `#import`"),
        "{text}"
    );
}

#[test]
fn colliding_flat_names_fail() {
    let text = error(&[
        ("src/a.wgsl", "#define_import_path a::b\nfn c_d() {}\n"),
        ("src/b.wgsl", "#define_import_path a::b::c\nfn d() {}\n"),
    ]);
    assert!(text.contains("both flatten to `a_b_c_d`"), "{text}");
}

#[test]
fn unknown_imports_fail() {
    let text = error(&[(
        "src/m.wgsl",
        "#define_import_path gup::m\n#import gup::nowhere as n\nfn f() -> f32 {\n    return n::g();\n}\n",
    )]);
    assert!(
        text.contains("imports `gup::nowhere`, which is not a library module"),
        "{text}"
    );
}
