// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU error scopes (GUP-410): every step in which `gup-core` creates
//! pipelines, builds GPU state, encodes or submits runs inside
//! [`Context::scoped`], so a GPU error comes back as [`Error::Gpu`]
//! instead of panicking (wgpu's native default) or vanishing into the
//! browser console (wasm32, where wgpu installs no handler).
//!
//! Popping a scope is asynchronous in WebGPU. Natively wgpu-core resolves
//! it at once, so `scoped` returns the error from the call that caused it.
//! In a browser the error arrives later: `scoped` keeps the pending
//! scope on the context, the next synchronous call reports whatever has
//! arrived ([`Context::take_gpu_errors`]) and asynchronous readbacks wait
//! for every pending scope ([`Context::settle_gpu_errors`]).
//!
//! When any scope reports an error, the context forgets its cached
//! pipelines (`gup-core`'s and the text system's), so an invalid pipeline
//! is never reused: the next render creates it again and reports the same
//! error again.

use crate::context::Context;
use crate::error::{Error, Result};
use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::Poll;

/// The error filters each scope pushes, in the order their errors are
/// preferred. wgpu's WebGPU backend panics on a `GPUInternalError`
/// (`Error::from_js`), so in a browser internal errors stay uncaptured
/// and reach the context's uncaptured-error handler instead.
#[cfg(not(target_arch = "wasm32"))]
const FILTERS: [wgpu::ErrorFilter; 3] = [
    wgpu::ErrorFilter::Validation,
    wgpu::ErrorFilter::OutOfMemory,
    wgpu::ErrorFilter::Internal,
];
#[cfg(target_arch = "wasm32")]
const FILTERS: [wgpu::ErrorFilter; 2] = [
    wgpu::ErrorFilter::Validation,
    wgpu::ErrorFilter::OutOfMemory,
];

/// wgpu-core keeps one scope stack per device, shared by every thread
/// (and every `Context` wrapping that device). Scopes from two threads
/// would interleave and catch each other's errors, so the outermost scope
/// on a thread holds this lock until it pops. Nested scopes on the same
/// thread do not take it again.
static SCOPES: Mutex<()> = Mutex::new(());

thread_local! {
    /// How many scopes this thread has open.
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// A GPU error as `gup-core` reports it: what failed and the GPU's
/// message. `Send`, unlike `wgpu::Error` in a browser.
#[derive(Clone, Debug)]
pub(crate) struct GpuFailure {
    what: String,
    message: String,
}

impl GpuFailure {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn new(what: impl Into<String>, error: &wgpu::Error) -> Self {
        Self {
            what: what.into(),
            message: error.to_string(),
        }
    }

    fn into_error(self) -> Error {
        Error::Gpu {
            what: self.what,
            message: self.message,
        }
    }
}

/// A popped scope's future for one filter.
type PopFuture = Pin<Box<dyn Future<Output = Option<wgpu::Error>>>>;

/// One popped scope: a future per filter, resolving to the message of
/// the first error in [`FILTERS`] order.
struct Popped {
    futures: Vec<Option<PopFuture>>,
    errors: Vec<Option<String>>,
}

impl Future for Popped {
    type Output = Option<String>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        for (slot, error) in this.futures.iter_mut().zip(&mut this.errors) {
            if let Some(future) = slot
                && let Poll::Ready(e) = future.as_mut().poll(cx)
            {
                *error = e.map(|e| e.to_string());
                *slot = None;
            }
        }
        if this.futures.iter().any(Option::is_some) {
            return Poll::Pending;
        }
        Poll::Ready(this.errors.iter_mut().find_map(Option::take))
    }
}

impl Popped {
    /// Poll once without waiting: natively always ready; in a browser,
    /// ready once the scope's promises have settled.
    fn poll_now(&mut self) -> Poll<Option<String>> {
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        Pin::new(self).poll(&mut cx)
    }
}

