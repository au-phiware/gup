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

/// What bytes written to GPU memory are for. Every write `gup-core` makes
/// goes through [`Context`] and is counted under one of these, so tests and
/// benchmarks can prove claims such as "zooming writes 0 column bytes"
/// (RFC-001 §1).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Upload {
    /// Column chunks: the data, written once per chunk and context.
    Column,
    /// Uniform buffers: encodings, per-chunk bases, the view.
    Uniform,
    /// Per-frame guide instances: rules and glyph quads.
    Instances,
    /// Textures: palette LUTs and the glyph atlas.
    Texture,
}

impl Upload {
    const ALL: [Upload; 4] = [
        Upload::Column,
        Upload::Uniform,
        Upload::Instances,
        Upload::Texture,
    ];
}

/// Bytes and write calls of one [`Upload`] kind.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct WriteCount {
    /// Bytes written.
    pub bytes: u64,
    /// `write_buffer`/`write_texture` calls.
    pub writes: u64,
}

impl std::ops::Sub for WriteCount {
    type Output = WriteCount;
    fn sub(self, rhs: Self) -> Self {
        Self {
            bytes: self.bytes - rhs.bytes,
            writes: self.writes - rhs.writes,
        }
    }
}

/// Everything a [`Context`] has written to GPU memory, by kind. Take two
/// snapshots and subtract them to measure an interval.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct UploadStats {
    /// Column chunk bytes.
    pub columns: WriteCount,
    /// Uniform bytes.
    pub uniforms: WriteCount,
    /// Guide instance bytes.
    pub instances: WriteCount,
    /// Texture bytes.
    pub textures: WriteCount,
}

impl std::ops::Sub for UploadStats {
    type Output = UploadStats;
    fn sub(self, rhs: Self) -> Self {
        Self {
            columns: self.columns - rhs.columns,
            uniforms: self.uniforms - rhs.uniforms,
            instances: self.instances - rhs.instances,
            textures: self.textures - rhs.textures,
        }
    }
}

/// Lock-free per-kind counters.
#[derive(Default)]
struct UploadCounters {
    bytes: [AtomicU64; 4],
    writes: [AtomicU64; 4],
}

impl UploadCounters {
    fn add(&self, kind: Upload, bytes: usize) {
        self.bytes[kind as usize].fetch_add(bytes as u64, Ordering::Relaxed);
        self.writes[kind as usize].fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> UploadStats {
        let get = |kind: Upload| WriteCount {
            bytes: self.bytes[kind as usize].load(Ordering::Relaxed),
            writes: self.writes[kind as usize].load(Ordering::Relaxed),
        };
        let [columns, uniforms, instances, textures] = Upload::ALL.map(get);
        UploadStats {
            columns,
            uniforms,
            instances,
            textures,
        }
    }
}

struct Inner {
    id: ContextId,
    /// Set when Gup created the device; `WindowTarget` creates surfaces
    /// from it.
    #[cfg_attr(not(feature = "window"), allow(dead_code))]
    instance: Option<wgpu::Instance>,
    adapter: Option<wgpu::Adapter>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    caps: Caps,
    shaders: Mutex<ShaderLibrary>,
    pipelines: Mutex<PipelineCache>,
    text: Mutex<TextSystem>,
    uploads: UploadCounters,
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
                uploads: UploadCounters::default(),
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

    /// The adapter, when this context created its own device.
    pub fn adapter_info(&self) -> Option<wgpu::AdapterInfo> {
        self.inner.adapter.as_ref().map(wgpu::Adapter::get_info)
    }

    /// The instance and adapter, when this context created its own device.
    #[cfg(feature = "window")]
    pub(crate) fn instance_and_adapter(&self) -> Option<(&wgpu::Instance, &wgpu::Adapter)> {
        self.inner
            .instance
            .as_ref()
            .zip(self.inner.adapter.as_ref())
    }

    /// The device's limits and features.
    pub fn caps(&self) -> &Caps {
        &self.inner.caps
    }

    /// Everything this context has written to GPU memory so far, by kind.
    pub fn upload_stats(&self) -> UploadStats {
        self.inner.uploads.snapshot()
    }

    /// Write `data` into `buffer` at `offset`, counted as `kind`. This,
    /// [`buffer_with_data`](Self::buffer_with_data) and
    /// [`write_texture`](Self::write_texture) are the only ways `gup-core`
    /// writes to GPU memory (`tests::every_gpu_write_is_counted` enforces
    /// it).
    pub(crate) fn write_buffer(
        &self,
        kind: Upload,
        buffer: &wgpu::Buffer,
        offset: u64,
        data: &[u8],
    ) {
        self.inner.uploads.add(kind, data.len());
        self.inner.queue.write_buffer(buffer, offset, data);
    }

    /// A new buffer holding `data` (`usage` gains `COPY_DST`), counted as
    /// `kind`.
    pub(crate) fn buffer_with_data(
        &self,
        kind: Upload,
        label: &str,
        usage: wgpu::BufferUsages,
        data: &[u8],
    ) -> wgpu::Buffer {
        let size = (data.len() as u64)
            .next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT)
            .max(wgpu::COPY_BUFFER_ALIGNMENT);
        let buffer = self.inner.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if data.len() as u64 == size {
            self.write_buffer(kind, &buffer, 0, data);
        } else {
            let mut padded = data.to_vec();
            padded.resize(size as usize, 0);
            self.write_buffer(kind, &buffer, 0, &padded);
        }
        buffer
    }

    /// Write `data` into `texture`, counted as [`Upload::Texture`].
    pub(crate) fn write_texture(
        &self,
        texture: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) {
        self.inner.uploads.add(Upload::Texture, data.len());
        self.inner.queue.write_texture(texture, data, layout, size);
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
        cx.write_buffer(Upload::Uniform, &src, 0, &payload);
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

    /// The upload counters are only proof if nothing bypasses them: no
    /// source file but this one may write to GPU memory directly. (The
    /// test-only compute harness in `scale/conformance.rs` is exempt; it
    /// builds its own inputs and never runs in a chart.)
    #[test]
    fn every_gpu_write_is_counted() {
        const DIRECT: [&str; 5] = [
            "queue().write_buffer",
            "write_texture(",
            "create_buffer_init",
            "create_texture_with_data",
            "mapped_at_creation: true",
        ];
        const EXEMPT: [&str; 2] = ["context.rs", "conformance.rs"];
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        assert!(files.len() > 10, "{files:?}");
        let mut offenders = Vec::new();
        for file in files {
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            if EXEMPT.contains(&name.as_str()) {
                continue;
            }
            let text = std::fs::read_to_string(&file).unwrap();
            for (n, line) in text.lines().enumerate() {
                if DIRECT.iter().any(|d| line.contains(d)) && !line.contains("cx.write_texture(") {
                    offenders.push(format!("{}:{}: {}", file.display(), n + 1, line.trim()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "write through Context::write_buffer / buffer_with_data / write_texture so \
             the upload counters see it:\n{}",
            offenders.join("\n")
        );
    }
}
