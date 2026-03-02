// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Linked view coordination for cross-chart selection.
//!
//! This module provides [`SharedSelectionState`] — a thread-safe, cheaply
//! cloneable selection state that multiple charts can share. When a brush or
//! click in one chart modifies the shared state, every other chart that
//! holds a clone can detect the change (via a generation counter) and
//! re-render with visual dimming applied to unselected items.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────┐       ┌─────────────────────────┐       ┌────────────┐
//! │  Chart A   │──────▶│  SharedSelectionState<K> │◀──────│  Chart B   │
//! │  (brush)   │       │  Arc<Mutex<Inner>>       │       │  (render)  │
//! └────────────┘       └─────────────────────────┘       └────────────┘
//! ```
//!
//! Selection identity is key-based (`K: Hash + Eq`) rather than index-based,
//! so two charts can share selection even if their data arrays are ordered
//! differently.
//!
//! # Quick Start
//!
//! ```rust
//! use gup::linked_selection::SharedSelectionState;
//!
//! // Create a shared state keyed by usize (item index).
//! let shared = SharedSelectionState::<usize>::new();
//!
//! // Clone cheaply — both handles point to the same inner data.
//! let shared2 = shared.clone();
//!
//! // Chart A selects some items.
//! shared.select([1, 3, 5]);
//! assert!(shared2.is_selected(&3));
//! assert!(!shared2.is_selected(&2));
//!
//! // Chart B detects the change via the generation counter.
//! assert!(shared2.generation() > 0);
//!
//! // Clear resets everything.
//! shared.clear();
//! assert!(!shared2.is_selected(&3));
//! ```
//!
//! # Visual Dimming
//!
//! The [`DimInstance`] trait allows mark instance types to have their alpha
//! channel reduced when an item is not part of the current selection. Use
//! [`build_dimmed_instances`] to produce a `Vec<I>` with dimming applied:
//!
//! ```rust
//! use gup::linked_selection::{SharedSelectionState, build_dimmed_instances};
//! use gup::mark::circle::CircleInstance;
//!
//! # let data: Vec<f32> = vec![1.0, 2.0, 3.0];
//! let shared = SharedSelectionState::<usize>::new();
//! shared.select([0]);
//!
//! let instances = build_dimmed_instances(
//!     &data,
//!     |d| CircleInstance {
//!         center: [*d, 0.0],
//!         radius: 0.05,
//!         _pad0: 0.0,
//!         fill_color: [1.0, 0.0, 0.0, 1.0],
//!         stroke_width: 0.0,
//!         _pad1: [0.0; 3],
//!         stroke_color: [0.0; 4],
//!     },
//!     |_d, idx| idx,  // key = index
//!     &shared,
//!     0.2,
//! );
//!
//! // Item 0 is selected → full opacity.
//! assert!((instances[0].fill_color[3] - 1.0).abs() < f32::EPSILON);
//! // Items 1, 2 are unselected → dimmed.
//! assert!((instances[1].fill_color[3] - 0.2).abs() < f32::EPSILON);
//! ```

use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// KeyedSelectionState — inner state
// ---------------------------------------------------------------------------

/// Key-based selection state storing selected item keys and a generation
/// counter for change detection.
///
/// This is the inner state wrapped by [`SharedSelectionState`]. You do not
/// need to interact with it directly — use `SharedSelectionState` instead.
#[derive(Clone)]
pub struct KeyedSelectionState<K: Hash + Eq> {
    selected: HashSet<K>,
    generation: u64,
}

impl<K: Hash + Eq + fmt::Debug> fmt::Debug for KeyedSelectionState<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyedSelectionState")
            .field("selected", &self.selected)
            .field("generation", &self.generation)
            .finish()
    }
}

impl<K: Hash + Eq> Default for KeyedSelectionState<K> {
    fn default() -> Self {
        Self {
            selected: HashSet::new(),
            generation: 0,
        }
    }
}

impl<K: Hash + Eq> KeyedSelectionState<K> {
    /// Create a new empty selection state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current generation counter.
    ///
    /// The generation is incremented on every mutation (`select`, `deselect`,
    /// `clear`), so consumers can compare against their last-seen generation
    /// to detect changes without inspecting the full set.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns `true` if the given key is in the selected set.
    pub fn is_selected(&self, key: &K) -> bool {
        self.selected.contains(key)
    }

    /// Returns the number of currently selected keys.
    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }

    /// Returns `true` if no keys are selected.
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Add the given keys to the selection. Increments the generation
    /// counter.
    pub fn select(&mut self, keys: impl IntoIterator<Item = K>) {
        self.selected.extend(keys);
        self.generation += 1;
    }

    /// Remove the given keys from the selection. Increments the generation
    /// counter.
    pub fn deselect<'a>(&mut self, keys: impl IntoIterator<Item = &'a K>)
    where
        K: 'a,
    {
        for key in keys {
            self.selected.remove(key);
        }
        self.generation += 1;
    }

    /// Replace the entire selection with the given keys. Increments the
    /// generation counter.
    pub fn set(&mut self, keys: impl IntoIterator<Item = K>) {
        self.selected.clear();
        self.selected.extend(keys);
        self.generation += 1;
    }

    /// Clear all selected keys. Increments the generation counter.
    pub fn clear(&mut self) {
        self.selected.clear();
        self.generation += 1;
    }

    /// Returns an iterator over the currently selected keys.
    pub fn selected_keys(&self) -> impl Iterator<Item = &K> {
        self.selected.iter()
    }
}