/// An open error scope; popped by [`Scope::pop`], or on drop if the
/// scoped work panicked.
struct Scope<'a> {
    device: &'a wgpu::Device,
    open: bool,
    _serial: Option<MutexGuard<'static, ()>>,
}

impl<'a> Scope<'a> {
    fn push(device: &'a wgpu::Device) -> Self {
        let depth = DEPTH.get();
        let serial = (depth == 0).then(|| {
            // The scope lock is always taken first: never while holding
            // a context's pipeline or text lock.
            #[cfg(debug_assertions)]
            crate::context::assert_no_context_locks("an outermost GPU error scope");
            lock(&SCOPES)
        });
        DEPTH.set(depth + 1);
        for filter in FILTERS {
            device.push_error_scope(filter);
        }
        Self {
            device,
            open: true,
            _serial: serial,
        }
    }

    fn pop(mut self) -> Popped {
        self.open = false;
        let mut futures: Vec<_> = FILTERS
            .iter()
            .map(|_| Some(Box::pin(self.device.pop_error_scope()) as PopFuture))
            .collect();
        // Popped innermost first: the last filter pushed.
        futures.reverse();
        Popped {
            errors: vec![None; futures.len()],
            futures,
        }
    }
}

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        if self.open {
            for _ in FILTERS {
                drop(self.device.pop_error_scope());
            }
        }
        DEPTH.set(DEPTH.get() - 1);
    }
}

/// GPU errors that arrived after the call that caused them returned: in
/// a browser, scopes that settled later and uncaptured errors.
#[derive(Default)]
pub(crate) struct GpuErrors {
    /// Shared with the uncaptured-error handler.
    late: Arc<Mutex<Vec<GpuFailure>>>,
    /// Scopes whose errors have not arrived yet (wasm32 only; natively
    /// every scope resolves when it is popped). A spawned task drives
    /// each scope's promises and sends the result, so the context stays
    /// `Send + Sync`.
    #[cfg(target_arch = "wasm32")]
    pending: Mutex<Vec<futures_channel::oneshot::Receiver<Option<GpuFailure>>>>,
}

impl GpuErrors {
    /// An uncaptured-error handler that records into these errors.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn handler(&self) -> Arc<dyn wgpu::UncapturedErrorHandler> {
        let late = Arc::clone(&self.late);
        Arc::new(move |e: wgpu::Error| {
            lock(&late).push(GpuFailure::new(
                "GPU work outside any gup-core call (uncaptured)",
                &e,
            ));
        })
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Context {
    /// Run `f` inside a GPU error scope. A GPU error raised by `f`'s work
    /// becomes [`Error::Gpu`] naming `what`: returned from this call
    /// natively (where `what` is only called to report an error), and
    /// from a later call in a browser. An error `f` returns itself wins.
    pub(crate) fn scoped<T>(
        &self,
        what: impl FnOnce() -> String,
        f: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let scope = Scope::push(self.device());
        let result = f();
        let mut popped = scope.pop();
        match popped.poll_now() {
            Poll::Ready(None) => result,
            Poll::Ready(Some(message)) => {
                self.forget_pipelines();
                result.and(Err(Error::Gpu {
                    what: what(),
                    message,
                }))
            }
            Poll::Pending => {
                self.defer(what(), popped);
                result
            }
        }
    }

    /// Keep a scope whose errors have not arrived yet.
    #[cfg(target_arch = "wasm32")]
    fn defer(&self, what: String, popped: Popped) {
        let (tx, rx) = futures_channel::oneshot::channel();
        wasm_bindgen_futures::spawn_local(async move {
            let failure = popped.await.map(|message| GpuFailure { what, message });
            let _ = tx.send(failure);
        });
        lock(&self.gpu_errors().pending).push(rx);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn defer(&self, _: String, _: Popped) {
        unreachable!("wgpu-core resolves error scopes when they are popped");
    }

    /// Report GPU errors that arrived since the last call (browser
    /// scopes that settled, uncaptured errors): the first as `Err`, the
    /// rest dropped (they usually follow from the first).
    pub(crate) fn take_gpu_errors(&self) -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        {
            let mut arrived = Vec::new();
            lock(&self.gpu_errors().pending).retain_mut(|rx| match rx.try_recv() {
                Ok(Some(failure)) => {
                    arrived.extend(failure);
                    false
                }
                Ok(None) => true,
                Err(futures_channel::oneshot::Canceled) => false,
            });
            lock(&self.gpu_errors().late).extend(arrived);
        }
        let first = {
            let mut late = lock(&self.gpu_errors().late);
            let first = late.first().cloned();
            late.clear();
            first
        };
        match first {
            None => Ok(()),
            Some(failure) => {
                self.forget_pipelines();
                Err(failure.into_error())
            }
        }
    }

    /// Wait for every pending scope, then report as
    /// [`take_gpu_errors`](Self::take_gpu_errors) does. Readbacks call it
    /// so that a browser render returns its own GPU errors.
    pub(crate) async fn settle_gpu_errors(&self) -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        {
            let pending = std::mem::take(&mut *lock(&self.gpu_errors().pending));
            for rx in pending {
                if let Ok(Some(failure)) = rx.await {
                    lock(&self.gpu_errors().late).push(failure);
                }
            }
        }
        self.take_gpu_errors()
    }
}

