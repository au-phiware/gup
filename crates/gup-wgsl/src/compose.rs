// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Build-time flattening with naga_oil (feature `compose`).
//!
//! [`Library::new`] loads WGSL library modules into a naga_oil composer and
//! flattens each one: a probe importing all of the module's items is
//! composed, its items are renamed from naga_oil's decorated names to
//! [`flat_name`]s, and naga's WGSL writer prints the result. The module's
//! own declarations are kept; its imports' declarations come from their own
//! flattened text.
//!
//! Every step is checked here, at build time, so the run-time
//! concatenation can be trusted on targets that have no naga:
//!
//! - naga_oil composes and validates each module (errors name the source
//!   file and line);
//! - each module's flattened text, with its imports', parses and validates
//!   as standalone WGSL;
//! - every struct lays out identically in that text and in naga_oil's
//!   composed module (naga's WGSL writer drops `@align`/`@size`, so a
//!   struct that relies on them fails here, not in a browser);
//! - uniform `Params` structs span a multiple of 16 bytes (RFC-001 "S3
//!   findings").
//!
//! Identifiers that end in a digit are rejected by naga_oil itself when it
//! builds the module.

use crate::{flat_name, lex};
use naga_oil::compose::{
    ComposableModuleDescriptor, Composer, ComposerError, ImportDefinition, NagaModuleDescriptor,
    ShaderLanguage, ShaderType, get_preprocessor_data,
};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// A WGSL source file.
#[derive(Clone, Debug)]
pub struct Source {
    /// The path shown in error reports, e.g. `src/shaders/view.wgsl`.
    pub file: String,
    /// The WGSL text, with naga_oil directives.
    pub text: String,
}

/// A composition failure: what failed and naga_oil's or naga's report,
/// which points at the source line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// The source file or label being composed.
    pub module: String,
    /// The report (multi-line, with source excerpts where available).
    pub report: String,
}

impl Error {
    fn new(module: impl Into<String>, report: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            report: report.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "shader composition failed for {}:\n{}",
            self.module, self.report
        )
    }
}

impl std::error::Error for Error {}

/// A library module flattened to plain WGSL.
#[derive(Clone, Debug)]
pub struct FlatModule {
    /// The module's `#define_import_path`.
    pub import_path: String,
    /// The source file it came from.
    pub file: String,
    /// Import paths it imports directly.
    pub imports: Vec<String>,
    /// Its items' (unflattened) names, in source order.
    pub items: Vec<String>,
    /// Its own declarations, named by [`flat_name`], as naga's WGSL writer
    /// prints them.
    pub wgsl: String,
}

/// Byte layout of a struct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructLayout {
    /// Size in bytes (the struct's span).
    pub span: u32,
    /// `(member name, byte offset)` in declaration order.
    pub members: Vec<(String, u32)>,
}

/// Every named struct in `module` and its layout, in arena order.
pub fn struct_layouts(module: &naga::Module) -> Vec<(String, StructLayout)> {
    module
        .types
        .iter()
        .filter_map(|(_, ty)| match (&ty.name, &ty.inner) {
            (Some(name), naga::TypeInner::Struct { members, span }) => Some((
                name.clone(),
                StructLayout {
                    span: *span,
                    members: members
                        .iter()
                        .map(|m| (m.name.clone().unwrap_or_default(), m.offset))
                        .collect(),
                },
            )),
            _ => None,
        })
        .collect()
}

/// Validate `module` and print it with naga's WGSL writer.
pub fn write_wgsl(label: &str, module: &naga::Module) -> Result<String, Error> {
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(module)
    .map_err(|e| Error::new(label, format!("{e:?}")))?;
    naga::back::wgsl::write_string(module, &info, naga::back::wgsl::WriterFlags::empty())
        .map_err(|e| Error::new(label, e.to_string()))
}

/// Parse and validate standalone WGSL with naga, reporting errors against
/// `source`.
pub fn parse_wgsl(label: &str, source: &str) -> Result<naga::Module, Error> {
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|e| Error::new(label, e.emit_to_string_with_path(source, label)))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .map_err(|e| Error::new(label, e.emit_to_string_with_path(source, label)))?;
    Ok(module)
}

/// The naga_oil composer with the library loaded, and each module's
/// flattened text.
pub struct Library {
    composer: Composer,
    modules: Vec<FlatModule>,
    /// naga_oil's decorated item names → flat names.
    rename: HashMap<String, String>,
}