// ---------------------------------------------------------------------------
// SharedSelectionState — Arc<Mutex<…>> wrapper
// ---------------------------------------------------------------------------

/// Thread-safe, cheaply cloneable shared selection state for linked views.
///
/// Internally wraps `Arc<Mutex<KeyedSelectionState<K>>>`. Cloning produces
/// another handle to the **same** underlying data, so multiple charts can
/// observe and mutate the same selection.
///
/// All public methods acquire the lock internally, so callers never need
/// to manage the mutex directly.
///
/// # Type Parameter
///
/// - `K` — The key type used for cross-chart item identity. Must implement
///   `Hash + Eq + Send + Sync + 'static`. Common choices include `usize`
///   (index), `String` (named field), or a domain-specific ID type.
///
/// # Examples
///
/// ```rust
/// use gup::linked_selection::SharedSelectionState;
///
/// let state = SharedSelectionState::<usize>::new();
/// let state2 = state.clone(); // cheap Arc clone
///
/// state.select([10, 20, 30]);
/// assert_eq!(state2.selected_count(), 3);
/// assert!(state2.is_selected(&20));
/// ```
pub struct SharedSelectionState<K: Hash + Eq + Send + Sync + 'static> {
    inner: Arc<Mutex<KeyedSelectionState<K>>>,
}

impl<K: Hash + Eq + Send + Sync + 'static> Clone for SharedSelectionState<K> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<K: Hash + Eq + Send + Sync + fmt::Debug + 'static> fmt::Debug for SharedSelectionState<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner.try_lock() {
            Ok(guard) => f
                .debug_struct("SharedSelectionState")
                .field("inner", &*guard)
                .finish(),
            Err(_) => f
                .debug_struct("SharedSelectionState")
                .field("inner", &"<locked>")
                .finish(),
        }
    }
}

impl<K: Hash + Eq + Send + Sync + 'static> Default for SharedSelectionState<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Hash + Eq + Send + Sync + 'static> SharedSelectionState<K> {
    /// Create a new shared selection state with an empty selection.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(KeyedSelectionState::new())),
        }
    }

    /// Returns the current generation counter.
    ///
    /// Use this together with a locally cached generation to detect
    /// changes without reading the full selection set on every frame.
    pub fn generation(&self) -> u64 {
        self.inner.lock().expect("poisoned lock").generation()
    }

    /// Try to read the generation counter without blocking.
    ///
    /// Returns `None` if the lock is currently held by another thread.
    /// This is useful in hot render paths where blocking is undesirable.
    pub fn try_generation(&self) -> Option<u64> {
        self.inner.try_lock().ok().map(|g| g.generation())
    }

    /// Returns `true` if the given key is in the selected set.
    pub fn is_selected(&self, key: &K) -> bool {
        self.inner.lock().expect("poisoned lock").is_selected(key)
    }

    /// Returns the number of currently selected keys.
    pub fn selected_count(&self) -> usize {
        self.inner.lock().expect("poisoned lock").selected_count()
    }

    /// Returns `true` if no keys are selected.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().expect("poisoned lock").is_empty()
    }

    /// Add the given keys to the selection.
    ///
    /// Increments the generation counter so that all charts sharing this
    /// state can detect the change on their next render tick.
    pub fn select(&self, keys: impl IntoIterator<Item = K>) {
        self.inner.lock().expect("poisoned lock").select(keys);
    }

    /// Remove the given keys from the selection.
    ///
    /// Increments the generation counter.
    pub fn deselect<'a>(&self, keys: impl IntoIterator<Item = &'a K>)
    where
        K: 'a,
    {
        self.inner.lock().expect("poisoned lock").deselect(keys);
    }

    /// Replace the entire selection with the given keys.
    ///
    /// Increments the generation counter.
    pub fn set(&self, keys: impl IntoIterator<Item = K>) {
        self.inner.lock().expect("poisoned lock").set(keys);
    }

    /// Clear all selected keys.
    ///
    /// Increments the generation counter.
    pub fn clear(&self) {
        self.inner.lock().expect("poisoned lock").clear();
    }

    /// Returns a snapshot of the currently selected keys.
    ///
    /// This clones the keys out of the locked state, so the lock is held
    /// only briefly.
    pub fn selected_keys(&self) -> Vec<K>
    where
        K: Clone,
    {
        self.inner
            .lock()
            .expect("poisoned lock")
            .selected_keys()
            .cloned()
            .collect()
    }

    /// Execute a closure with read access to the inner state.
    ///
    /// The lock is held for the duration of `f`. Keep `f` short.
    pub fn with_state<R>(&self, f: impl FnOnce(&KeyedSelectionState<K>) -> R) -> R {
        let guard = self.inner.lock().expect("poisoned lock");
        f(&guard)
    }

    /// Execute a closure with mutable access to the inner state.
    ///
    /// The lock is held for the duration of `f`. Keep `f` short.
    pub fn with_state_mut<R>(&self, f: impl FnOnce(&mut KeyedSelectionState<K>) -> R) -> R {
        let mut guard = self.inner.lock().expect("poisoned lock");
        f(&mut guard)
    }
}

// ---------------------------------------------------------------------------
// DimInstance — trait for alpha modification
// ---------------------------------------------------------------------------

