// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Selection<T, M>`: rows of `T` drawn as mark `M`, with typed channels
//! (RFC-001 §4). S0a scope: data and `attr`; events, transitions and
//! append are later S-stories.

use crate::channel::{Channel, Mark, Role, Visual};
use crate::column::ColumnStore;
use crate::context::Context;
use crate::encoding::{Encoding, IntoEncoding, Resource};
use crate::error::{Error, Result};
use crate::render::{LayerGpu, LayerUniforms};
use crate::scene::MarkBatch;
use crate::shader::glue::{self, ChannelSource, Glue, GlueChannel, GlueSpec};
use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;

/// The palette LUTs a layer's GPU state was built with.
type Luts = Vec<Vec<[u8; 4]>>;

/// Rows of `T` drawn as mark `M`.
///
/// ```
/// use gup_core::prelude::*;
///
/// struct Reading { t: f64, value: f64, temp: f64 }
/// let mut sel = Selection::<Reading, Circle>::new(vec![
///     Reading { t: 0.0, value: 1.0, temp: 10.0 },
/// ]);
/// sel.attr(Circle::X, Linear::new().encode(|r: &Reading| r.t))
///     .attr(Circle::FILL, Sequential::viridis().encode(|r: &Reading| r.temp))
///     .attr(Circle::RADIUS, Px(2.5));
/// ```
pub struct Selection<T, M: Mark> {
    rows: Vec<T>,
    encodings: Vec<Option<Encoding<T>>>,
    /// Evaluated columns, one per column-encoded channel in channel order.
    columns: Option<ColumnStore>,
    /// GPU state from the last `prepare` and the LUTs it holds. Later
    /// prepares with the same program, context and columns only write
    /// uniforms into it.
    gpu: Option<(Arc<LayerGpu>, Luts)>,
    _mark: PhantomData<fn() -> M>,
}

impl<T, M: Mark> std::fmt::Debug for Selection<T, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Selection")
            .field("mark", &M::NAME)
            .field("rows", &self.rows.len())
            .field("encodings", &self.encodings)
            .finish_non_exhaustive()
    }
}

impl<T: Send + Sync + 'static, M: Mark> Selection<T, M> {
    /// A selection over `rows`. Channels not set with [`attr`](Self::attr)
    /// use the mark's defaults.
    pub fn new(rows: impl Into<Vec<T>>) -> Self {
        Self {
            rows: rows.into(),
            encodings: M::CHANNELS.iter().map(|_| None).collect(),
            columns: None,
            gpu: None,
            _mark: PhantomData,
        }
    }

    /// Number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Drive `channel` with a constant or an encoding. The encoding's
    /// output type must be the channel's visual type, or this does not
    /// compile.
    pub fn attr<V: Visual, E, K>(&mut self, channel: Channel<M, V>, encoding: E) -> &mut Self
    where
        E: IntoEncoding<T, V, K>,
    {
        self.encodings[usize::from(channel.slot())] = Some(encoding.into_encoding());
        // S0a re-evaluates every column; per-column invalidation is S4.
        self.columns = None;
        self.gpu = None;
        self
    }

    fn column_encodings(&self) -> impl Iterator<Item = (usize, &dyn crate::DynColumnEncoding<T>)> {
        self.encodings
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match e {
                Some(Encoding::Column(c)) => Some((i, c.as_ref())),
                _ => None,
            })
    }

    /// Run the accessors (once) into a column store.
    fn evaluate(&mut self) -> Result<&mut ColumnStore> {
        if self.columns.is_none() {
            let columns = self
                .column_encodings()
                .map(|(_, c)| (c.func().input_format(), c.evaluate(&self.rows)))
                .collect();
            self.columns = Some(ColumnStore::from_columns(columns)?);
        }
        Ok(self.columns.as_mut().expect("evaluated above"))
    }

    fn glue(&self) -> Glue {
        let channels = M::CHANNELS
            .iter()
            .zip(&self.encodings)
            .map(|(desc, enc)| GlueChannel {
                name: desc.name,
                wgsl_type: desc.wgsl_type,
                source: match enc {
                    Some(Encoding::Column(c)) => ChannelSource::Column(c.func()),
                    _ => ChannelSource::Const,
                },
            })
            .collect();
        glue::emit(&GlueSpec {
            mark_name: M::NAME,
            mark_module: M::MODULE,
            channels,
        })
    }
}

