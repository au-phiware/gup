// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The calling convention gup-core's glue uses, checked against the
//! composed user module (RFC-001 "GUP-406 findings", S5 item 4): the entry
//! function's arguments and result, its `Params` struct and the stage it
//! can run in.

use gup_wgsl::flat_name;
use naga::{
    Handle, ImageClass, ImageDimension, Module, Scalar, ScalarKind, Type, TypeInner, VectorSize,
    valid::{ModuleInfo, ShaderStages},
};

/// How the entry function reads its column (gup-core's `ColumnFormat`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// `v: f32`.
    F32,
    /// `v: f32, base: f32`.
    F32Relative,
    /// `v: vec2<f32>, base: vec2<f32>` (hi/lo).
    F32x2Relative,
    /// `code: u32` (dictionary codes).
    U32,
}

impl Input {
    pub fn is_relative(self) -> bool {
        matches!(self, Self::F32Relative | Self::F32x2Relative)
    }
}

/// What the entry function returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// `f32`: a number (or pixels).
    F32,
    /// `vec4<f32>`: a colour.
    Color,
}

impl Output {
    pub fn wgsl(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::Color => "vec4<f32>",
        }
    }
}

/// A `Params` member's Rust twin type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Member {
    F32,
    U32,
    I32,
    Color,
}

/// The entry function, checked.
#[derive(Debug)]
pub struct Convention {
    pub input: Input,
    pub output: Output,
    pub lut: bool,
    /// `Params`' span in bytes.
    pub params_span: u32,
    /// `Params`' members and their offsets, in order, including any
    /// padding `gup_wgsl::compose::Library::add_module` appended.
    pub params: Vec<(String, Member, u32)>,
}

/// The shapes the glue calls, for error messages.
fn expected(entry: &str) -> String {
    format!(
        "the glue calls one of these shapes:\n    \
         fn {entry}(v: f32, p: Params) -> f32                          a number\n    \
         fn {entry}(v: f32, base: f32, p: Params) -> f32               a number stored relative to its chunk (`base` comes from `chunk_base`)\n    \
         fn {entry}(v: vec2<f32>, base: vec2<f32>, p: Params) -> f32   a hi/lo pair relative to its chunk, for deep zoom\n    \
         fn {entry}(code: u32, p: Params) -> f32                       a dictionary code (`encode_key`)\n\
         each returning `f32` (a number) or `vec4<f32>` (a colour), optionally with \
         `lut: texture_2d<f32>, smp: sampler` after `p` for a palette LUT"
    )
}

/// `ty` as WGSL, with the module's own items unqualified.
pub(crate) fn type_name(module: &Module, ty: Handle<Type>, prefix: &str) -> String {
    let scalar = |s: Scalar| match (s.kind, s.width) {
        (ScalarKind::Float, 4) => "f32".to_string(),
        (ScalarKind::Float, 2) => "f16".to_string(),
        (ScalarKind::Uint, 4) => "u32".to_string(),
        (ScalarKind::Sint, 4) => "i32".to_string(),
        (ScalarKind::Bool, _) => "bool".to_string(),
        (k, w) => format!("{k:?}{}", w * 8),
    };
    let t = &module.types[ty];
    match &t.inner {
        TypeInner::Scalar(s) => scalar(*s),
        TypeInner::Vector { size, scalar: s } => format!("vec{}<{}>", *size as u8, scalar(*s)),
        TypeInner::Matrix {
            columns,
            rows,
            scalar: s,
        } => format!("mat{}x{}<{}>", *columns as u8, *rows as u8, scalar(*s)),
        TypeInner::Struct { .. } => t
            .name
            .as_deref()
            .map(|n| n.strip_prefix(prefix).unwrap_or(n).to_string())
            .unwrap_or_else(|| "struct".to_string()),
        TypeInner::Image { dim, class, .. } => {
            let dim = match dim {
                ImageDimension::D1 => "1d",
                ImageDimension::D2 => "2d",
                ImageDimension::D3 => "3d",
                ImageDimension::Cube => "cube",
            };
            match class {
                ImageClass::Sampled { kind, .. } => {
                    format!(
                        "texture_{dim}<{}>",
                        scalar(Scalar {
                            kind: *kind,
                            width: 4
                        })
                    )
                }
                ImageClass::Depth { .. } => format!("texture_depth_{dim}"),
                _ => format!("texture_storage_{dim}<…>"),
            }
        }
        TypeInner::Sampler { comparison: false } => "sampler".to_string(),
        TypeInner::Sampler { comparison: true } => "sampler_comparison".to_string(),
        TypeInner::Array { base, .. } => format!("array<{}>", type_name(module, *base, prefix)),
        TypeInner::Pointer { base, .. } => format!("ptr<{}>", type_name(module, *base, prefix)),
        other => format!("{other:?}"),
    }
}