/// Trait for mark instance types that support alpha-channel dimming.
///
/// Implement this for each GPU instance struct so that
/// [`build_dimmed_instances`] can reduce the opacity of unselected items
/// without modifying shader code.
///
/// Types that also support GPU-side dimming via [`SelectionMaskBuffer`]
/// should override [`alpha_offsets`](Self::alpha_offsets) to return the
/// corresponding [`AlphaOffsets`].
pub trait DimInstance {
    /// Multiply the instance's alpha channel(s) by `factor`.
    ///
    /// Typically this means `fill_color[3] *= factor` and, if present,
    /// `stroke_color[3] *= factor`.
    fn dim_alpha(&mut self, factor: f32);

    /// Returns [`AlphaOffsets`] for GPU-side dimming via
    /// [`SelectionMaskBuffer`], or `None` if only CPU dimming is supported.
    ///
    /// The default implementation returns `None`.  Built-in mark instance
    /// types override this to enable automatic GPU dimming in
    /// [`LinkedSelection`].
    fn alpha_offsets() -> Option<crate::selection_mask::AlphaOffsets> {
        None
    }
}

// ---------------------------------------------------------------------------
// build_dimmed_instances — helper for preparing instance data
// ---------------------------------------------------------------------------

/// Build a `Vec<I>` of mark instances with selection-based dimming applied.
///
/// For each data item in `data`:
/// 1. Call `mapper` to produce the base instance.
/// 2. Call `key_fn` to obtain the item's cross-chart identity key.
/// 3. If the shared selection is non-empty and the key is **not** selected,
///    call [`DimInstance::dim_alpha`] with `dim_opacity`.
///
/// If the shared selection is empty (no items selected), all instances are
/// returned at full opacity.
///
/// # Arguments
///
/// - `data` — Slice of data items.
/// - `mapper` — Converts a data item to its GPU instance representation.
/// - `key_fn` — Extracts the cross-chart identity key from a data item.
///   Receives both a reference to the item and its index in the slice.
/// - `state` — The shared selection state to read from.
/// - `dim_opacity` — The opacity factor applied to unselected items (e.g. 0.2).
pub fn build_dimmed_instances<T, I, K>(
    data: &[T],
    mapper: impl Fn(&T) -> I,
    key_fn: impl Fn(&T, usize) -> K,
    state: &SharedSelectionState<K>,
    dim_opacity: f32,
) -> Vec<I>
where
    I: DimInstance,
    K: Hash + Eq + Send + Sync + 'static,
{
    let guard = state.inner.lock().expect("poisoned lock");
    let has_selection = !guard.is_empty();

    data.iter()
        .enumerate()
        .map(|(idx, item)| {
            let mut inst = mapper(item);
            if has_selection {
                let key = key_fn(item, idx);
                if !guard.is_selected(&key) {
                    inst.dim_alpha(dim_opacity);
                }
            }
            inst
        })
        .collect()
}

/// Check whether the shared state has changed since a given generation.
///
/// Returns `Some(current_generation)` if the state has changed, or `None`
/// if it has not. This is a non-blocking check using `try_lock`.
pub fn has_changed_since<K>(state: &SharedSelectionState<K>, last_gen: u64) -> Option<u64>
where
    K: Hash + Eq + Send + Sync + 'static,
{
    state.try_generation().and_then(|current_gen| {
        if current_gen > last_gen {
            Some(current_gen)
        } else {
            None
        }
    })
}

// ---------------------------------------------------------------------------
// DimInstance implementations for built-in marks
// ---------------------------------------------------------------------------

use crate::mark::circle::CircleInstance;
use crate::mark::line::LineInstance;
use crate::mark::rectangle::RectangleInstance;

impl DimInstance for CircleInstance {
    fn dim_alpha(&mut self, factor: f32) {
        self.fill_color[3] *= factor;
        self.stroke_color[3] *= factor;
    }

    fn alpha_offsets() -> Option<crate::selection_mask::AlphaOffsets> {
        Some(crate::selection_mask::AlphaOffsets::for_circle())
    }
}

impl DimInstance for RectangleInstance {
    fn dim_alpha(&mut self, factor: f32) {
        self.fill_color[3] *= factor;
        self.stroke_color[3] *= factor;
    }

    fn alpha_offsets() -> Option<crate::selection_mask::AlphaOffsets> {
        Some(crate::selection_mask::AlphaOffsets::for_rectangle())
    }
}

impl DimInstance for LineInstance {
    fn dim_alpha(&mut self, factor: f32) {
        self.color[3] *= factor;
    }

    fn alpha_offsets() -> Option<crate::selection_mask::AlphaOffsets> {
        Some(crate::selection_mask::AlphaOffsets::for_line())
    }
}

// BoxPlot support (if available)
use crate::mark::boxplot::BoxPlotInstance;

impl DimInstance for BoxPlotInstance {
    fn dim_alpha(&mut self, factor: f32) {
        self.box_fill_color[3] *= factor;
        self.box_stroke_color[3] *= factor;
        self.median_color[3] *= factor;
        self.whisker_color[3] *= factor;
        self.outlier_color[3] *= factor;
    }

    fn alpha_offsets() -> Option<crate::selection_mask::AlphaOffsets> {
        Some(crate::selection_mask::AlphaOffsets::for_boxplot())
    }
}

