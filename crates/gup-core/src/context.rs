// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one GPU context (RFC-001 §2).
//!
//! A [`Context`] owns (or wraps) a `wgpu::Device` and `wgpu::Queue` together
//! with every device-scoped cache: the naga_oil shader library, the pipeline
//! cache and the glyph atlas. It is cheap to clone (`Arc<Inner>`), so charts,
//! targets and hosts can all hold one.

use crate::error::Result;
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
    /// Set when Gup created the device; surfaces are created from it.
    instance: Option<wgpu::Instance>,
    adapter: Option<wgpu::Adapter>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    caps: Caps,
    // Lock order (RFC-001 S1): `pipelines` may be held while `shaders` is
    // taken (a pipeline-cache miss composes its program), never the other
    // way round, and `text` is never held together with either. Debug
    // builds check this on every acquisition (`LockRank::check`).
    /// Taken first: composed programs and pipelines.
    pipelines: Mutex<PipelineCache>,
    /// The naga_oil composer and library modules. Taken alone, or while
    /// `pipelines` is held.
    shaders: Mutex<ShaderLibrary>,
    /// The glyph atlas. Never held together with `pipelines` or `shaders`.
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

/// How [`Context::with_options`] creates its device.
///
/// The defaults ask for the platform's primary backends (Vulkan, Metal,
/// DX12; WebGPU or WebGL in a browser) and a high-performance adapter.
/// `WGPU_BACKEND` and `WGPU_POWER_PREF` override `backends` and
/// `power_preference`.
#[derive(Clone, Debug)]
pub struct ContextOptions {
    /// Backends to look for an adapter on. If none of them has one, and
    /// they were not chosen through `WGPU_BACKEND`, GL is tried as an
    /// explicit fallback. GL is not in the native default: its EGL display
    /// binds to the window system's connection as soon as any surface
    /// exists, and dropping the instance after the event loop has exited
    /// then crashes in `eglTerminate` (RFC-001 "S0b findings", item 4).
    pub backends: wgpu::Backends,
    /// Which adapter to prefer.
    pub power_preference: wgpu::PowerPreference,
    /// Features the device must have; creation fails without them.
    pub required_features: wgpu::Features,
    /// Features to enable if the adapter has them, such as
    /// `TIMESTAMP_QUERY` for GPU timing (on by default). Read
    /// [`Caps::features`] to see which were enabled.
    pub optional_features: wgpu::Features,
    /// Limits the device must have on top of Gup's own: the WebGPU
    /// defaults where the adapter supports them (downlevel defaults
    /// otherwise), raised to the adapter's buffer sizes and texture
    /// resolution. `None` asks for Gup's own only.
    pub required_limits: Option<wgpu::Limits>,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            power_preference: wgpu::PowerPreference::HighPerformance,
            required_features: wgpu::Features::empty(),
            optional_features: wgpu::Features::TIMESTAMP_QUERY,
            required_limits: None,
        }
    }
}

impl Context {
    /// Create a context with its own instance, adapter and device, with
    /// [`ContextOptions::default`].
    ///
    /// The device is created with the adapter's own buffer-size limits
    /// (not the WebGPU defaults), so column chunks can be large on capable
    /// hardware (RFC-001 §2). Respects `WGPU_BACKEND` and friends.
    ///
    /// Every context preloads the shader library. Callers that just need
    /// a device should use [`Context::shared`] instead.
    pub async fn new() -> Result<Self> {
        Self::with_options(ContextOptions::default()).await
    }