/// Check function `entry` of the module at `path` in `module` (the
/// module's flattened text with its imports, validated as `info`).
pub fn check(
    module: &Module,
    info: &ModuleInfo,
    path: &str,
    entry: &str,
) -> Result<Convention, String> {
    let prefix = flat_name(path, "");
    let flat = flat_name(path, entry);
    let (handle, f) = module
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some(flat.as_str()))
        .ok_or_else(|| format!("the WGSL declares no function `{entry}`"))?;

    let ty = |h: Handle<Type>| &module.types[h].inner;
    let args: Vec<Handle<Type>> = f.arguments.iter().map(|a| a.ty).collect();
    let found = format!(
        "fn {entry}({}){}",
        f.arguments
            .iter()
            .map(|a| format!(
                "{}: {}",
                a.name.as_deref().unwrap_or("_"),
                type_name(module, a.ty, &prefix)
            ))
            .collect::<Vec<_>>()
            .join(", "),
        f.result
            .as_ref()
            .map(|r| format!(" -> {}", type_name(module, r.ty, &prefix)))
            .unwrap_or_default()
    );
    let fail = |why: &str| {
        Err(format!(
            "`{entry}` cannot be called by gup's glue: {why}.\nfound:\n    {found}\n{}",
            expected(entry)
        ))
    };

    let is = |h: Handle<Type>, s: Scalar| *ty(h) == TypeInner::Scalar(s);
    let is_vec2 = |h: Handle<Type>| {
        *ty(h)
            == TypeInner::Vector {
                size: VectorSize::Bi,
                scalar: Scalar::F32,
            }
    };
    let is_struct = |h: Handle<Type>| matches!(ty(h), TypeInner::Struct { .. });

    let Some(&first) = args.first() else {
        return fail("it takes no arguments; the first is the value it maps");
    };
    let (input, mut i) = if is(first, Scalar::F32) {
        if args.get(1).is_some_and(|&b| is(b, Scalar::F32)) {
            (Input::F32Relative, 2)
        } else {
            (Input::F32, 1)
        }
    } else if is_vec2(first) {
        if !args.get(1).is_some_and(|&b| is_vec2(b)) {
            return fail(
                "a hi/lo `vec2<f32>` value is stored relative to its chunk, so a \
                 `base: vec2<f32>` argument must follow it",
            );
        }
        (Input::F32x2Relative, 2)
    } else if is(first, Scalar::U32) {
        (Input::U32, 1)
    } else if is_struct(first) {
        return fail("the value it maps comes first, then its `Params`");
    } else {
        return fail(&format!(
            "its first argument is a `{}`; the value it maps is an `f32`, a hi/lo \
             `vec2<f32>` or a `u32` dictionary code",
            type_name(module, first, &prefix)
        ));
    };

    let params_name = flat_name(path, "Params");
    let Some(&params) = args.get(i) else {
        return fail(&format!(
            "it takes no `Params` struct; every function takes one, after its value{}, \
             for its uniform values",
            if input.is_relative() { " and base" } else { "" }
        ));
    };
    match (&module.types[params].name, ty(params)) {
        (Some(n), TypeInner::Struct { .. }) if *n == params_name => {}
        (_, TypeInner::Struct { .. }) => {
            return fail(&format!(
                "its uniform struct is `{}`; name it `Params` (the glue declares its \
                 uniform field as `<module>::Params`)",
                type_name(module, params, &prefix)
            ));
        }
        _ => {
            return fail(&format!(
                "argument {} is a `{}` where its `Params` struct belongs",
                i + 1,
                type_name(module, params, &prefix)
            ));
        }
    }
    i += 1;

    let lut = match args.len() - i {
        0 => false,
        2 if matches!(
            ty(args[i]),
            TypeInner::Image {
                dim: ImageDimension::D2,
                arrayed: false,
                class: ImageClass::Sampled {
                    kind: ScalarKind::Float,
                    multi: false
                },
            }
        ) && *ty(args[i + 1]) == TypeInner::Sampler { comparison: false } =>
        {
            true
        }
        _ => {
            return fail(
                "after `Params` it may take only a palette LUT, as \
                 `lut: texture_2d<f32>, smp: sampler`",
            );
        }
    };

    let output = match f.result.as_ref().map(|r| ty(r.ty)) {
        Some(TypeInner::Scalar(Scalar::F32)) => Output::F32,
        Some(TypeInner::Vector {
            size: VectorSize::Quad,
            scalar: Scalar::F32,
        }) => Output::Color,
        _ => {
            return fail("it must return an `f32` (a number) or a `vec4<f32>` (a colour)");
        }
    };

    if !info[handle].available_stages.contains(ShaderStages::VERTEX) {
        return Err(format!(
            "`{entry}` runs in the vertex stage (the glue calls it once per mark), but it \
             uses something only fragment shaders have: a derivative (`dpdx`, `fwidth`, …) \
             or implicit-level sampling (`textureSample`). Read a LUT with \
             `textureSampleLevel(lut, smp, uv, 0.0)` or `textureLoad` instead."
        ));
    }

    // Resources come only through the glue's bindings.
    for (_, g) in module.global_variables.iter() {
        let Some(name) = g.name.as_deref().and_then(|n| n.strip_prefix(&prefix)) else {
            continue;
        };
        if g.binding.is_some() || g.space != naga::AddressSpace::Private {
            return Err(format!(
                "the WGSL declares the global `{name}` in the {:?} address space: a \
                 function's inputs are its value, its `Params` and an optional palette \
                 LUT, and gup binds nothing else (`var<private>` globals are fine)",
                g.space
            ));
        }
    }

    // Overrides would need values at pipeline creation, which gup never
    // gives.
    if let Some(name) = module
        .overrides
        .iter()
        .find_map(|(_, o)| o.name.as_deref().and_then(|n| n.strip_prefix(&prefix)))
    {
        return Err(format!(
            "the WGSL declares `override {name}`, which gup never sets at pipeline \
             creation; make it a `Params` member or a `const`"
        ));
    }

    let TypeInner::Struct { members, span } = ty(params) else {
        unreachable!("checked above")
    };
    let mut fields = Vec::new();
    for m in members {
        let name = m.name.clone().unwrap_or_default();
        let member = match ty(m.ty) {
            TypeInner::Scalar(Scalar::F32) => Member::F32,
            TypeInner::Scalar(Scalar::U32) => Member::U32,
            TypeInner::Scalar(Scalar::I32) => Member::I32,
            TypeInner::Vector {
                size: VectorSize::Quad,
                scalar: Scalar::F32,
            } => Member::Color,
            _ => {
                return Err(format!(
                    "`Params` member `{name}: {}` has no Rust twin: members may be `f32`, \
                     `u32`, `i32` or `vec4<f32>` (a `Color`); split it into those",
                    type_name(module, m.ty, &prefix)
                ));
            }
        };
        fields.push((name, member, m.offset));
    }

    Ok(Convention {
        input,
        output,
        lut,
        params_span: *span,
        params: fields,
    })
}

/// The functions the module at `path` declares itself.
pub fn own_functions(module: &Module, path: &str, items: &[String]) -> Vec<String> {
    items
        .iter()
        .filter(|item| {
            let flat = flat_name(path, item);
            module
                .functions
                .iter()
                .any(|(_, f)| f.name.as_deref() == Some(flat.as_str()))
        })
        .cloned()
        .collect()
}
