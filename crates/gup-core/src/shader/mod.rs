// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shader composition (RFC-001 §6): WGSL library modules composed with
//! naga_oil, plus the typed glue emitter in [`glue`].
//!
//! Library modules (marks, scales, palettes, the view transform) are plain
//! WGSL files with a `#define_import_path gup::…` header. They use only
//! `#define_import_path`/`#import` (no `#ifdef`), so plain concatenation of
//! namespaced modules remains a fallback if naga_oil ever blocks a wgpu
//! upgrade (RFC-001 §12 risk 1). **Gup never edits authored WGSL as text.**

pub(crate) mod glue;

use crate::error::{Error, Result};
use naga_oil::compose::{
    ComposableModuleDescriptor, Composer, ComposerError, NagaModuleDescriptor, ShaderLanguage,
    ShaderType,
};
use std::time::Duration;
// `std::time::Instant::now` panics on wasm32-unknown-unknown.
use web_time::Instant;

/// A WGSL library module: an import path, its source and the modules it
/// imports (which must be added to the composer first).
#[derive(Debug)]
pub struct WgslModule {
    /// The `#define_import_path` of the module, e.g. `gup::scale::linear`.
    pub import_path: &'static str,
    /// The WGSL source, including the `#define_import_path` line.
    pub source: &'static str,
    /// Modules this module `#import`s.
    pub imports: &'static [&'static WgslModule],
}

/// `gup::view`: the `View` uniform and `px_to_clip`.
pub static VIEW: WgslModule = WgslModule {
    import_path: "gup::view",
    source: include_str!("../shaders/view.wgsl"),
    imports: &[],
};

/// `gup::scale::linear`.
pub static SCALE_LINEAR: WgslModule = WgslModule {
    import_path: "gup::scale::linear",
    source: include_str!("../shaders/scale_linear.wgsl"),
    imports: &[],
};

/// `gup::scale::log`.
pub static SCALE_LOG: WgslModule = WgslModule {
    import_path: "gup::scale::log",
    source: include_str!("../shaders/scale_log.wgsl"),
    imports: &[],
};

/// `gup::color::sequential`.
pub static COLOR_SEQUENTIAL: WgslModule = WgslModule {
    import_path: "gup::color::sequential",
    source: include_str!("../shaders/color_sequential.wgsl"),
    imports: &[],
};

/// `gup::marks::circle`.
pub static MARK_CIRCLE: WgslModule = WgslModule {
    import_path: "gup::marks::circle",
    source: include_str!("../shaders/circle.wgsl"),
    imports: &[&VIEW],
};

/// The library modules preloaded into every context's composer.
pub(crate) static BUILTIN_MODULES: &[&WgslModule] = &[
    &VIEW,
    &SCALE_LINEAR,
    &SCALE_LOG,
    &COLOR_SEQUENTIAL,
    &MARK_CIRCLE,
];

/// Hand-authored top-level shader for guide rules.
pub(crate) const RULE_SHADER: &str = include_str!("../shaders/rule.wgsl");

/// Hand-authored top-level shader for flat rectangles.
pub(crate) const RECT_SHADER: &str = include_str!("../shaders/rect.wgsl");

/// Hand-authored top-level shader for colour-legend gradient bars.
pub(crate) const GRADIENT_SHADER: &str = include_str!("../shaders/gradient.wgsl");

/// A composed top-level shader and how long composition took.
pub(crate) struct Composed {
    pub module: naga::Module,
    pub compose_time: Duration,
}

/// The naga_oil composer with Gup's library modules loaded (one per
/// [`Context`](crate::Context)).
pub(crate) struct ShaderLibrary {
    composer: Composer,
}

impl ShaderLibrary {
    /// A composer with every built-in library module preloaded.
    ///
    /// The built-in modules are compile-time constants covered by this
    /// crate's tests, so a failure here is a bug in Gup, not a user error.
    pub(crate) fn new() -> Self {
        let mut lib = Self {
            composer: Composer::default(),
        };
        for module in BUILTIN_MODULES {
            if let Err(e) = lib.ensure(module) {
                panic!("built-in WGSL module failed to load: {e}");
            }
        }
        lib
    }