    /// Like [`Context::new`], with the backends, adapter preference,
    /// features and limits in `options`.
    pub async fn with_options(options: ContextOptions) -> Result<Self> {
        let backends_from_env = wgpu::Backends::from_env().is_some();
        let power_preference =
            wgpu::PowerPreference::from_env().unwrap_or(options.power_preference);
        let (instance, adapter) = match request_adapter(options.backends, power_preference).await {
            Ok(found) => found,
            Err(e) if backends_from_env || options.backends.contains(wgpu::Backends::GL) => {
                return Err(e);
            }
            // The explicit GL fallback: only when the requested backends
            // have no adapter at all.
            Err(_) => request_adapter(wgpu::Backends::GL, power_preference).await?,
        };
        let required_limits = device_limits(&adapter.limits(), options.required_limits.as_ref());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("gup-core device"),
                required_features: options.required_features
                    | (options.optional_features & adapter.features()),
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

    /// The process-wide default context: created with
    /// [`Context::new_blocking`] on first use, then cheaply cloned (native
    /// only).
    ///
    /// Every `Context` preloads the naga_oil shader library, which costs
    /// about 6.6 ms in a release build (RFC-001 "S0a findings") on top of
    /// requesting an adapter and device. One-shot calls such as `save_png`
    /// use this shared default so they don't pay that on every call, and
    /// everything built on it shares one device, so its GPU resources can
    /// be used together.
    ///
    /// If creation fails, the error is returned and the next call tries
    /// again.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn shared() -> Result<Self> {
        static SHARED: std::sync::OnceLock<Context> = std::sync::OnceLock::new();
        static INIT: Mutex<()> = Mutex::new(());
        if let Some(cx) = SHARED.get() {
            return Ok(cx.clone());
        }
        // Serialise creation so that concurrent first calls make one device.
        let _creating = lock(&INIT);
        if let Some(cx) = SHARED.get() {
            return Ok(cx.clone());
        }
        let cx = Self::new_blocking()?;
        Ok(SHARED.get_or_init(|| cx).clone())
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

    /// The instance, when this context created its own device. Surfaces
    /// for this device must be created from it.
    pub fn instance(&self) -> Option<&wgpu::Instance> {
        self.inner.instance.as_ref()
    }

    /// The adapter, when this context created its own device.
    pub fn adapter(&self) -> Option<&wgpu::Adapter> {
        self.inner.adapter.as_ref()
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
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn wait_idle(&self) -> Result<()> {
        self.inner
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map(|_| ())
            .map_err(|e| crate::error::Error::Readback(e.to_string()))
    }

    /// The pipeline cache. Take it before [`shaders`](Self::shaders).
    pub(crate) fn pipelines(&self) -> Ordered<'_, PipelineCache> {
        Ordered::new(LockRank::Pipelines, &self.inner.pipelines)
    }

    /// The shader library: alone, or while holding
    /// [`pipelines`](Self::pipelines).
    pub(crate) fn shaders(&self) -> Ordered<'_, ShaderLibrary> {
        Ordered::new(LockRank::Shaders, &self.inner.shaders)
    }

    /// The text system: never together with the other two.
    pub(crate) fn text(&self) -> Ordered<'_, TextSystem> {
        Ordered::new(LockRank::Text, &self.inner.text)
    }
}

/// Find an adapter on `backends` (`WGPU_BACKEND` overrides them).
async fn request_adapter(
    backends: wgpu::Backends,
    power_preference: wgpu::PowerPreference,
) -> Result<(wgpu::Instance, wgpu::Adapter)> {
    let instance = wgpu::Instance::new(
        &wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        }
        .with_env(),
    );
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await?;
    Ok((instance, adapter))
}

/// The limits to create a device with on an adapter that has `adapter`
/// limits: the WebGPU defaults where the adapter supports them (downlevel
/// defaults otherwise, e.g. on GL), raised to the adapter's buffer sizes
/// and texture resolution, then to anything in `required`.
fn device_limits(adapter: &wgpu::Limits, required: Option<&wgpu::Limits>) -> wgpu::Limits {
    let base = if wgpu::Limits::default().check_limits(adapter) {
        wgpu::Limits::default()
    } else {
        wgpu::Limits::downlevel_defaults()
    };
    let limits = wgpu::Limits {
        max_buffer_size: adapter.max_buffer_size,
        max_storage_buffer_binding_size: adapter.max_storage_buffer_binding_size,
        ..base
    }
    .using_resolution(adapter.clone());
    match required {
        Some(required) => limits.or_better_values_from(required),
        None => limits,
    }
}

/// Lock a mutex, recovering the data if a panicking thread poisoned it: the
/// caches it guards stay structurally valid across a panic.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The cache locks a [`Context`] holds, as bits for the lock-order check.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum LockRank {
    Pipelines = 1 << 0,
    Shaders = 1 << 1,
    Text = 1 << 2,
}

#[cfg(debug_assertions)]
thread_local! {
    /// The [`LockRank`] bits of the context locks this thread holds.
    static HELD: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

impl LockRank {
    /// Panic if taking `self` while holding the locks in `held` breaks the
    /// lock order. The only nesting allowed is `shaders` inside
    /// `pipelines`. (The bits are shared by every `Context` on the thread,
    /// so nesting one context's locks inside another's is refused too.)
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    fn check(self, held: u8) {
        let allowed = match self {
            LockRank::Pipelines | LockRank::Text => 0,
            LockRank::Shaders => LockRank::Pipelines as u8,
        };
        if held & !allowed != 0 {
            let holding: Vec<LockRank> = [LockRank::Pipelines, LockRank::Shaders, LockRank::Text]
                .into_iter()
                .filter(|r| held & *r as u8 != 0)
                .collect();
            panic!(
                "gup-core Context lock order violated: taking {self:?} while holding \
                 {holding:?} (allowed: Pipelines then Shaders; Text alone)"
            );
        }
    }
}

/// A [`MutexGuard`] that, in debug builds, checks the lock order when it
/// is taken and records the lock as held by this thread until it drops.
pub(crate) struct Ordered<'a, T> {
    guard: MutexGuard<'a, T>,
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    rank: LockRank,
}