/// A plot layer: the object-safe face of a [`Selection`] (internal; the
/// public `Layer`/`Chart` traits are RFC-001 S7).
pub(crate) trait Layer: wgpu::WasmNotSendSync {
    /// Evaluate accessors and fit every data-driven domain to its column.
    fn fit_domains(&mut self) -> Result<()>;
    /// How far marks may extend past their position (e.g. a constant
    /// radius), in pixels.
    fn overhang(&self) -> f32;
    /// Generate the glue, link it (cached), write uniforms and upload.
    fn prepare(&mut self, cx: &Context) -> Result<MarkBatch>;
    /// The generated glue (for tests and diagnostics).
    fn glue_source(&self) -> Glue;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Send + Sync + 'static, M: Mark> Layer for Selection<T, M> {
    fn fit_domains(&mut self) -> Result<()> {
        let stats: Vec<_> = self
            .evaluate()?
            .columns()
            .iter()
            .map(|c| c.stats())
            .collect();
        let mut k = 0;
        for (i, enc) in self.encodings.iter_mut().enumerate() {
            if let Some(Encoding::Column(c)) = enc {
                if let Some(s) = stats[k] {
                    c.func_mut()
                        .fit_domain(s.extent())
                        .map_err(|e| context(e, M::NAME, M::CHANNELS[i].name))?;
                }
                k += 1;
            }
        }
        Ok(())
    }

    fn overhang(&self) -> f32 {
        M::CHANNELS
            .iter()
            .zip(&self.encodings)
            .filter(|(d, _)| d.role == Some(Role::Size))
            .map(|(d, e)| match e {
                Some(Encoding::Const(v)) => v,
                _ => &d.default,
            })
            .map(|v| match v {
                crate::ConstValue::F32(r) => *r,
                crate::ConstValue::Vec4(_) => 0.0,
            })
            .fold(0.0, f32::max)
    }

    fn prepare(&mut self, cx: &Context) -> Result<MarkBatch> {
        let glue = self.glue();
        let program = cx.pipelines().program(cx, &glue)?;

        // The `Encodings` uniform: each channel's Params or constant at the
        // offset the glue emitter laid out (encase sizes, 16-byte fields).
        let mut encodings = vec![0u8; program.encodings.span as usize];
        let mut chunk = vec![0u8; program.chunk.span as usize];
        let mut luts: Vec<Vec<[u8; 4]>> = Vec::new();
        let store = self.columns.as_ref().ok_or_else(|| {
            Error::config("layer", "prepare called before the columns were evaluated")
        })?;
        let mut col = 0;
        for (i, desc) in M::CHANNELS.iter().enumerate() {
            let bytes = match &self.encodings[i] {
                Some(Encoding::Column(c)) => {
                    let func = c.func();
                    if glue.relative.contains(&i) {
                        let origin = store.columns()[col].origin();
                        write_field(
                            &mut chunk,
                            &program.chunk,
                            &format!("{}_base", desc.name),
                            &func.chunk_base(origin).to_le_bytes(),
                        )?;
                    }
                    if glue.luts.iter().any(|l| l.channel == i) {
                        for r in func.resources() {
                            let Resource::Lut(lut) = r;
                            luts.push(lut);
                        }
                    }
                    col += 1;
                    func.params_bytes()?
                }
                Some(Encoding::Const(v)) => v.bytes(),
                None => desc.default.bytes(),
            };
            write_field(&mut encodings, &program.encodings, desc.name, &bytes)?;
        }
        write_field(&mut chunk, &program.chunk, "row_base", &0u32.to_le_bytes())?;

        let rows = store.rows();
        let ranges = (0..store.columns().len())
            .map(|k| store.column_range(k))
            .collect();
        let store = self.columns.as_mut().expect("checked above");
        let buffer = store.upload(cx)?.clone();

        // Zoom, pan and resize land here every frame: write the new
        // uniform values into the existing buffers.
        if let Some((gpu, held)) = &self.gpu
            && gpu.reusable(cx, &program, &buffer)
            && *held == luts
        {
            gpu.write_uniforms(cx, &encodings, &chunk);
            return Ok(MarkBatch {
                gpu: Arc::clone(gpu),
            });
        }
        let gpu = Arc::new(
            LayerUniforms {
                program,
                encodings,
                chunk,
                luts: luts.iter().map(Vec::as_slice).collect(),
            }
            .build(cx, buffer, ranges, rows, M::VERTICES_PER_INSTANCE),
        );
        self.gpu = Some((Arc::clone(&gpu), luts));
        Ok(MarkBatch { gpu })
    }

