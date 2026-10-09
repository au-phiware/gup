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