impl fmt::Debug for Library {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Library")
            .field("modules", &self.modules)
            .finish_non_exhaustive()
    }
}

/// Only `#define_import_path` and `#import` are allowed in library
/// modules: with no shader defs, flattening them once is exact.
fn check_directives(src: &Source) -> Result<(), Error> {
    for (n, line) in src.text.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with('#') && !t.starts_with("#define_import_path") && !t.starts_with("#import")
        {
            return Err(Error::new(
                format!("{}:{}", src.file, n + 1),
                format!(
                    "`{}`: library modules may use only `#define_import_path` and `#import` \
                     (they are flattened once, at build time, without shader defs)",
                    t.trim_end()
                ),
            ));
        }
    }
    Ok(())
}

impl Library {
    /// Load and flatten `sources`, each a module with a
    /// `#define_import_path`. Imports must be among `sources`; their order
    /// does not matter.
    pub fn new(sources: &[Source]) -> Result<Self, Error> {
        let mut parsed: Vec<Parsed<'_>> = Vec::new();
        for src in sources {
            check_directives(src)?;
            let (name, imports, _) = get_preprocessor_data(&src.text);
            let path = name.ok_or_else(|| {
                Error::new(
                    &src.file,
                    "a library module needs a `#define_import_path` line",
                )
            })?;
            if let Some(other) = parsed.iter().find(|p| p.path == path) {
                return Err(Error::new(
                    &src.file,
                    format!("`{path}` is also defined by {}", other.src.file),
                ));
            }
            parsed.push(Parsed {
                src,
                path,
                imports: imports.into_iter().map(|i| i.import).collect(),
                items: lex::declarations(&src.text)
                    .iter()
                    .filter_map(|d| d.name.map(str::to_string))
                    .collect(),
            });
        }

        // Dependencies first.
        fn visit(
            i: usize,
            parsed: &[Parsed<'_>],
            order: &mut Vec<usize>,
            stack: &mut Vec<usize>,
        ) -> Result<(), Error> {
            if order.contains(&i) {
                return Ok(());
            }
            let p = &parsed[i];
            if stack.contains(&i) {
                return Err(Error::new(
                    &p.src.file,
                    format!("`{}` imports itself", p.path),
                ));
            }
            stack.push(i);
            for import in &p.imports {
                let j = parsed
                    .iter()
                    .position(|q| q.path == *import)
                    .ok_or_else(|| {
                        Error::new(
                            &p.src.file,
                            format!(
                                "`{}` imports `{import}`, which is not a library module",
                                p.path
                            ),
                        )
                    })?;
                visit(j, parsed, order, stack)?;
            }
            stack.pop();
            order.push(i);
            Ok(())
        }
        let mut order = Vec::new();
        for i in 0..parsed.len() {
            visit(i, &parsed, &mut order, &mut Vec::new())?;
        }

        let mut composer = Composer::default();
        let mut rename = HashMap::new();
        let mut owner: HashMap<String, String> = HashMap::new();
        for &i in &order {
            let Parsed {
                src, path, items, ..
            } = &parsed[i];
            let added = composer
                .add_composable_module(ComposableModuleDescriptor {
                    source: &src.text,
                    file_path: &src.file,
                    language: ShaderLanguage::Wgsl,
                    ..Default::default()
                })
                .map(|_| ());
            added.map_err(|e| Error::new(&src.file, e.emit_to_string(&composer)))?;
            for item in items {
                let flat = flat_name(path, item);
                if let Some(other) = owner.insert(flat.clone(), format!("{path}::{item}")) {
                    return Err(Error::new(
                        &src.file,
                        format!("`{path}::{item}` and `{other}` both flatten to `{flat}`"),
                    ));
                }
                rename.insert(Composer::decorated_name(Some(path), item), flat);
            }
        }

        let mut lib = Self {
            composer,
            modules: Vec::new(),
            rename,
        };
        for &i in &order {
            let module = lib.flatten_module(&parsed[i])?;
            lib.modules.push(module);
        }
        Ok(lib)
    }

    /// The flattened modules, dependencies first.
    pub fn modules(&self) -> &[FlatModule] {
        &self.modules
    }

    /// The flattened module at `import_path`.
    pub fn module(&self, import_path: &str) -> Option<&FlatModule> {
        self.modules.iter().find(|m| m.import_path == import_path)
    }

    /// The flattened text of `import_path` and of its imports, transitively,
    /// dependencies first: what [`link`](crate::link) appends for it.
    pub fn closure_wgsl(&self, import_path: &str) -> String {
        fn visit<'a>(lib: &'a Library, path: &str, order: &mut Vec<&'a FlatModule>) {
            let Some(m) = lib.module(path) else { return };
            if order.iter().any(|o| o.import_path == m.import_path) {
                return;
            }
            for dep in &m.imports {
                visit(lib, dep, order);
            }
            order.push(m);
        }
        let mut order = Vec::new();
        visit(self, import_path, &mut order);
        order
            .iter()
            .map(|m| m.wgsl.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Compose a top-level shader with naga_oil, with library items renamed
    /// to their flat names.
    pub fn compose(&mut self, label: &str, source: &str) -> Result<naga::Module, Error> {
        self.compose_with(label, source, &[])
    }

    fn compose_with(
        &mut self,
        label: &str,
        source: &str,
        additional_imports: &[ImportDefinition],
    ) -> Result<naga::Module, Error> {
        let composed = self
            .composer
            .make_naga_module(NagaModuleDescriptor {
                source,
                file_path: label,
                shader_type: ShaderType::Wgsl,
                additional_imports,
                ..Default::default()
            })
            .map_err(|e: ComposerError| Error::new(label, e.emit_to_string(&self.composer)))?;
        self.rename(label, composed)
    }

    /// Flatten an authored top-level shader completely: compose it, print it
    /// with naga's WGSL writer and check that the printed text has the same
    /// entry points and struct layouts.
    pub fn flatten_shader(&mut self, label: &str, source: &str) -> Result<String, Error> {
        let composed = self.compose(label, source)?;
        let text = write_wgsl(label, &composed)?;
        let reparsed = parse_wgsl(label, &text)?;
        let entry_points = |m: &naga::Module| {
            m.entry_points
                .iter()
                .map(|e| (e.name.clone(), e.stage))
                .collect::<Vec<_>>()
        };
        if entry_points(&composed) != entry_points(&reparsed) {
            return Err(Error::new(
                label,
                format!(
                    "the flattened text has entry points {:?}, the composed module {:?}",
                    entry_points(&reparsed),
                    entry_points(&composed)
                ),
            ));
        }
        check_layouts(label, &composed, &reparsed)?;
        Ok(text)
    }

    fn flatten_module(&mut self, parsed: &Parsed<'_>) -> Result<FlatModule, Error> {
        let Parsed {
            src,
            path,
            imports,
            items,
        } = parsed;
        let mut flat = FlatModule {
            import_path: path.clone(),
            file: src.file.clone(),
            imports: imports.clone(),
            items: items.clone(),
            wgsl: String::new(),
        };
        if items.is_empty() {
            return Ok(flat);
        }
        // naga_oil imports only the items a shader names; an additional
        // import names them all without a source that uses them.
        let all = [ImportDefinition {
            import: path.clone(),
            items: items.clone(),
        }];
        let composed = self.compose_with(&src.file, "", &all)?;
        let text = write_wgsl(&src.file, &composed)?;

        let own: Vec<String> = items.iter().map(|i| flat_name(path, i)).collect();
        let library: HashSet<&String> = self.rename.values().collect();
        let mut kept = Vec::new();
        for decl in lex::declarations(&text) {
            match decl.name {
                Some(name) if own.iter().any(|o| o == name) => kept.push((name, decl.text)),
                Some(name) if library.contains(&name.to_string()) => {}
                other => {
                    return Err(Error::new(
                        &src.file,
                        format!(
                            "naga's WGSL writer printed an unexpected declaration ({other:?}):\n{}",
                            decl.text
                        ),
                    ));
                }
            }
        }
        for (item, name) in items.iter().zip(&own) {
            if !kept.iter().any(|(n, _)| n == name) {
                return Err(Error::new(
                    &src.file,
                    format!(
                        "`{path}::{item}` is missing from the flattened text (expected `{name}`; \
                         naga renames identifiers that end in a digit or contain `__`)"
                    ),
                ));
            }
        }
        flat.wgsl = kept
            .iter()
            .map(|(_, text)| format!("{text}\n"))
            .collect::<Vec<_>>()
            .join("\n");

        // Uniform `Params` structs span a multiple of 16 bytes, so they pack
        // into generated uniform structs without layout attributes.
        let params = flat_name(path, "Params");
        for (name, layout) in struct_layouts(&composed) {
            if name == params && layout.span % 16 != 0 {
                return Err(Error::new(
                    &src.file,
                    format!(
                        "struct `Params` of `{path}` spans {} bytes; uniform `Params` structs \
                         must span a multiple of 16 bytes (pad it in WGSL and in its encase twin)",
                        layout.span
                    ),
                ));
            }
        }

        // The round trip: this module's text plus its imports' parses and
        // validates alone, and lays every struct out as composed.
        self.modules.push(flat.clone());
        let closure = self.closure_wgsl(path);
        self.modules.pop();
        let reparsed = parse_wgsl(&format!("{} (flattened)", src.file), &closure)?;
        check_layouts(&src.file, &composed, &reparsed)?;
        Ok(flat)
    }

    /// Rename decorated library items to flat names.
    fn rename(&self, label: &str, mut module: naga::Module) -> Result<naga::Module, Error> {
        let renamed = |name: &Option<String>| name.as_ref().and_then(|n| self.rename.get(n));
        let types: Vec<_> = module
            .types
            .iter()
            .filter_map(|(h, ty)| {
                renamed(&ty.name).map(|n| {
                    (
                        h,
                        naga::Type {
                            name: Some(n.clone()),
                            inner: ty.inner.clone(),
                        },
                    )
                })
            })
            .collect();
        for (h, ty) in types {
            module.types.replace(h, ty);
        }
        for (_, c) in module.constants.iter_mut() {
            if let Some(n) = renamed(&c.name) {
                c.name = Some(n.clone());
            }
        }
        for (_, o) in module.overrides.iter_mut() {
            if let Some(n) = renamed(&o.name) {
                o.name = Some(n.clone());
            }
        }
        for (_, g) in module.global_variables.iter_mut() {
            if let Some(n) = renamed(&g.name) {
                g.name = Some(n.clone());
            }
        }
        for (_, f) in module.functions.iter_mut() {
            if let Some(n) = renamed(&f.name) {
                f.name = Some(n.clone());
            }
        }
        let leftover = module
            .types
            .iter()
            .filter_map(|(_, t)| t.name.as_ref())
            .chain(module.constants.iter().filter_map(|(_, c)| c.name.as_ref()))
            .chain(module.overrides.iter().filter_map(|(_, o)| o.name.as_ref()))
            .chain(
                module
                    .global_variables
                    .iter()
                    .filter_map(|(_, g)| g.name.as_ref()),
            )
            .chain(module.functions.iter().filter_map(|(_, f)| f.name.as_ref()))
            .find(|n| n.contains("_naga_oil_"));
        if let Some(name) = leftover {
            return Err(Error::new(
                label,
                format!("`{name}` is a library item the module scan did not find"),
            ));
        }
        Ok(module)
    }
}

/// A library source with its import path, direct imports and items.
struct Parsed<'a> {
    src: &'a Source,
    path: String,
    imports: Vec<String>,
    items: Vec<String>,
}

/// Every struct of `composed` must lay out the same in `reparsed`.
fn check_layouts(
    label: &str,
    composed: &naga::Module,
    reparsed: &naga::Module,
) -> Result<(), Error> {
    let flattened = struct_layouts(reparsed);
    for (name, layout) in struct_layouts(composed) {
        let Some((_, found)) = flattened.iter().find(|(n, _)| *n == name) else {
            return Err(Error::new(
                label,
                format!(
                    "struct `{name}` is missing from the flattened WGSL (naga renames \
                     identifiers that end in a digit, contain `__` or are WGSL keywords)"
                ),
            ));
        };
        let offsets =
            |l: &StructLayout| (l.span, l.members.iter().map(|m| m.1).collect::<Vec<_>>());
        if offsets(found) != offsets(&layout) {
            return Err(Error::new(
                label,
                format!(
                    "struct `{name}` lays out as {layout:?} when composed but as {found:?} in the \
                     flattened WGSL (naga's WGSL writer drops `@align` and `@size`; pad the \
                     struct instead)"
                ),
            ));
        }
        for ((before, _), (after, _)) in layout.members.iter().zip(&found.members) {
            if before != after {
                return Err(Error::new(
                    label,
                    format!(
                        "naga's WGSL writer renamed member `{name}.{before}` to `{after}`: it \
                         renames identifiers that end in a digit or are WGSL keywords, so \
                         rename it in the source"
                    ),
                ));
            }
        }
    }
    Ok(())
}