    fn glue_source(&self) -> Glue {
        self.glue()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn context(e: Error, mark: &str, channel: &str) -> Error {
    match e {
        Error::Configuration { what, detail } => Error::Configuration {
            what,
            detail: format!("{mark}::{} channel: {detail}", channel.to_uppercase()),
        },
        other => other,
    }
}

/// Copy `bytes` into member `name` of a uniform struct, checking it fits
/// before the next member (or the end of the struct).
fn write_field(
    dst: &mut [u8],
    layout: &crate::shader::StructLayout,
    name: &str,
    bytes: &[u8],
) -> Result<()> {
    let offset = layout.offset(name).ok_or_else(|| {
        Error::config(
            "generated uniform layout",
            format!("member `{name}` missing from {:?}", layout.members),
        )
    })? as usize;
    let end = layout
        .members
        .iter()
        .map(|&(_, o)| o as usize)
        .filter(|&o| o > offset)
        .min()
        .unwrap_or(dst.len());
    if offset + bytes.len() > end {
        return Err(Error::config(
            "generated uniform layout",
            format!(
                "`{name}` needs {} bytes but has {} (encase size vs generated layout mismatch)",
                bytes.len(),
                end - offset
            ),
        ));
    }
    dst[offset..offset + bytes.len()].copy_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::Px;
    use crate::encoding::ShaderFn;
    use crate::marks::Circle;
    use crate::scale::{Linear, Log, ScaleRef, Sequential};
    use crate::shader::link;
    use crate::shader::testing::{parse, struct_layout};
    use std::path::Path;

    pub(crate) struct Row {
        x: f64,
        y: f64,
        v: f64,
    }

    /// The reference signature of RFC-001 §6: linear x, log y, sequential
    /// fill, constant radius.
    pub(crate) fn reference() -> Selection<Row, Circle> {
        let rows = (1..=20)
            .map(|i| {
                let i = f64::from(i);
                Row {
                    x: 1.7e9 + i * 60.0,
                    y: i * i,
                    v: i,
                }
            })
            .collect::<Vec<_>>();
        let mut sel = Selection::<Row, Circle>::new(rows);
        sel.attr(
            Circle::X,
            ScaleRef::new(Linear::new()).encode(|r: &Row| r.x),
        )
        .attr(Circle::Y, Log::new().encode(|r: &Row| r.y))
        .attr(Circle::FILL, Sequential::viridis().encode(|r: &Row| r.v))
        .attr(Circle::RADIUS, Px(4.0));
        sel
    }

    /// Compare `actual` with a checked-in fixture; `GUP_BLESS=1` rewrites
    /// it.
    fn check_fixture(name: &str, actual: &str) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        if std::env::var_os("GUP_BLESS").is_some() || !path.exists() {
            std::fs::write(&path, actual).unwrap();
        }
        let expected = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            actual,
            expected,
            "{} changed; re-run with GUP_BLESS=1 and review the diff",
            path.display()
        );
    }