    /// Add `module` (after its imports) unless it is already loaded.
    pub(crate) fn ensure(&mut self, module: &WgslModule) -> Result<()> {
        if self.composer.contains_module(module.import_path) {
            return Ok(());
        }
        for dep in module.imports {
            self.ensure(dep)?;
        }
        self.composer
            .add_composable_module(ComposableModuleDescriptor {
                source: module.source,
                file_path: module.import_path,
                language: ShaderLanguage::Wgsl,
                ..Default::default()
            })
            .map(|_| ())
            .map_err(|e| self.error(module.import_path, &e))
    }

    /// Compose a top-level shader whose `#import`s refer to `modules`
    /// (each is loaded first if needed).
    pub(crate) fn compose(
        &mut self,
        label: &str,
        source: &str,
        modules: &[&WgslModule],
    ) -> Result<Composed> {
        for module in modules {
            self.ensure(module)?;
        }
        let start = Instant::now();
        let module = self
            .composer
            .make_naga_module(NagaModuleDescriptor {
                source,
                file_path: label,
                shader_type: ShaderType::Wgsl,
                ..Default::default()
            })
            .map_err(|e| self.error(label, &e))?;
        Ok(Composed {
            module,
            compose_time: start.elapsed(),
        })
    }

    fn error(&self, module: &str, e: &ComposerError) -> Error {
        Error::Compose {
            module: module.to_string(),
            report: e.emit_to_string(&self.composer),
        }
    }
}

/// Byte layout of a struct in a composed module, as naga computed it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StructLayout {
    /// Total size in bytes (the struct's span).
    pub span: u32,
    /// `(member name, byte offset)` in declaration order.
    pub members: Vec<(String, u32)>,
}

impl StructLayout {
    /// The offset of member `name`.
    pub(crate) fn offset(&self, name: &str) -> Option<u32> {
        self.members
            .iter()
            .find(|(n, _)| n == name)
            .map(|&(_, o)| o)
    }
}

/// The layout of struct `name` in `module` (top-level names are not
/// mangled by naga_oil).
pub(crate) fn struct_layout(module: &naga::Module, name: &str) -> Option<StructLayout> {
    module.types.iter().find_map(|(_, ty)| match &ty.inner {
        naga::TypeInner::Struct { members, span } if ty.name.as_deref() == Some(name) => {
            Some(StructLayout {
                span: *span,
                members: members
                    .iter()
                    .map(|m| (m.name.clone().unwrap_or_default(), m.offset))
                    .collect(),
            })
        }
        _ => None,
    })
}

/// Pretty-print a composed module back to WGSL (for diagnostics, fixtures
/// and the RFC record).
#[cfg(test)]
pub(crate) fn to_wgsl(module: &naga::Module) -> Result<String> {
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(module)
    .map_err(|e| Error::Compose {
        module: "composed module".into(),
        report: format!("{e:?}"),
    })?;
    naga::back::wgsl::write_string(module, &info, naga::back::wgsl::WriterFlags::empty()).map_err(
        |e| Error::Compose {
            module: "composed module".into(),
            report: e.to_string(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_modules_load_into_the_composer() {
        let lib = ShaderLibrary::new();
        for m in BUILTIN_MODULES {
            assert!(
                lib.composer.contains_module(m.import_path),
                "{}",
                m.import_path
            );
        }
    }

    #[test]
    fn guide_shaders_compose() {
        let mut lib = ShaderLibrary::new();
        for (name, source, imports) in [
            ("rule", RULE_SHADER, &[&VIEW][..]),
            ("rect", RECT_SHADER, &[&VIEW]),
            ("gradient", GRADIENT_SHADER, &[&VIEW, &COLOR_SEQUENTIAL]),
        ] {
            let composed = lib.compose(name, source, imports).unwrap();
            assert_eq!(composed.module.entry_points.len(), 2, "{name}");
        }
    }

    #[test]
    fn compose_errors_point_at_the_authored_source() {
        let mut lib = ShaderLibrary::new();
        let err = lib
            .compose(
                "broken_glue",
                "#import gup::scale::linear as linear\n\
                 fn f() -> f32 { return linear::map_rel(1.0, 2.0); }\n",
                &[&SCALE_LINEAR],
            )
            .err()
            .expect("wrong arity must fail");
        let text = err.to_string();
        assert!(text.contains("broken_glue"), "{text}");
    }
}