// ---------------------------------------------------------------------------
// LinkedSelection — wrapper combining Selection + SharedSelectionState
// ---------------------------------------------------------------------------

use crate::buffer::BufferPool;
use crate::error::GupResult;
use crate::pipeline_cache::PipelineCache;
use crate::selection::{Mark, Selection};
use crate::selection_mask::SelectionMaskBuffer;
use wgpu::{Device, Queue, RenderPass};

/// A wrapper that combines a [`Selection`] with a [`SharedSelectionState`]
/// and a key function, providing automatic generation-based change detection
/// and instance rebuild with dimming.
///
/// `LinkedSelection` eliminates the manual orchestration that was previously
/// required when using [`build_dimmed_instances`] and [`has_changed_since`]
/// directly.  Instead of tracking generation counters yourself, call
/// [`prepare_render`](Self::prepare_render) on each frame and the wrapper
/// takes care of the rest: it checks whether the shared selection state has
/// changed and only rebuilds the dimmed instance buffer when necessary.
///
/// # GPU Dimming
///
/// When the instance count exceeds [`gpu_dimming_threshold`](Self::gpu_dimming_threshold)
/// (default: 10 000) and the instance type provides
/// [`alpha_offsets`](DimInstance::alpha_offsets), dimming is performed on the
/// GPU via a [`SelectionMaskBuffer`] compute shader instead of the CPU-side
/// [`build_dimmed_instances`] path.  The transition is transparent to the
/// caller.
///
/// # Type Parameters
///
/// - `T` — The data item type stored in the inner [`Selection`].
/// - `M` — The mark type (e.g. [`Circle`](crate::Circle),
///   [`Rectangle`](crate::Rectangle)).
/// - `K` — The cross-chart identity key type (must be `Hash + Eq + Send +
///   Sync + 'static`).
///
/// # Examples
///
/// ```rust,no_run
/// use gup::linked_selection::{LinkedSelection, SharedSelectionState};
/// use gup::mark::circle::CircleInstance;
/// use gup::Circle;
///
/// let shared = SharedSelectionState::<usize>::new();
/// let data = vec![1.0f32, 2.0, 3.0];
///
/// let mut linked: LinkedSelection<f32, Circle, usize> =
///     LinkedSelection::new(data, shared.clone(), |_item, idx| idx)
///         .dim_opacity(0.2);
/// ```
pub struct LinkedSelection<T, M: Mark, K: Hash + Eq + Send + Sync + 'static> {
    selection: Selection<T, M>,
    shared_state: SharedSelectionState<K>,
    key_fn: Box<dyn Fn(&T, usize) -> K>,
    dim_opacity: f32,
    last_generation: u64,
    /// Instance count threshold above which the GPU dimming path is used.
    gpu_threshold: u32,
    /// GPU-side selection mask buffer (lazily initialised on the GPU path).
    mask_buffer: Option<SelectionMaskBuffer>,
    /// GPU buffer holding undimmed (source) instances for the compute shader.
    source_buffer: Option<wgpu::Buffer>,
}