    /// Two threads that miss the pipeline cache at once link the program
    /// once: the cache lock is held across the link.
    #[test]
    fn pipeline_cache_miss_links_once() {
        let host = Context::shared().expect("shared context");
        // A fresh cache on the shared device, so this misses.
        let cx = Context::from_wgpu(host.device().clone(), host.queue().clone());
        let glue = reference().glue();
        std::thread::scope(|s| {
            for _ in 0..2 {
                s.spawn(|| {
                    let program = cx.pipelines().program(&cx, &glue).unwrap();
                    drop(cx.text());
                    program
                });
            }
        });
        let stats = cx.pipelines().stats;
        assert_eq!(stats.programs_linked, 1, "{stats:?}");
    }

    /// The glue emitter's output is unchanged by build-time composition
    /// (GUP-406), and what wgpu compiles on every target is checked in.
    #[test]
    fn reference_glue_matches_fixtures() {
        let glue = reference().glue();
        assert_eq!(
            glue.signature,
            "Circle {x: f32rel→gup::scale::linear::map_rel, y: f32→gup::scale::log::map, \
             radius: const f32, fill: f32→gup::color::sequential::map(lut)}"
        );
        assert_eq!(glue.columns, vec![0, 1, 3]);
        assert_eq!(glue.relative, vec![0]);
        check_fixture("scatter_glue.wgsl", &glue.source);
        let linked = link(&glue.signature, &glue.source, &glue.modules).unwrap();
        check_fixture("scatter_linked.wgsl", &linked);
    }

    /// The run-time path (glue linked to the library flattened at build
    /// time) against naga_oil composing the same glue directly: the same
    /// entry points and every struct laid out the same. naga_oil is a
    /// dev-dependency here; native tests are the only place it can check
    /// what wasm runs.
    #[test]
    fn linked_glue_matches_naga_oil_composition() {
        let glue = reference().glue();
        let linked = parse(
            &glue.signature,
            &link(&glue.signature, &glue.source, &glue.modules).unwrap(),
        );
        let dir = gup_wgsl::compose::read_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shaders"),
            "src/shaders",
        )
        .unwrap();
        let mut oracle = gup_wgsl::compose::Library::new(&dir.modules).unwrap();
        let composed = oracle
            .compose(&glue.signature, &glue.source)
            .unwrap_or_else(|e| panic!("{e}"));
        let entry_points = |m: &naga::Module| {
            m.entry_points
                .iter()
                .map(|e| (e.name.clone(), e.stage))
                .collect::<Vec<_>>()
        };
        assert_eq!(entry_points(&linked), entry_points(&composed));
        assert_eq!(entry_points(&linked).len(), 2);
        let mut a = gup_wgsl::compose::struct_layouts(&linked);
        let mut b = gup_wgsl::compose::struct_layouts(&composed);
        a.sort_by(|x, y| x.0.cmp(&y.0));
        b.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(a, b);
        assert_eq!(a.len(), 9, "{a:?}");
    }

    /// The uniform offsets gup-core writes come from encase sizes under
    /// the 16-byte `Params` rule (no naga at run time); naga's layout of the
    /// linked module must agree, so the two never drift silently.
    #[test]
    fn uniform_offsets_match_naga_layout() {
        let glue = reference().glue();
        let linked = parse(
            &glue.signature,
            &link(&glue.signature, &glue.source, &glue.modules).unwrap(),
        );
        assert_eq!(
            Some(glue.encodings.clone()),
            struct_layout(&linked, "Encodings")
        );
        assert_eq!(Some(glue.chunk.clone()), struct_layout(&linked, "Chunk"));
        // Every Encodings field starts on a 16-byte boundary.
        assert!(
            glue.encodings
                .members
                .iter()
                .filter(|(name, _)| !name.contains("_pad_"))
                .all(|(_, o)| o % 16 == 0),
            "{:?}",
            glue.encodings
        );
        assert_eq!(glue.encodings.span, 64);
    }

    #[test]
    fn rust_params_match_wgsl_params() {
        use crate::shader::{COLOR_SEQUENTIAL, SCALE_LINEAR, SCALE_LOG};
        use encase::ShaderType;
        for (module, size) in [
            (&SCALE_LINEAR, crate::scale::LinearParams::min_size().get()),
            (&SCALE_LOG, crate::scale::LogParams::min_size().get()),
            (
                &COLOR_SEQUENTIAL,
                crate::scale::SequentialParams::min_size().get(),
            ),
        ] {
            let wgsl = parse(module.import_path, module.wgsl);
            let params = gup_wgsl::flat_name(module.import_path, "Params");
            let layout = struct_layout(&wgsl, &params).unwrap();
            assert_eq!(u64::from(layout.span), size, "{}", module.import_path);
        }
    }
}

