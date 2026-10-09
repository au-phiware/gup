// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use gup_wgsl::WgslModule;
use gup_wgsl::compose::{Library, Source, parse_wgsl, struct_layouts};

#[test]
fn spike() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../gup-core/src/shaders");
    let mut sources = vec![];
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let text = std::fs::read_to_string(&p).unwrap();
        let file = format!("src/shaders/{}", p.file_name().unwrap().to_str().unwrap());
        if text.contains("#define_import_path") {
            sources.push(Source { file, text });
        }
    }
    let mut lib = Library::new(&sources).unwrap_or_else(|e| panic!("{e}"));
    // Leak run-time modules, dependencies first.
    let mut statics: Vec<&'static WgslModule> = vec![];
    for m in lib.modules() {
        let imports: Vec<&'static WgslModule> = m
            .imports
            .iter()
            .map(|p| *statics.iter().find(|s| s.import_path == p).unwrap())
            .collect();
        statics.push(Box::leak(Box::new(WgslModule {
            import_path: Box::leak(m.import_path.clone().into_boxed_str()),
            wgsl: Box::leak(m.wgsl.clone().into_boxed_str()),
            imports: Box::leak(imports.into_boxed_slice()),
        })));
    }
    let glue = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../gup-core/tests/fixtures/scatter_glue.wgsl"
    ))
    .unwrap();
    let linked = gup_wgsl::link("glue", &glue, &statics).unwrap();
    eprintln!("==== linked\n{linked}");
    let linked_module = parse_wgsl("linked", &linked).unwrap_or_else(|e| panic!("{e}"));
    let oracle = lib.compose("glue", &glue).unwrap_or_else(|e| panic!("{e}"));
    let eps = |m: &naga::Module| {
        m.entry_points
            .iter()
            .map(|e| (e.name.clone(), e.stage))
            .collect::<Vec<_>>()
    };
    assert_eq!(eps(&linked_module), eps(&oracle));
    let mut a = struct_layouts(&linked_module);
    let mut b = struct_layouts(&oracle);
    a.sort_by(|x, y| x.0.cmp(&y.0));
    b.sort_by(|x, y| x.0.cmp(&y.0));
    for (x, y) in a.iter().zip(&b) {
        eprintln!("{} {:?}", x.0, x.1);
        assert_eq!(x, y);
    }
    assert_eq!(a.len(), b.len());
}

#[test]
fn spike_errors() {
    let view = Source {
        file: "src/shaders/view.wgsl".into(),
        text: std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../gup-core/src/shaders/view.wgsl"
        ))
        .unwrap(),
    };
    let broken = Source {
        file: "src/shaders/broken.wgsl".into(),
        text: "#define_import_path gup::broken\n#import gup::view::{View, px_to_clip}\n\nfn f(view: View) -> vec4<f32> {\n    return px_to_clip(vec2<f32>(0.0));\n}\n".into(),
    };
    eprintln!(
        "---- arity\n{}",
        Library::new(&[view.clone(), broken]).unwrap_err()
    );
    let digit = Source {
        file: "src/shaders/digit.wgsl".into(),
        text: "#define_import_path gup::digit\n\nstruct Params {\n    r0: f32,\n    k: f32,\n    a: f32,\n    b: f32,\n}\n\nfn f(p: Params) -> f32 {\n    return p.r0;\n}\n".into(),
    };
    eprintln!("---- digit\n{}", Library::new(&[digit]).unwrap_err());
    let pad = Source {
        file: "src/shaders/pad.wgsl".into(),
        text: "#define_import_path gup::scale::pad\n\nstruct Params {\n    k: f32,\n}\n\nfn map(v: f32, p: Params) -> f32 {\n    return v * p.k;\n}\n".into(),
    };
    eprintln!("---- pad\n{}", Library::new(&[pad]).unwrap_err());
}