impl<'a, T> Ordered<'a, T> {
    fn new(rank: LockRank, mutex: &'a Mutex<T>) -> Self {
        #[cfg(debug_assertions)]
        HELD.with(|held| {
            rank.check(held.get());
            held.set(held.get() | rank as u8);
        });
        Self {
            guard: lock(mutex),
            rank,
        }
    }
}

impl<T> Drop for Ordered<'_, T> {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        HELD.with(|held| held.set(held.get() & !(self.rank as u8)));
    }
}

impl<T> std::ops::Deref for Ordered<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T> std::ops::DerefMut for Ordered<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
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

    /// `shared()` makes one context per process and pays the device
    /// request and library preload once: a later call is a clone.
    #[test]
    fn shared_is_one_context_created_once() {
        let t = std::time::Instant::now();
        let first = Context::shared().expect("shared context");
        let first_call = t.elapsed();
        let t = std::time::Instant::now();
        let second = Context::shared().expect("shared context");
        let second_call = t.elapsed();
        assert_eq!(first.id(), second.id());
        assert_eq!(first.device(), second.device());
        assert!(first.adapter().is_some() && first.instance().is_some());

        // Another test may have created the shared context already, so
        // compare a warm call with what creating a context really costs.
        let t = std::time::Instant::now();
        let fresh = Context::new_blocking().expect("fresh context");
        let creation = t.elapsed();
        assert_ne!(fresh.id(), first.id());
        assert!(
            second_call * 10 < creation,
            "shared() should be a clone after the first call: first {first_call:?}, \
             second {second_call:?}, new_blocking {creation:?}"
        );
    }

    /// The device is created with at least the WebGPU default limits
    /// where the adapter has them, plus the adapter's buffer sizes.
    #[test]
    fn device_limits_cover_the_webgpu_defaults_and_adapter_buffers() {
        let cx = Context::shared().expect("shared context");
        let adapter = cx.adapter().unwrap().limits();
        let limits = &cx.caps().limits;
        if wgpu::Limits::default().check_limits(&adapter) {
            assert!(wgpu::Limits::default().check_limits(limits), "{limits:?}");
        }
        assert_eq!(limits.max_buffer_size, adapter.max_buffer_size);
        assert_eq!(
            limits.max_texture_dimension_2d,
            adapter.max_texture_dimension_2d
        );
        let required = wgpu::Limits {
            max_bind_groups: 2,
            max_texture_dimension_2d: limits.max_texture_dimension_2d + 1,
            ..wgpu::Limits::downlevel_webgl2_defaults()
        };
        let raised = device_limits(&adapter, Some(&required));
        assert_eq!(raised.max_bind_groups, limits.max_bind_groups);
        assert_eq!(
            raised.max_texture_dimension_2d,
            limits.max_texture_dimension_2d + 1
        );
    }

    /// The one nesting the lock order allows: a pipeline-cache miss
    /// composes its program while holding the cache.
    #[test]
    fn lock_order_allows_shaders_inside_pipelines() {
        let cx = Context::shared().expect("shared context");
        let pipelines = cx.pipelines();
        let shaders = cx.shaders();
        drop((shaders, pipelines));
        // Released locks can be taken again, in either order.
        drop(cx.shaders());
        drop(cx.text());
        drop(cx.pipelines());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "lock order violated: taking Pipelines while holding [Shaders]")]
    fn lock_order_refuses_pipelines_inside_shaders() {
        let cx = Context::shared().expect("shared context");
        let _shaders = cx.shaders();
        let _pipelines = cx.pipelines();
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "lock order violated: taking Text while holding [Pipelines]")]
    fn lock_order_refuses_text_inside_pipelines() {
        let cx = Context::shared().expect("shared context");
        let _pipelines = cx.pipelines();
        let _text = cx.text();
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "lock order violated: taking Shaders while holding [Text]")]
    fn lock_order_refuses_shaders_inside_text() {
        let cx = Context::shared().expect("shared context");
        let _text = cx.text();
        let _shaders = cx.shaders();
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