/// Wall-clock cost of creating the reference pipeline (RFC-001 §12 risk
/// 2): run with
/// `cargo test -p gup-core --lib pipeline_timings -- --ignored --nocapture`
/// (add `--release` for release numbers).
#[cfg(test)]
mod timings {
    use super::tests::reference;
    use crate::context::Context;
    use crate::render::TargetDesc;
    use std::time::{Duration, Instant};

    fn summary(name: &str, mut v: Vec<Duration>) {
        v.sort();
        let ms = |d: Duration| d.as_secs_f64() * 1e3;
        eprintln!(
            "{name:<36} min {:>8.3} ms  median {:>8.3}  max {:>8.3}",
            ms(v[0]),
            ms(v[v.len() / 2]),
            ms(v[v.len() - 1]),
        );
    }

    #[test]
    #[ignore = "measurement, not a check; see RFC-001 S0a and GUP-406 findings"]
    fn pipeline_timings() {
        const RUNS: usize = 20;
        let host = Context::new_blocking().unwrap();
        let info = host
            .adapter_info()
            .map(|i| format!("{} ({:?}, {})", i.name, i.backend, i.driver_info))
            .unwrap_or_default();
        let sel = reference();
        let desc = TargetDesc {
            format: wgpu::TextureFormat::Rgba8Unorm,
            width: 640,
            height: 400,
            dpr: 1.0,
            samples: 1,
        };
        let (mut context, mut emit, mut link, mut create, mut total) =
            (vec![], vec![], vec![], vec![], vec![]);
        let mut first = None;
        for _ in 0..RUNS {
            // A fresh pipeline cache on the same device.
            let t = Instant::now();
            let cx = Context::from_wgpu(host.device().clone(), host.queue().clone());
            context.push(t.elapsed());
            let t = Instant::now();
            let glue = sel.glue();
            emit.push(t.elapsed());
            let program = cx.pipelines().program(&cx, &glue).unwrap();
            let _pipeline = cx.pipelines().mark_pipeline(&cx, &program, &desc);
            let stats = cx.pipelines().stats;
            link.push(stats.last_link);
            create.push(stats.last_create);
            total.push(stats.last_link + stats.last_create);
            first.get_or_insert(stats.last_link + stats.last_create);
        }
        eprintln!(
            "profile: {}, runs: {RUNS}, adapter: {info}",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
        summary("context creation (from_wgpu)", context);
        summary("glue emit", emit);
        summary("link (gup_wgsl::link)", link);
        summary("create_shader_module + pipeline", create);
        eprintln!(
            "link + create, first run             {:>8.3} ms",
            first.unwrap().as_secs_f64() * 1e3
        );
        summary("link + create", total);
    }
}
