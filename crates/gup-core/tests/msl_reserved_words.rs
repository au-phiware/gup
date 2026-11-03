// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! No identifier in Gup's WGSL is a Metal Shading Language reserved word
//! (GUP-591).
//!
//! naga's MSL backend does not rename every user identifier, so a varying
//! named `half` (an MSL type) reached Metal as `metal::float2 half
//! [[user(loc2)]];` and the macOS pipeline failed to compile. This test
//! checks every identifier of every library module and authored shader, and
//! also translates each module to MSL with naga's `msl-out` to prove the
//! module is translatable.

use std::collections::BTreeSet;

use gup_wgsl::compose::{Library, library, parse_wgsl};

/// MSL keywords and type names a WGSL identifier must not use. Vector and
/// matrix types (`float2`, `half4`, `float3x3`) are matched by `reserved`.
const RESERVED: &[&str] = &[
    "half",
    "char",
    "uchar",
    "short",
    "ushort",
    "int",
    "uint",
    "long",
    "ulong",
    "float",
    "double",
    "bool",
    "void",
    "size_t",
    "ptrdiff_t",
    "device",
    "constant",
    "thread",
    "threadgroup",
    "threadgroup_imageblock",
    "ray_data",
    "object_data",
    "kernel",
    "vertex",
    "fragment",
    "mesh",
    "object",
    "visible",
    "stage_in",
    "texture1d",
    "texture2d",
    "texture3d",
    "texturecube",
    "sampler",
    "atomic",
    "metal",
    "using",
    "namespace",
    "template",
    "typename",
    "class",
    "struct",
    "union",
    "enum",
    "auto",
    "register",
    "static",
    "extern",
    "inline",
    "volatile",
    "const",
    "typedef",
    "operator",
    "private",
    "public",
    "protected",
    "this",
    "new",
    "delete",
    "goto",
    "switch",
    "case",
    "default",
    "main",
    "bfloat",
    "array",
    "vec",
    "matrix",
    "saturate",
    "fast",
    "precise",
    "uniform",
    "buffer",
    "if",
    "else",
    "for",
    "while",
    "do",
    "return",
    "break",
    "continue",
    "true",
    "false",
    "nullptr",
    "sizeof",
    "alignas",
    "alignof",
    "decltype",
    "explicit",
    "friend",
    "mutable",
    "noexcept",
    "static_assert",
    "try",
    "catch",
    "throw",
    "virtual",
];

const SCALARS: &[&str] = &[
    "half", "char", "uchar", "short", "ushort", "int", "uint", "long", "ulong", "float", "double",
    "bool", "bfloat",
];

fn reserved(name: &str) -> bool {
    if RESERVED.contains(&name) {
        return true;
    }
    // half2, uint4, float3x3, ...
    let dim = |c: char| ('2'..='4').contains(&c);
    SCALARS.iter().any(|s| {
        name.strip_prefix(s).is_some_and(|rest| {
            let c: Vec<char> = rest.chars().collect();
            match c.as_slice() {
                [a] => dim(*a),
                [a, 'x', b] => dim(*a) && dim(*b),
                _ => false,
            }
        })
    })
}

/// Every user-chosen name in `module`, with where it came from.
fn names(module: &naga::Module) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    let mut add = |what: &str, n: &Option<String>| {
        if let Some(n) = n {
            out.insert((what.to_string(), n.clone()));
        }
    };
    for (_, ty) in module.types.iter() {
        if let naga::TypeInner::Struct { members, .. } = &ty.inner {
            add("struct", &ty.name);
            for m in members {
                add("struct member", &m.name);
            }
        }
    }
    for (_, c) in module.constants.iter() {
        add("constant", &c.name);
    }
    for (_, g) in module.global_variables.iter() {
        add("global", &g.name);
    }
    let mut function = |f: &naga::Function, entry: bool| {
        add(if entry { "entry point" } else { "function" }, &f.name);
        for a in &f.arguments {
            add("argument", &a.name);
        }
        for (_, l) in f.local_variables.iter() {
            add("local", &l.name);
        }
        for n in f.named_expressions.values() {
            add("let", &Some(n.clone()));
        }
    };
    for (_, f) in module.functions.iter() {
        function(f, false);
    }
    for e in &module.entry_points {
        function(&e.function, true);
    }
    out
}

fn offenders(module: &naga::Module) -> Vec<String> {
    names(module)
        .into_iter()
        .filter(|(_, n)| reserved(n))
        .map(|(what, n)| format!("{what} `{n}`"))
        .collect()
}

/// Translate to MSL; a module without entry points still has to validate.
fn to_msl(label: &str, module: &naga::Module) {
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(module)
    .unwrap_or_else(|e| panic!("{label}: {e:?}"));
    let options = naga::back::msl::Options::default();
    let pipeline = naga::back::msl::PipelineOptions::default();
    let (msl, _) = naga::back::msl::write_string(module, &info, &options, &pipeline)
        .unwrap_or_else(|e| panic!("{label}: MSL translation failed: {e}"));
    // A varying declared with a reserved name looks like
    // `metal::float2 half [[user(loc2), flat]];`.
    for line in msl.lines().filter(|l| l.contains("[[user(")) {
        let decl = line.split("[[").next().unwrap_or(line);
        if let Some(name) = decl.split_whitespace().last() {
            assert!(
                !reserved(name),
                "{label}: MSL declares reserved word `{name}`: {line}"
            );
        }
    }
}

#[test]
fn library_modules_use_no_msl_reserved_words() {
    let lib = Library::new(&library()).unwrap();
    let mut bad = Vec::new();
    for m in lib.modules() {
        let text = lib.closure_wgsl(&m.import_path);
        let module = parse_wgsl(&m.import_path, &text).unwrap_or_else(|e| panic!("{e}"));
        for o in offenders(&module) {
            bad.push(format!("{}: {o}", m.file));
        }
        to_msl(&m.import_path, &module);
    }
    assert!(
        bad.is_empty(),
        "MSL reserved words in the library:\n{}",
        bad.join("\n")
    );
}

#[test]
fn authored_shaders_use_no_msl_reserved_words() {
    let mut lib = Library::new(&library()).unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shaders");
    let mut seen = 0;
    let mut bad = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "wgsl") {
            continue;
        }
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path).unwrap();
        let module = lib.compose(&file, &text).unwrap_or_else(|e| panic!("{e}"));
        for o in offenders(&module) {
            bad.push(format!("{file}: {o}"));
        }
        to_msl(&file, &module);
        seen += 1;
    }
    assert!(seen >= 4, "expected the guide shaders, found {seen}");
    assert!(
        bad.is_empty(),
        "MSL reserved words in shaders:\n{}",
        bad.join("\n")
    );
}

/// Seeded: the checker must catch the identifier that broke macOS.
#[test]
fn the_check_catches_half() {
    let module = parse_wgsl(
        "seed",
        "struct V { @location(1) half: vec2<f32> }\n\
         @fragment fn fs(v: V) -> @location(0) vec4<f32> { return vec4<f32>(v.half, 0.0, 1.0); }",
    )
    .unwrap();
    assert_eq!(offenders(&module), vec!["struct member `half`".to_string()]);
    assert!(reserved("float2") && reserved("half4") && reserved("float3x3"));
    assert!(!reserved("half_size"));
}