#[cfg(test)]
mod tests {
    use crate::render::GlueProgram;
    use crate::scene::{ItemKind, MarkBatch, Scene};
    use crate::{
        Circle, Context, Error, ImageTarget, Linear, Log, Plot, Renderer, Selection, ShaderFn,
        TargetDesc,
    };
    use std::sync::Arc;

    /// The two-point reference scene, resolved on `cx` (no pipelines yet).
    fn scene(cx: &Context) -> Scene {
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
        plot.add(Selection::<(f64, f64), Circle>::new(vec![
            (1.0, 1.0),
            (5.0, 100.0),
        ]))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1));
        plot.resolve(cx, 300.0, 200.0).unwrap().scene
    }

    /// `scene` with its mark layer drawn by `seed(program)`.
    fn seeded(
        cx: &Context,
        mut scene: Scene,
        seed: impl Fn(&Context, &GlueProgram) -> GlueProgram,
    ) -> Scene {
        for item in &mut scene.items {
            if let ItemKind::Marks(batch) = &mut item.kind {
                let bad = Arc::new(seed(cx, &batch.gpu.program));
                *batch = MarkBatch {
                    gpu: Arc::new(batch.gpu.with_program(bad)),
                };
            }
        }
        scene
    }

    fn copy(p: &GlueProgram, wgsl: String, layout: wgpu::PipelineLayout) -> GlueProgram {
        GlueProgram {
            glue: p.glue.clone(),
            wgsl,
            encodings: p.encodings.clone(),
            chunk: p.chunk.clone(),
            enc_bgl: p.enc_bgl.clone(),
            chunk_bgl: p.chunk_bgl.clone(),
            layout,
        }
    }

    /// A pipeline layout without group 2 (the per-chunk uniforms the glue
    /// binds): a bind group layout mismatch only pipeline creation sees.
    fn missing_group(cx: &Context, p: &GlueProgram) -> GlueProgram {
        let view = cx.pipelines().view_layout(cx.device());
        let layout = cx
            .device()
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("seeded: no chunk group"),
                bind_group_layouts: &[&view, &p.enc_bgl],
                push_constant_ranges: &[],
            });
        copy(p, p.wgsl.clone(), layout)
    }

    /// WGSL that does not parse.
    fn broken_wgsl(_: &Context, p: &GlueProgram) -> GlueProgram {
        copy(p, format!("{}\nfn seeded( {{", p.wgsl), p.layout.clone())
    }

    fn gpu_error(e: Error) -> (String, String) {
        match e {
            Error::Gpu { what, message } => (what, message),
            other => panic!("expected Error::Gpu, got {other:?}"),
        }
    }

    /// GUP-410 seeded proof (native): an invalid mark pipeline is an
    /// `Err` from `render`, naming the pipeline and carrying wgpu's
    /// message, not a panic; it is not cached, so the next render
    /// reports it again; and the context renders the valid scene after.
    #[test]
    fn an_invalid_mark_pipeline_is_an_err_every_time_and_never_cached() {
        let cx = Context::new_blocking().unwrap();
        let good = scene(&cx);
        let bad = seeded(&cx, good.clone(), missing_group);
        let signature = bad
            .items
            .iter()
            .find_map(|i| match &i.kind {
                ItemKind::Marks(b) => Some(b.gpu.program.glue.signature.clone()),
                _ => None,
            })
            .expect("a mark layer");
        let mut target = ImageTarget::new(&cx, 300, 200).unwrap();
        for attempt in 0..2 {
            let (what, message) = gpu_error(target.render_blocking(&cx, &bad).unwrap_err());
            eprintln!("attempt {attempt}: GPU error in {what}: {message}");
            assert!(
                what.contains(&format!("mark pipeline `{signature}`")),
                "{what}"
            );
            assert!(message.contains("create_render_pipeline"), "{message}");
            assert!(cx.pipelines().mark_pipelines().is_empty());
        }
        let image = target.render_blocking(&cx, &good).unwrap();
        assert_eq!(image.dimensions(), (300, 200));
        assert_eq!(cx.pipelines().mark_pipelines(), vec![signature]);
    }

    /// A WGSL error is an `Err` carrying the WGSL diagnostic, from
    /// `Renderer::prepare` for hosts that own the pass.
    #[test]
    fn a_wgsl_error_is_an_err_from_prepare() {
        let cx = Context::new_blocking().unwrap();
        let bad = seeded(&cx, scene(&cx), broken_wgsl);
        let desc = TargetDesc {
            format: wgpu::TextureFormat::Rgba8Unorm,
            width: 300,
            height: 200,
            dpr: 1.0,
            samples: 1,
        };
        let (what, message) = gpu_error(Renderer::new().prepare(&cx, &bad, &desc).unwrap_err());
        eprintln!("GPU error in {what}: {message}");
        assert!(what.starts_with("mark pipeline `Circle {"), "{what}");
        assert!(message.contains("seeded"), "{message}");
        assert_eq!(cx.submissions(), 0);
    }

    /// A target whose frame cannot be rendered to: the render scope
    /// (encoding and submission) turns the failure into an `Err`.
    #[test]
    fn a_failed_submission_is_an_err_from_render() {
        use crate::target::{Frame, RenderTarget};
        struct NotAttachable(wgpu::Texture);
        impl RenderTarget for NotAttachable {
            fn desc(&self) -> TargetDesc {
                TargetDesc {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    width: 300,
                    height: 200,
                    dpr: 1.0,
                    samples: 1,
                }
            }
            fn acquire(&mut self, _: &Context) -> crate::Result<Frame> {
                let view = self.0.create_view(&Default::default());
                Ok(Frame::new(self.0.clone(), view, None, None))
            }
            fn present(
                &mut self,
                cx: &Context,
                _: Frame,
                commands: wgpu::CommandBuffer,
            ) -> crate::Result<()> {
                cx.submit([commands]);
                Ok(())
            }
        }
        let cx = Context::new_blocking().unwrap();
        let texture = cx.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("seeded: no RENDER_ATTACHMENT"),
            size: wgpu::Extent3d {
                width: 300,
                height: 200,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let mut target = NotAttachable(texture);
        let (what, message) = gpu_error(
            Renderer::new()
                .render(&cx, &scene(&cx), &mut target)
                .unwrap_err(),
        );
        eprintln!("GPU error in {what}: {message}");
        assert!(what.starts_with("drawing a scene into a 300×200"), "{what}");
        assert!(message.contains("RENDER_ATTACHMENT"), "{message}");
    }
}