impl<T, M: Mark, K: Hash + Eq + Send + Sync + 'static> LinkedSelection<T, M, K> {
    /// Create a new `LinkedSelection` from data, a shared selection state,
    /// and a key function that maps each data item to its cross-chart
    /// identity key.
    ///
    /// The default dim opacity is `0.2` (20% of original alpha for
    /// unselected items).  Use [`dim_opacity`](Self::dim_opacity) to
    /// customise this.
    ///
    /// # Arguments
    ///
    /// - `data` — The data items to render.
    /// - `shared_state` — The shared selection state to observe.
    /// - `key_fn` — A function that takes a reference to a data item and
    ///   its index and returns the cross-chart identity key.
    pub fn new(
        data: Vec<T>,
        shared_state: SharedSelectionState<K>,
        key_fn: impl Fn(&T, usize) -> K + 'static,
    ) -> Self {
        Self {
            selection: Selection::from_data(data),
            shared_state,
            key_fn: Box::new(key_fn),
            dim_opacity: 0.2,
            last_generation: 0,
            gpu_threshold: 10_000,
            mask_buffer: None,
            source_buffer: None,
        }
    }

    /// Wrap an existing [`Selection`] with linked-view state.
    ///
    /// Use this when you already have a `Selection` (e.g. one created with
    /// a [`RenderContext`](crate::RenderContext) for interaction support)
    /// and want to add linked-view dimming.
    pub fn from_selection(
        selection: Selection<T, M>,
        shared_state: SharedSelectionState<K>,
        key_fn: impl Fn(&T, usize) -> K + 'static,
    ) -> Self {
        Self {
            selection,
            shared_state,
            key_fn: Box::new(key_fn),
            dim_opacity: 0.2,
            last_generation: 0,
            gpu_threshold: 10_000,
            mask_buffer: None,
            source_buffer: None,
        }
    }

    /// Set the opacity factor applied to unselected items (default `0.2`).
    ///
    /// A value of `0.0` makes unselected items fully transparent; `1.0`
    /// disables dimming entirely.
    #[must_use]
    pub fn dim_opacity(mut self, opacity: f32) -> Self {
        self.dim_opacity = opacity;
        self
    }

    /// Set the instance count threshold for GPU-side dimming (default 10 000).
    ///
    /// When the number of instances in
    /// [`prepare_render`](Self::prepare_render) meets or exceeds this
    /// threshold **and** the instance type provides
    /// [`DimInstance::alpha_offsets`], dimming is performed entirely on the
    /// GPU via a compute shader.  Below this threshold the CPU-based
    /// [`build_dimmed_instances`] path is used instead.
    ///
    /// Set to `u32::MAX` to force the CPU path regardless of dataset size.
    /// Set to `0` to always use the GPU path (useful for testing).
    #[must_use]
    pub fn gpu_dimming_threshold(mut self, threshold: u32) -> Self {
        self.gpu_threshold = threshold;
        self
    }

    /// Returns the current GPU dimming threshold.
    pub fn gpu_threshold(&self) -> u32 {
        self.gpu_threshold
    }

    /// Prepare GPU resources for rendering, automatically rebuilding the
    /// dimmed instance buffer when the shared selection state has changed.
    ///
    /// On the first call this always creates GPU resources.  On subsequent
    /// calls it checks the generation counter of the shared state and only
    /// rebuilds when a change is detected, or when the inner selection has
    /// no render state (e.g. after [`set_data`](Self::set_data)).
    ///
    /// When the instance count meets or exceeds
    /// [`gpu_dimming_threshold`](Self::gpu_dimming_threshold) and the
    /// instance type supports GPU dimming (via [`DimInstance::alpha_offsets`]),
    /// a compute shader is used instead of CPU-side dimming.  The transition
    /// is transparent.
    ///
    /// The `mapper` closure converts each data item `T` into a GPU-ready
    /// instance struct `I`.  The wrapper applies selection-based dimming on
    /// top of the mapper output.
    ///
    /// # Errors
    ///
    /// Returns an error if pipeline or buffer creation fails.
    pub fn prepare_render<I>(
        &mut self,
        device: &Device,
        queue: &Queue,
        mapper: impl Fn(&T) -> I,
        cache: Option<&mut PipelineCache>,
        pool: Option<&mut BufferPool>,
    ) -> GupResult<()>
    where
        I: DimInstance + bytemuck::Pod + bytemuck::Zeroable,
    {
        let data_changed = !self.selection.is_render_ready();
        let selection_changed = match has_changed_since(&self.shared_state, self.last_generation) {
            Some(new_gen) => {
                self.last_generation = new_gen;
                true
            }
            None => false,
        };

        let needs_rebuild = data_changed || selection_changed;
        if !needs_rebuild {
            return Ok(());
        }

        let instance_count = self.selection.data().len();
        let use_gpu = instance_count as u32 >= self.gpu_threshold && I::alpha_offsets().is_some();

        if use_gpu {
            self.prepare_render_gpu(
                device,
                queue,
                &mapper,
                instance_count,
                data_changed,
                cache,
                pool,
            )
        } else {
            // Drop GPU resources when falling back to CPU path.
            self.mask_buffer = None;
            self.source_buffer = None;

            let instances = build_dimmed_instances(
                self.selection.data(),
                mapper,
                &*self.key_fn,
                &self.shared_state,
                self.dim_opacity,
            );
            self.selection
                .prepare_render_raw(device, queue, &instances, cache, pool)
        }
    }

    /// GPU dimming path: upload undimmed instances, run the compute shader,
    /// then copy the dimmed output into the Selection's instance buffer.
    fn prepare_render_gpu<I>(
        &mut self,
        device: &Device,
        queue: &Queue,
        mapper: &dyn Fn(&T) -> I,
        instance_count: usize,
        data_changed: bool,
        cache: Option<&mut PipelineCache>,
        pool: Option<&mut BufferPool>,
    ) -> GupResult<()>
    where
        I: DimInstance + bytemuck::Pod + bytemuck::Zeroable,
    {
        let alpha_offsets =
            I::alpha_offsets().expect("GPU path requires DimInstance::alpha_offsets");
        let count = instance_count as u32;

        // -- 1. Rebuild undimmed instances & source buffer when data changed --
        if data_changed {
            let instances: Vec<I> = self.selection.data().iter().map(mapper).collect();

            // Set up the render pipeline (vertex buffers, bind group, etc.)
            // by uploading the undimmed instances.
            self.selection
                .prepare_render_raw(device, queue, &instances, cache, pool)?;

            // Create / resize the source buffer with undimmed instance data.
            let instance_bytes: &[u8] = bytemuck::cast_slice(&instances);
            let src_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("linked_selection_source"),
                size: instance_bytes.len() as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&src_buf, 0, instance_bytes);
            self.source_buffer = Some(src_buf);

            // (Re)create the mask buffer for the new capacity.
            self.mask_buffer = Some(SelectionMaskBuffer::new(device, count, &alpha_offsets)?);
        }

        // -- 2. Update mask & dispatch dimming compute shader ---------------
        let mask = self.mask_buffer.as_mut().expect("mask_buffer initialised");
        mask.ensure_capacity(device, count);

        let source = self
            .source_buffer
            .as_ref()
            .expect("source_buffer initialised");

        // Force a mask update when data changed (even if generation hasn't
        // advanced since the mask buffer was just recreated).
        let mask_changed = if data_changed {
            // Reset mask generation so update_mask always runs.
            // We do this by calling update_mask which will see a generation
            // difference because we set last_generation on the mask to 0
            // when it was recreated above.
            mask.update_mask(
                queue,
                self.selection.data(),
                &*self.key_fn,
                &self.shared_state,
            )
        } else {
            mask.update_mask(
                queue,
                self.selection.data(),
                &*self.key_fn,
                &self.shared_state,
            )
        };

        // If mask or data changed, run the compute shader and copy result.
        if mask_changed || data_changed {
            let instance_byte_size = (std::mem::size_of::<I>() * instance_count) as u64;

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("linked_selection_dim_encoder"),
            });

            // Encode the dimming compute pass.
            mask.encode_dimming(device, queue, &mut encoder, source, count, self.dim_opacity);

            // Copy the dimmed output into the Selection's instance buffer.
            let dst = self
                .selection
                .instance_buffer()
                .expect("render state initialised");
            encoder.copy_buffer_to_buffer(mask.output_buffer(), 0, dst, 0, instance_byte_size);

            queue.submit([encoder.finish()]);
        }

        Ok(())
    }

    /// Render to an active render pass.
    ///
    /// Delegates to the inner [`Selection::render`].  You must call
    /// [`prepare_render`](Self::prepare_render) at least once before the
    /// first render call.
    pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) -> GupResult<()> {
        self.selection.render(render_pass)
    }

    /// Returns `true` if the inner selection has been prepared for rendering.
    pub fn is_render_ready(&self) -> bool {
        self.selection.is_render_ready()
    }

    /// Get a reference to the underlying data.
    pub fn data(&self) -> &[T] {
        self.selection.data()
    }

    /// Get a reference to the inner selection.
    pub fn selection(&self) -> &Selection<T, M> {
        &self.selection
    }

    /// Get a mutable reference to the inner selection.
    pub fn selection_mut(&mut self) -> &mut Selection<T, M> {
        &mut self.selection
    }

    /// Get a reference to the shared selection state.
    pub fn shared_state(&self) -> &SharedSelectionState<K> {
        &self.shared_state
    }

    /// Replace the data in this linked selection.
    ///
    /// This invalidates the GPU render state; the next call to
    /// [`prepare_render`](Self::prepare_render) will rebuild everything.
    pub fn set_data(&mut self, data: Vec<T>) {
        self.selection.set_data(data);
        // Invalidate GPU dimming resources — they will be recreated on the
        // next prepare_render call.
        self.mask_buffer = None;
        self.source_buffer = None;
    }

    /// Returns the generation counter value that was last observed by
    /// [`prepare_render`](Self::prepare_render).
    pub fn last_generation(&self) -> u64 {
        self.last_generation
    }

    /// Returns `true` if the GPU dimming path is currently active.
    ///
    /// This is `true` after [`prepare_render`](Self::prepare_render) has
    /// been called with an instance count at or above the GPU threshold.
    pub fn is_gpu_dimming_active(&self) -> bool {
        self.mask_buffer.is_some()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- KeyedSelectionState unit tests --

    #[test]
    fn test_new_state_is_empty() {
        let state = KeyedSelectionState::<usize>::new();
        assert!(state.is_empty());
        assert_eq!(state.selected_count(), 0);
        assert_eq!(state.generation(), 0);
    }

    #[test]
    fn test_select_adds_keys() {
        let mut state = KeyedSelectionState::new();
        state.select([1, 2, 3]);
        assert!(state.is_selected(&1));
        assert!(state.is_selected(&2));
        assert!(state.is_selected(&3));
        assert!(!state.is_selected(&4));
        assert_eq!(state.selected_count(), 3);
    }

    #[test]
    fn test_deselect_removes_keys() {
        let mut state = KeyedSelectionState::new();
        state.select([1, 2, 3, 4, 5]);
        state.deselect(&[2, 4]);
        assert!(state.is_selected(&1));
        assert!(!state.is_selected(&2));
        assert!(state.is_selected(&3));
        assert!(!state.is_selected(&4));
        assert!(state.is_selected(&5));
        assert_eq!(state.selected_count(), 3);
    }

    #[test]
    fn test_clear_removes_all() {
        let mut state = KeyedSelectionState::new();
        state.select([1, 2, 3]);
        assert_eq!(state.selected_count(), 3);
        state.clear();
        assert!(state.is_empty());
        assert_eq!(state.selected_count(), 0);
    }

    #[test]
    fn test_set_replaces_selection() {
        let mut state = KeyedSelectionState::new();
        state.select([1, 2, 3]);
        state.set([10, 20]);
        assert!(!state.is_selected(&1));
        assert!(state.is_selected(&10));
        assert!(state.is_selected(&20));
        assert_eq!(state.selected_count(), 2);
    }

    #[test]
    fn test_generation_increments_on_every_mutation() {
        let mut state = KeyedSelectionState::<usize>::new();
        assert_eq!(state.generation(), 0);

        state.select([1]);
        assert_eq!(state.generation(), 1);

        state.select([2, 3]);
        assert_eq!(state.generation(), 2);

        state.deselect(&[1]);
        assert_eq!(state.generation(), 3);

        state.set([10]);
        assert_eq!(state.generation(), 4);

        state.clear();
        assert_eq!(state.generation(), 5);
    }

    #[test]
    fn test_selected_keys_iterator() {
        let mut state = KeyedSelectionState::new();
        state.select([3, 1, 4, 1, 5]); // duplicates should be deduped
        let mut keys: Vec<_> = state.selected_keys().copied().collect();
        keys.sort();
        assert_eq!(keys, vec![1, 3, 4, 5]);
    }

    #[test]
    fn test_deselect_nonexistent_key_still_increments() {
        let mut state = KeyedSelectionState::new();
        state.select([1]);
        let gen_before = state.generation();
        state.deselect(&[999]); // doesn't exist
        assert_eq!(state.generation(), gen_before + 1);
    }

    // -- SharedSelectionState unit tests --

    #[test]
    fn test_shared_clone_shares_data() {
        let shared = SharedSelectionState::<usize>::new();
        let shared2 = shared.clone();

        shared.select([42, 99]);
        assert!(shared2.is_selected(&42));
        assert!(shared2.is_selected(&99));
        assert_eq!(shared2.selected_count(), 2);
    }

    #[test]
    fn test_shared_generation_propagates() {
        let shared = SharedSelectionState::<usize>::new();
        let shared2 = shared.clone();

        assert_eq!(shared2.generation(), 0);
        shared.select([1]);
        assert_eq!(shared2.generation(), 1);
        shared.clear();
        assert_eq!(shared2.generation(), 2);
    }

    #[test]
    fn test_shared_clear_propagates() {
        let shared = SharedSelectionState::<usize>::new();
        let shared2 = shared.clone();

        shared.select([1, 2, 3]);
        assert_eq!(shared2.selected_count(), 3);

        shared.clear();
        assert!(shared2.is_empty());
        assert!(!shared2.is_selected(&1));
    }

    #[test]
    fn test_shared_selected_keys_snapshot() {
        let shared = SharedSelectionState::<usize>::new();
        shared.select([5, 10, 15]);

        let mut keys = shared.selected_keys();
        keys.sort();
        assert_eq!(keys, vec![5, 10, 15]);
    }

    #[test]
    fn test_shared_with_state() {
        let shared = SharedSelectionState::<usize>::new();
        shared.select([1, 2, 3]);

        let count = shared.with_state(|s| s.selected_count());
        assert_eq!(count, 3);
    }

    #[test]
    fn test_shared_with_state_mut() {
        let shared = SharedSelectionState::<usize>::new();
        shared.with_state_mut(|s| s.select([7, 8, 9]));
        assert_eq!(shared.selected_count(), 3);
    }

    #[test]
    fn test_shared_try_generation() {
        let shared = SharedSelectionState::<usize>::new();
        shared.select([1]);
        assert_eq!(shared.try_generation(), Some(1));
    }

    #[test]
    fn test_shared_debug() {
        let shared = SharedSelectionState::<usize>::new();
        shared.select([1]);
        let debug = format!("{:?}", shared);
        assert!(debug.contains("SharedSelectionState"));
    }

    #[test]
    fn test_shared_default() {
        let shared = SharedSelectionState::<String>::default();
        assert!(shared.is_empty());
    }

    #[test]
    fn test_shared_string_keys() {
        let shared = SharedSelectionState::<String>::new();
        shared.select(["alice".to_string(), "bob".to_string()]);
        assert!(shared.is_selected(&"alice".to_string()));
        assert!(!shared.is_selected(&"carol".to_string()));
    }

    // -- has_changed_since tests --

    #[test]
    fn test_has_changed_since_detects_change() {
        let shared = SharedSelectionState::<usize>::new();
        assert_eq!(has_changed_since(&shared, 0), None); // gen 0, last 0

        shared.select([1]);
        assert_eq!(has_changed_since(&shared, 0), Some(1));
        assert_eq!(has_changed_since(&shared, 1), None);
    }

    // -- DimInstance tests --

    #[test]
    fn test_circle_dim_instance() {
        let mut inst = CircleInstance {
            center: [0.0, 0.0],
            radius: 1.0,
            _pad0: 0.0,
            fill_color: [1.0, 0.0, 0.0, 0.8],
            stroke_width: 1.0,
            _pad1: [0.0; 3],
            stroke_color: [0.0, 0.0, 0.0, 1.0],
        };
        inst.dim_alpha(0.25);
        assert!((inst.fill_color[3] - 0.2).abs() < f32::EPSILON);
        assert!((inst.stroke_color[3] - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_rectangle_dim_instance() {
        let mut inst = RectangleInstance {
            center: [0.0, 0.0],
            size: [1.0, 1.0],
            fill_color: [0.0, 1.0, 0.0, 1.0],
            stroke_width: 0.0,
            _pad1: [0.0; 3],
            stroke_color: [0.0, 0.0, 0.0, 0.5],
            corner_radius: 0.0,
            _padding: 0.0,
            _pad2: [0.0; 2],
        };
        inst.dim_alpha(0.5);
        assert!((inst.fill_color[3] - 0.5).abs() < f32::EPSILON);
        assert!((inst.stroke_color[3] - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_line_dim_instance() {
        let mut inst = LineInstance {
            start: [0.0, 0.0],
            end: [1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            width: 2.0,
            style: 0,
            _padding: [0.0; 2],
        };
        inst.dim_alpha(0.3);
        assert!((inst.color[3] - 0.3).abs() < f32::EPSILON);
    }

    // -- build_dimmed_instances tests --

    #[test]
    fn test_build_dimmed_no_selection() {
        let state = SharedSelectionState::<usize>::new();
        let data = vec![1.0f32, 2.0, 3.0];
        let instances = build_dimmed_instances(
            &data,
            |d| CircleInstance {
                center: [*d, 0.0],
                radius: 0.05,
                _pad0: 0.0,
                fill_color: [1.0, 0.0, 0.0, 1.0],
                stroke_width: 0.0,
                _pad1: [0.0; 3],
                stroke_color: [0.0; 4],
            },
            |_d, idx| idx,
            &state,
            0.2,
        );
        // No selection → all full opacity
        for inst in &instances {
            assert!((inst.fill_color[3] - 1.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_build_dimmed_with_selection() {
        let state = SharedSelectionState::<usize>::new();
        state.select([0, 2]);

        let data = vec![10.0f32, 20.0, 30.0, 40.0];
        let instances = build_dimmed_instances(
            &data,
            |d| CircleInstance {
                center: [*d, 0.0],
                radius: 0.05,
                _pad0: 0.0,
                fill_color: [1.0, 0.0, 0.0, 1.0],
                stroke_width: 0.0,
                _pad1: [0.0; 3],
                stroke_color: [0.0; 4],
            },
            |_d, idx| idx,
            &state,
            0.2,
        );
        // Selected items: full opacity
        assert!((instances[0].fill_color[3] - 1.0).abs() < f32::EPSILON);
        assert!((instances[2].fill_color[3] - 1.0).abs() < f32::EPSILON);
        // Unselected items: dimmed
        assert!((instances[1].fill_color[3] - 0.2).abs() < f32::EPSILON);
        assert!((instances[3].fill_color[3] - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn test_build_dimmed_clear_restores_full_opacity() {
        let state = SharedSelectionState::<usize>::new();
        state.select([0]);

        let data = vec![1.0f32, 2.0];
        let make_instances = || {
            build_dimmed_instances(
                &data,
                |d| CircleInstance {
                    center: [*d, 0.0],
                    radius: 0.05,
                    _pad0: 0.0,
                    fill_color: [1.0, 0.0, 0.0, 1.0],
                    stroke_width: 0.0,
                    _pad1: [0.0; 3],
                    stroke_color: [0.0; 4],
                },
                |_d, idx| idx,
                &state,
                0.2,
            )
        };

        // With selection: item 1 is dimmed
        let instances = make_instances();
        assert!((instances[1].fill_color[3] - 0.2).abs() < f32::EPSILON);

        // Clear selection: item 1 returns to full opacity
        state.clear();
        let instances = make_instances();
        assert!((instances[1].fill_color[3] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_build_dimmed_with_string_keys() {
        let state = SharedSelectionState::<String>::new();
        state.select(["alice".to_string()]);

        #[derive(Debug)]
        struct Person {
            name: String,
        }

        let data = vec![
            Person {
                name: "alice".into(),
            },
            Person { name: "bob".into() },
        ];

        let instances = build_dimmed_instances(
            &data,
            |_p| CircleInstance {
                center: [0.0, 0.0],
                radius: 0.05,
                _pad0: 0.0,
                fill_color: [1.0, 0.0, 0.0, 1.0],
                stroke_width: 0.0,
                _pad1: [0.0; 3],
                stroke_color: [0.0; 4],
            },
            |p, _idx| p.name.clone(),
            &state,
            0.2,
        );

        // Alice is selected → full opacity
        assert!((instances[0].fill_color[3] - 1.0).abs() < f32::EPSILON);
        // Bob is not selected → dimmed
        assert!((instances[1].fill_color[3] - 0.2).abs() < f32::EPSILON);
    }

    // -- LinkedSelection unit tests --

    #[test]
    fn linked_selection_new_creates_wrapper() {
        let shared = SharedSelectionState::<usize>::new();
        let data = vec![1.0f32, 2.0, 3.0];

        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(data, shared.clone(), |_item, idx| idx);

        assert_eq!(linked.data().len(), 3);
        assert!(!linked.is_render_ready());
        assert_eq!(linked.last_generation(), 0);
    }

    #[test]
    fn linked_selection_builder_dim_opacity() {
        let shared = SharedSelectionState::<usize>::new();

        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx).dim_opacity(0.5);

        // Just verify it builds without error; dim_opacity is tested
        // via prepare_render in the GPU integration tests.
        assert_eq!(linked.data().len(), 1);
    }

    #[test]
    fn linked_selection_from_selection() {
        let shared = SharedSelectionState::<usize>::new();
        let sel: Selection<f32, crate::Circle> = Selection::from_data(vec![1.0, 2.0]);

        let linked = LinkedSelection::from_selection(sel, shared, |_item, idx| idx);

        assert_eq!(linked.data().len(), 2);
        assert!(!linked.is_render_ready());
    }

    #[test]
    fn linked_selection_set_data() {
        let shared = SharedSelectionState::<usize>::new();
        let mut linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx);

        linked.set_data(vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(linked.data().len(), 4);
    }

    #[test]
    fn linked_selection_accessors() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0, 2.0], shared.clone(), |_item, idx| idx);

        // shared_state returns a reference to the same shared state
        shared.select([42usize]);
        assert!(linked.shared_state().is_selected(&42));

        // selection() and selection_mut() provide access
        assert_eq!(linked.selection().data().len(), 2);
    }

    #[test]
    fn linked_selection_last_generation_tracks_state() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared.clone(), |_item, idx| idx);

        assert_eq!(linked.last_generation(), 0);
        // last_generation only updates after prepare_render, which
        // requires GPU resources.  We verify the initial value here.
    }
}
