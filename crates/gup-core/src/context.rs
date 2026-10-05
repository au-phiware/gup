// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one GPU context (RFC-001 §2).
//!
//! A [`Context`] owns (or wraps) a `wgpu::Device` and `wgpu::Queue` together
//! with every device-scoped cache: the naga_oil shader library, the pipeline
//! cache and the glyph atlas. It is cheap to clone (`Arc<Inner>`), so charts,
//! targets and hosts can all hold one.

use crate::error::{Error, Result};
use crate::render::PipelineCache;
use crate::shader::ShaderLibrary;
use crate::text::TextSystem;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Identifies a [`Context`]. GPU state built for one context is tagged with
/// its id so it is never used with another device.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContextId(u64);

impl ContextId {
    fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// What the device can do, read once from its limits and features.
#[derive(Clone, Debug)]
pub struct Caps {
    /// The device's limits (exactly what it was created with).
    pub limits: wgpu::Limits,
    /// The device's enabled features.
    pub features: wgpu::Features,
}

impl Caps {
    fn of(device: &wgpu::Device) -> Self {
        Self {
            limits: device.limits(),
            features: device.features(),
        }
    }
}

struct Inner {
    id: ContextId,
    // Kept alive for the device's lifetime when Gup created it; S0b's
    // `WindowTarget` creates surfaces from the instance.
    #[allow(dead_code)]
    instance: Option<wgpu::Instance>,
    #[allow(dead_code)]
    adapter: Option<wgpu::Adapter>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    caps: Caps,
    shaders: Mutex<ShaderLibrary>,
    pipelines: Mutex<PipelineCache>,
    text: Mutex<TextSystem>,
}

/// Gup's GPU context: a device, its queue and the device-scoped caches.
///
/// Create one with [`Context::new_blocking`] (or [`Context::new`] in async
/// code), or wrap a device the host already owns with
/// [`Context::from_wgpu`].
#[derive(Clone)]
pub struct Context {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("id", &self.inner.id)
            .field("owns_device", &self.inner.adapter.is_some())
            .finish_non_exhaustive()
    }
}

impl Context {
    /// Create a context with its own instance, adapter and device.
    ///
    /// The device is created with the adapter's own buffer-size limits
    /// (not the WebGPU defaults), so column chunks can be large on capable
    /// hardware (RFC-001 §2). Respects `WGPU_BACKEND` and friends.
    pub async fn new() -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::from_env()
                    .unwrap_or(wgpu::PowerPreference::HighPerformance),
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await?;
        let adapter_limits = adapter.limits();
        let required_limits = wgpu::Limits {
            max_buffer_size: adapter_limits.max_buffer_size,
            max_storage_buffer_binding_size: adapter_limits.max_storage_buffer_binding_size,
            ..wgpu::Limits::downlevel_defaults()
        }
        .using_resolution(adapter_limits.clone());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("gup-core device"),
                required_features: wgpu::Features::empty(),
                required_limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await?;
        Ok(Self::build(Some(instance), Some(adapter), device, queue))
    }

    /// Blocking form of [`Context::new`] (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_blocking() -> Result<Self> {
        pollster::block_on(Self::new())
    }

    /// Wrap a device and queue the host already owns (egui, bevy, …).
    ///
    /// Requests nothing from the adapter: [`Caps`] is read from
    /// `device.limits()` and `device.features()`.
    pub fn from_wgpu(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self::build(None, None, device, queue)
    }

    fn build(
        instance: Option<wgpu::Instance>,
        adapter: Option<wgpu::Adapter>,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        let caps = Caps::of(&device);
        Self {
            inner: Arc::new(Inner {
                id: ContextId::next(),
                instance,
                adapter,
                device,
                queue,
                caps,
                shaders: Mutex::new(ShaderLibrary::new()),
                pipelines: Mutex::new(PipelineCache::default()),
                text: Mutex::new(TextSystem::new()),
            }),
        }
    }

    /// This context's id.
    pub fn id(&self) -> ContextId {
        self.inner.id
    }

    /// The device.
    pub fn device(&self) -> &wgpu::Device {
        &self.inner.device
    }

    /// The queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.inner.queue
    }

    /// The device's limits and features.
    pub fn caps(&self) -> &Caps {
        &self.inner.caps
    }

    /// Block until all submitted GPU work has finished (native), so that
    /// mapping callbacks have run.
    pub(crate) fn wait_idle(&self) -> Result<()> {
        self.inner
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map(|_| ())
            .map_err(|e| Error::Readback(e.to_string()))
    }

    pub(crate) fn shaders(&self) -> MutexGuard<'_, ShaderLibrary> {
        lock(&self.inner.shaders)
    }

    pub(crate) fn pipelines(&self) -> MutexGuard<'_, PipelineCache> {
        lock(&self.inner.pipelines)
    }

    pub(crate) fn text(&self) -> MutexGuard<'_, TextSystem> {
        lock(&self.inner.text)
    }
}

/// Lock a mutex, recovering the data if a panicking thread poisoned it: the
/// caches it guards stay structurally valid across a panic.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a few bytes through the queue and read them back through a
    /// mappable buffer, proving the device and queue are usable.
    fn round_trip(cx: &Context) -> Vec<u8> {
        let payload: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        let src = cx.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("round-trip src"),
            size: 8,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let dst = cx.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("round-trip dst"),
            size: 8,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        cx.queue().write_buffer(&src, 0, &payload);
        let mut enc = cx.device().create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(&src, 0, &dst, 0, 8);
        cx.queue().submit([enc.finish()]);
        dst.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
        cx.wait_idle().unwrap();
        let bytes = dst.slice(..).get_mapped_range().to_vec();
        dst.unmap();
        bytes
    }

    #[test]
    fn new_blocking_creates_a_usable_headless_context() {
        let cx = Context::new_blocking().expect("headless context");
        assert_eq!(round_trip(&cx), vec![1, 2, 3, 4, 5, 6, 7, 8]);
        // Clones share the same device and caches.
        let clone = cx.clone();
        assert_eq!(clone.id(), cx.id());
        assert!(clone.caps().limits.max_buffer_size >= 256 << 20);
    }

    #[test]
    fn from_wgpu_wraps_a_host_device_without_new_requests() {
        let host = Context::new_blocking().expect("host context");
        let wrapped = Context::from_wgpu(host.device().clone(), host.queue().clone());
        assert_ne!(wrapped.id(), host.id());
        assert_eq!(
            wrapped.caps().limits.max_buffer_size,
            host.device().limits().max_buffer_size
        );
        assert_eq!(wrapped.caps().features, host.device().features());
        assert_eq!(round_trip(&wrapped), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
