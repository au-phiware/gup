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
use std::time::Instant;

use crate::gpu_timer::GpuTimer;

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

// ---------------------------------------------------------------------------
// AutoTuneState — calibration state machine
// ---------------------------------------------------------------------------

/// Phase of the auto-tune calibration state machine.
///
/// The calibration proceeds through three phases:
/// 1. **ProbeCpu** — time the CPU dimming path for `calibration_frames` frames.
/// 2. **ProbeGpu** — time the GPU dimming path for `calibration_frames` frames.
/// 3. **Settled** — calibration complete; the effective threshold is fixed.
///
/// If the dataset size changes significantly after settling, calibration
/// restarts from `ProbeCpu`.
#[derive(Debug, Clone)]
enum AutoTunePhase {
    /// Measuring CPU path performance.
    ProbeCpu { remaining: u32, total_ns: u128 },
    /// Measuring GPU path performance (carries forward the CPU total).
    ProbeGpu {
        remaining: u32,
        total_ns: u128,
        cpu_total_ns: u128,
    },
    /// Calibration complete.
    Settled {
        /// Mean CPU time per frame in nanoseconds.
        cpu_mean_ns: u128,
        /// Mean GPU time per frame in nanoseconds.
        gpu_mean_ns: u128,
    },
}

/// Auto-tune state for adaptive CPU/GPU threshold selection.
///
/// When enabled, the auto-tune system profiles both the CPU and GPU dimming
/// paths during an initial calibration phase and then sets the effective
/// threshold based on which path is faster for the current dataset size.
#[derive(Debug, Clone)]
struct AutoTuneState {
    /// Whether auto-tune is enabled.
    enabled: bool,
    /// Number of frames to sample each path during calibration.
    calibration_frames: u32,
    /// Current calibration phase.
    phase: AutoTunePhase,
    /// The instance count at the time of the last calibration.
    calibrated_instance_count: u32,
    /// The effective threshold determined by calibration (or the initial
    /// estimate if calibration has not yet completed).
    effective_threshold: u32,
}

impl AutoTuneState {
    /// Create a new disabled auto-tune state.
    fn new(initial_threshold: u32) -> Self {
        Self {
            enabled: false,
            calibration_frames: 5,
            phase: AutoTunePhase::ProbeCpu {
                remaining: 5,
                total_ns: 0,
            },
            calibrated_instance_count: 0,
            effective_threshold: initial_threshold,
        }
    }

    /// Reset calibration to the beginning (ProbeCpu).
    fn reset(&mut self) {
        let frames = self.calibration_frames;
        self.phase = AutoTunePhase::ProbeCpu {
            remaining: frames,
            total_ns: 0,
        };
    }

    /// Returns the effective threshold. When auto-tune is disabled this
    /// returns the initial threshold set via `gpu_dimming_threshold`.
    fn effective_threshold(&self) -> u32 {
        self.effective_threshold
    }

    /// Returns `true` if calibration has settled.
    fn is_settled(&self) -> bool {
        matches!(self.phase, AutoTunePhase::Settled { .. })
    }

    /// Returns `true` if we should force the CPU path this frame
    /// (for calibration purposes).
    fn force_cpu(&self) -> bool {
        self.enabled && matches!(self.phase, AutoTunePhase::ProbeCpu { .. })
    }

    /// Returns `true` if we should force the GPU path this frame
    /// (for calibration purposes).
    fn force_gpu(&self) -> bool {
        self.enabled && matches!(self.phase, AutoTunePhase::ProbeGpu { .. })
    }

    /// Record a timing sample and advance the calibration state machine.
    ///
    /// `elapsed_ns` is the wall-clock time of the `prepare_render` call
    /// for this frame. `instance_count` is the current dataset size.
    fn record_sample(&mut self, elapsed_ns: u128, instance_count: u32) {
        if !self.enabled {
            return;
        }

        match &mut self.phase {
            AutoTunePhase::ProbeCpu {
                remaining,
                total_ns,
            } => {
                *total_ns += elapsed_ns;
                *remaining = remaining.saturating_sub(1);
                if *remaining == 0 {
                    let cpu_total = *total_ns;
                    let frames = self.calibration_frames;
                    self.phase = AutoTunePhase::ProbeGpu {
                        remaining: frames,
                        total_ns: 0,
                        cpu_total_ns: cpu_total,
                    };
                }
            }
            AutoTunePhase::ProbeGpu {
                remaining,
                total_ns,
                cpu_total_ns,
            } => {
                *total_ns += elapsed_ns;
                *remaining = remaining.saturating_sub(1);
                if *remaining == 0 {
                    let frames = self.calibration_frames as u128;
                    let cpu_mean = *cpu_total_ns / frames;
                    let gpu_mean = *total_ns / frames;

                    // Set the effective threshold based on which path was
                    // faster.  If GPU is faster (or equal), set the
                    // threshold to the current instance count so the GPU
                    // path activates for this size and larger.  If CPU is
                    // faster, set it above the current count so we stay on
                    // the CPU path.
                    self.effective_threshold = if gpu_mean <= cpu_mean {
                        instance_count
                    } else {
                        instance_count.saturating_add(1)
                    };

                    self.calibrated_instance_count = instance_count;
                    self.phase = AutoTunePhase::Settled {
                        cpu_mean_ns: cpu_mean,
                        gpu_mean_ns: gpu_mean,
                    };
                }
            }
            AutoTunePhase::Settled { .. } => {
                // Already settled — nothing to do.
            }
        }
    }

    /// Check whether re-calibration is needed because the instance count
    /// has changed significantly since the last calibration.
    ///
    /// A change of more than 50% triggers re-calibration.
    fn maybe_recalibrate(&mut self, instance_count: u32) {
        if !self.enabled || !self.is_settled() {
            return;
        }
        let prev = self.calibrated_instance_count;
        if prev == 0 {
            // First calibration — nothing to compare against.
            return;
        }
        let diff = (instance_count as i64 - prev as i64).unsigned_abs();
        if diff > (prev as u64) / 2 {
            self.reset();
        }
    }
}

// ---------------------------------------------------------------------------
// DimInstance implementations for built-in marks (continued)
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
/// # Adaptive Auto-Tune
///
/// Enable [`gpu_dimming_auto_tune`](Self::gpu_dimming_auto_tune) to let the
/// system profile both CPU and GPU paths during an initial calibration
/// phase and automatically select the faster one.  The static
/// [`gpu_dimming_threshold`](Self::gpu_dimming_threshold) serves as the
/// initial estimate until calibration completes.  Use
/// [`effective_threshold`](Self::effective_threshold) to read the current
/// threshold after calibration.
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
    /// Auto-tune state machine for adaptive threshold selection.
    auto_tune: AutoTuneState,
    /// GPU timestamp timer (lazily created when auto-tune needs precise
    /// GPU-side timing and `Features::TIMESTAMP_QUERY` is available).
    gpu_timer: Option<GpuTimer>,
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
        let gpu_threshold = 10_000;
        Self {
            selection: Selection::from_data(data),
            shared_state,
            key_fn: Box::new(key_fn),
            dim_opacity: 0.2,
            last_generation: 0,
            gpu_threshold,
            mask_buffer: None,
            source_buffer: None,
            auto_tune: AutoTuneState::new(gpu_threshold),
            gpu_timer: None,
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
        let gpu_threshold = 10_000;
        Self {
            selection,
            shared_state,
            key_fn: Box::new(key_fn),
            dim_opacity: 0.2,
            last_generation: 0,
            gpu_threshold,
            mask_buffer: None,
            source_buffer: None,
            auto_tune: AutoTuneState::new(gpu_threshold),
            gpu_timer: None,
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
        self.auto_tune.effective_threshold = threshold;
        self
    }

    /// Returns the current GPU dimming threshold.
    pub fn gpu_threshold(&self) -> u32 {
        self.gpu_threshold
    }

    /// Enable or disable adaptive auto-tuning of the CPU/GPU dimming
    /// threshold (default: disabled).
    ///
    /// When enabled, `LinkedSelection` profiles both the CPU and GPU
    /// dimming paths during an initial calibration phase (default: 5 frames
    /// per path) and selects the faster path automatically.  The static
    /// [`gpu_dimming_threshold`](Self::gpu_dimming_threshold) serves as the
    /// initial estimate until calibration completes.
    ///
    /// When disabled, the static threshold is used directly.
    #[must_use]
    pub fn gpu_dimming_auto_tune(mut self, enabled: bool) -> Self {
        self.auto_tune.enabled = enabled;
        if enabled {
            self.auto_tune.reset();
        }
        self
    }

    /// Set the number of frames used per path during auto-tune calibration
    /// (default: 5).
    ///
    /// Higher values give more accurate profiling but extend the
    /// calibration period.  Has no effect when auto-tune is disabled.
    #[must_use]
    pub fn auto_tune_calibration_frames(mut self, frames: u32) -> Self {
        let frames = frames.max(1);
        self.auto_tune.calibration_frames = frames;
        self.auto_tune.reset();
        self
    }

    /// Returns the current effective threshold.
    ///
    /// When auto-tune is disabled, this equals
    /// [`gpu_threshold`](Self::gpu_threshold).  When auto-tune is enabled
    /// and calibration has settled, this reflects the profiling result.
    /// During calibration it returns the initial estimate.
    pub fn effective_threshold(&self) -> u32 {
        if self.auto_tune.enabled {
            self.auto_tune.effective_threshold()
        } else {
            self.gpu_threshold
        }
    }

    /// Returns `true` if auto-tune is enabled.
    pub fn is_auto_tune_enabled(&self) -> bool {
        self.auto_tune.enabled
    }

    /// Returns `true` if auto-tune calibration has settled.
    ///
    /// Returns `false` when auto-tune is disabled or calibration is still
    /// in progress.
    pub fn is_auto_tune_settled(&self) -> bool {
        self.auto_tune.enabled && self.auto_tune.is_settled()
    }

    /// Returns `true` if a GPU timestamp timer has been created for
    /// auto-tune profiling.
    ///
    /// This indicates that `Features::TIMESTAMP_QUERY` is available on the
    /// device and the timer has been lazily initialised during calibration.
    /// When `false`, the auto-tune system uses `Instant`-based wall-clock
    /// timing instead.
    pub fn has_gpu_timer(&self) -> bool {
        self.gpu_timer.is_some()
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
        let count_u32 = instance_count as u32;

        // -- Auto-tune: check if re-calibration is needed --
        if self.auto_tune.enabled {
            self.auto_tune.maybe_recalibrate(count_u32);
        }

        // -- Determine which path to use --
        let has_gpu_support = I::alpha_offsets().is_some();
        let use_gpu = if self.auto_tune.force_gpu() && has_gpu_support {
            // Calibration: force GPU path.
            true
        } else if self.auto_tune.force_cpu() {
            // Calibration: force CPU path.
            false
        } else {
            // Normal path: use effective threshold.
            let threshold = self.effective_threshold();
            count_u32 >= threshold && has_gpu_support
        };

        // -- Time the execution when auto-tune is active and calibrating --
        let timing = self.auto_tune.enabled && !self.auto_tune.is_settled();

        // Lazily create the GPU timer when auto-tune is calibrating and the
        // device supports timestamp queries.  The timer is kept for the
        // lifetime of the LinkedSelection so it can be reused across frames.
        if timing && use_gpu && self.gpu_timer.is_none() {
            self.gpu_timer = GpuTimer::new(device, queue);
        }

        let start = if timing { Some(Instant::now()) } else { None };

        let result = if use_gpu {
            self.prepare_render_gpu(
                device,
                queue,
                &mapper,
                instance_count,
                data_changed,
                timing,
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
                .map(|()| None)
        };

        // -- Record timing sample --
        // Prefer GPU timestamps when available; fall back to wall-clock.
        if let Some(start) = start {
            let gpu_ns = match &result {
                Ok(Some(ns)) => Some(*ns),
                _ => None,
            };
            let elapsed_ns = gpu_ns.unwrap_or_else(|| start.elapsed().as_nanos());
            self.auto_tune.record_sample(elapsed_ns, count_u32);
        }

        result.map(|_| ())
    }

    /// GPU dimming path: upload undimmed instances, run the compute shader,
    /// then copy the dimmed output into the Selection's instance buffer.
    ///
    /// When `timing` is `true` and a [`GpuTimer`] is available, the compute
    /// pass records GPU-side timestamps and the elapsed nanoseconds are
    /// returned as `Ok(Some(ns))`.  Otherwise returns `Ok(None)`.
    fn prepare_render_gpu<I>(
        &mut self,
        device: &Device,
        queue: &Queue,
        mapper: &dyn Fn(&T) -> I,
        instance_count: usize,
        data_changed: bool,
        timing: bool,
        cache: Option<&mut PipelineCache>,
        pool: Option<&mut BufferPool>,
    ) -> GupResult<Option<u128>>
    where
        I: DimInstance + bytemuck::Pod + bytemuck::Zeroable,
    {
        let alpha_offsets =
            I::alpha_offsets().expect("GPU path requires DimInstance::alpha_offsets");
        let count = instance_count as u32;

        // If GPU resources don't exist yet (e.g. first GPU call after CPU
        // path, or after a CPU calibration phase), treat as data_changed
        // so that source buffer and mask buffer are created.
        let data_changed = data_changed || self.mask_buffer.is_none();

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
            self.mask_buffer = Some(SelectionMaskBuffer::new(
                device,
                count,
                &alpha_offsets,
                None,
            )?);
        }

        // -- 2. Update mask & dispatch dimming compute shader ---------------
        let mask = self.mask_buffer.as_mut().expect("mask_buffer initialised");
        mask.ensure_capacity(device, count, None);

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

            // Build optional timestamp writes for the compute pass.
            let use_gpu_timer = timing && self.gpu_timer.is_some();
            let ts_writes = if use_gpu_timer {
                self.gpu_timer
                    .as_ref()
                    .map(|t| t.compute_pass_timestamp_writes())
            } else {
                None
            };

            // Encode the dimming compute pass (with optional timestamps).
            mask.encode_dimming_timed(
                device,
                queue,
                &mut encoder,
                source,
                count,
                self.dim_opacity,
                ts_writes,
            );

            // Copy the dimmed output into the Selection's instance buffer.
            let dst = self
                .selection
                .instance_buffer()
                .expect("render state initialised");
            encoder.copy_buffer_to_buffer(mask.output_buffer(), 0, dst, 0, instance_byte_size);

            // Resolve timestamp queries before submit.
            if use_gpu_timer {
                if let Some(timer) = &self.gpu_timer {
                    timer.resolve(&mut encoder);
                }
            }

            queue.submit([encoder.finish()]);

            // Read back GPU timestamps synchronously (blocking).
            if use_gpu_timer {
                if let Some(timer) = &self.gpu_timer {
                    return Ok(timer.read_elapsed_ns(device));
                }
            }
        }

        Ok(None)
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

    /// Returns the mean timings (CPU, GPU) in nanoseconds from the last
    /// completed auto-tune calibration, or `None` if calibration has not
    /// settled or auto-tune is disabled.
    pub fn auto_tune_timings(&self) -> Option<(u128, u128)> {
        if !self.auto_tune.enabled {
            return None;
        }
        match &self.auto_tune.phase {
            AutoTunePhase::Settled {
                cpu_mean_ns,
                gpu_mean_ns,
            } => Some((*cpu_mean_ns, *gpu_mean_ns)),
            _ => None,
        }
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

    // -- GPU dimming threshold / builder tests --

    #[test]
    fn gpu_dimming_threshold_default_is_10k() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx);

        assert_eq!(linked.gpu_threshold(), 10_000);
    }

    #[test]
    fn gpu_dimming_threshold_builder() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx).gpu_dimming_threshold(500);

        assert_eq!(linked.gpu_threshold(), 500);
    }

    #[test]
    fn is_gpu_dimming_active_false_initially() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx);

        assert!(!linked.is_gpu_dimming_active());
    }

    #[test]
    fn alpha_offsets_circle() {
        let offsets = CircleInstance::alpha_offsets();
        assert!(offsets.is_some());
        let offsets = offsets.unwrap();
        assert_eq!(offsets.offsets(), &[7, 15]);
    }

    #[test]
    fn alpha_offsets_rectangle() {
        let offsets = RectangleInstance::alpha_offsets();
        assert!(offsets.is_some());
        let offsets = offsets.unwrap();
        assert_eq!(offsets.offsets(), &[7, 15]);
    }

    #[test]
    fn alpha_offsets_line() {
        let offsets = LineInstance::alpha_offsets();
        assert!(offsets.is_some());
        let offsets = offsets.unwrap();
        assert_eq!(offsets.offsets(), &[7]);
    }

    #[test]
    fn alpha_offsets_boxplot() {
        let offsets = BoxPlotInstance::alpha_offsets();
        assert!(offsets.is_some());
        let offsets = offsets.unwrap();
        assert_eq!(offsets.offsets(), &[11, 15, 19, 23, 27]);
    }

    #[test]
    fn set_data_clears_gpu_resources() {
        let shared = SharedSelectionState::<usize>::new();
        let mut linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx);

        // GPU resources start cleared.
        assert!(!linked.is_gpu_dimming_active());
        // set_data should also ensure GPU resources are cleared.
        linked.set_data(vec![1.0, 2.0]);
        assert!(!linked.is_gpu_dimming_active());
    }

    #[test]
    fn from_selection_has_default_threshold() {
        let shared = SharedSelectionState::<usize>::new();
        let sel = crate::selection::Selection::<f32, crate::Circle>::from_data(vec![1.0]);
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::from_selection(sel, shared, |_item, idx| idx);

        assert_eq!(linked.gpu_threshold(), 10_000);
        assert!(!linked.is_gpu_dimming_active());
    }

    // -- AutoTuneState unit tests --

    #[test]
    fn auto_tune_state_new_is_disabled() {
        let state = AutoTuneState::new(10_000);
        assert!(!state.enabled);
        assert_eq!(state.effective_threshold(), 10_000);
        assert!(!state.is_settled());
    }

    #[test]
    fn auto_tune_state_initial_phase_is_probe_cpu() {
        let state = AutoTuneState::new(10_000);
        assert!(matches!(
            state.phase,
            AutoTunePhase::ProbeCpu {
                remaining: 5,
                total_ns: 0
            }
        ));
    }

    #[test]
    fn auto_tune_state_reset_returns_to_probe_cpu() {
        let mut state = AutoTuneState::new(10_000);
        state.enabled = true;
        state.calibration_frames = 3;
        state.reset(); // start fresh with calibration_frames=3

        // Advance past ProbeCpu by recording 3 samples
        for _ in 0..3 {
            state.record_sample(1000, 500);
        }
        assert!(
            state.force_gpu(),
            "Should be in ProbeGpu after completing ProbeCpu"
        );

        // Reset should return to ProbeCpu
        state.reset();
        assert!(state.force_cpu());
        assert!(matches!(
            state.phase,
            AutoTunePhase::ProbeCpu {
                remaining: 3,
                total_ns: 0
            }
        ));
    }

    #[test]
    fn auto_tune_record_sample_disabled_is_noop() {
        let mut state = AutoTuneState::new(10_000);
        // Disabled by default
        state.record_sample(999_999, 500);
        // Phase should not have changed
        assert!(matches!(state.phase, AutoTunePhase::ProbeCpu { .. }));
    }

    #[test]
    fn auto_tune_probe_cpu_then_gpu_then_settled() {
        let mut state = AutoTuneState::new(10_000);
        state.enabled = true;
        state.calibration_frames = 2;
        state.reset();

        // Phase: ProbeCpu
        assert!(state.force_cpu());
        assert!(!state.force_gpu());
        assert!(!state.is_settled());

        // Record 2 CPU samples (total 2000 ns)
        state.record_sample(1000, 500);
        assert!(state.force_cpu());
        state.record_sample(1000, 500);

        // Should have transitioned to ProbeGpu
        assert!(!state.force_cpu());
        assert!(state.force_gpu());
        assert!(!state.is_settled());

        // Record 2 GPU samples (total 600 ns — GPU is faster)
        state.record_sample(300, 500);
        state.record_sample(300, 500);

        // Should be settled now
        assert!(state.is_settled());
        assert!(!state.force_cpu());
        assert!(!state.force_gpu());

        // GPU was faster → threshold should be set to instance_count (500)
        assert_eq!(state.effective_threshold(), 500);
    }

    #[test]
    fn auto_tune_cpu_wins_sets_threshold_above_count() {
        let mut state = AutoTuneState::new(10_000);
        state.enabled = true;
        state.calibration_frames = 2;
        state.reset();

        // CPU is faster: 500ns mean
        state.record_sample(500, 1000);
        state.record_sample(500, 1000);

        // GPU is slower: 2000ns mean
        state.record_sample(2000, 1000);
        state.record_sample(2000, 1000);

        assert!(state.is_settled());
        // CPU wins → threshold = instance_count + 1
        assert_eq!(state.effective_threshold(), 1001);
    }

    #[test]
    fn auto_tune_maybe_recalibrate_no_change() {
        let mut state = AutoTuneState::new(10_000);
        state.enabled = true;
        state.calibrated_instance_count = 1000;
        state.phase = AutoTunePhase::Settled {
            cpu_mean_ns: 100,
            gpu_mean_ns: 50,
        };

        // Same count — no re-calibration
        state.maybe_recalibrate(1000);
        assert!(state.is_settled());

        // Small change (20%) — no re-calibration
        state.maybe_recalibrate(1200);
        assert!(state.is_settled());
    }

    #[test]
    fn auto_tune_maybe_recalibrate_on_large_change() {
        let mut state = AutoTuneState::new(10_000);
        state.enabled = true;
        state.calibrated_instance_count = 1000;
        state.phase = AutoTunePhase::Settled {
            cpu_mean_ns: 100,
            gpu_mean_ns: 50,
        };

        // Large change (>50%) — should trigger re-calibration
        state.maybe_recalibrate(2000);
        assert!(!state.is_settled());
        assert!(matches!(state.phase, AutoTunePhase::ProbeCpu { .. }));
    }

    #[test]
    fn auto_tune_maybe_recalibrate_disabled_is_noop() {
        let mut state = AutoTuneState::new(10_000);
        state.calibrated_instance_count = 1000;
        state.phase = AutoTunePhase::Settled {
            cpu_mean_ns: 100,
            gpu_mean_ns: 50,
        };

        // Disabled — should not re-calibrate
        state.maybe_recalibrate(5000);
        assert!(state.is_settled());
    }

    // -- LinkedSelection auto-tune builder tests --

    #[test]
    fn linked_selection_auto_tune_default_disabled() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx);

        assert!(!linked.is_auto_tune_enabled());
        assert!(!linked.is_auto_tune_settled());
    }

    #[test]
    fn linked_selection_auto_tune_builder() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx).gpu_dimming_auto_tune(true);

        assert!(linked.is_auto_tune_enabled());
        assert!(!linked.is_auto_tune_settled());
    }

    #[test]
    fn linked_selection_effective_threshold_without_auto_tune() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx).gpu_dimming_threshold(5000);

        assert_eq!(linked.effective_threshold(), 5000);
    }

    #[test]
    fn linked_selection_effective_threshold_with_auto_tune_uses_initial() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx)
                .gpu_dimming_threshold(7500)
                .gpu_dimming_auto_tune(true);

        // Before calibration settles, effective_threshold returns the initial
        assert_eq!(linked.effective_threshold(), 7500);
    }

    #[test]
    fn linked_selection_calibration_frames_builder() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx)
                .gpu_dimming_auto_tune(true)
                .auto_tune_calibration_frames(10);

        assert!(linked.is_auto_tune_enabled());
        assert_eq!(linked.auto_tune.calibration_frames, 10);
    }

    #[test]
    fn linked_selection_auto_tune_timings_none_when_disabled() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx);

        assert!(linked.auto_tune_timings().is_none());
    }

    #[test]
    fn linked_selection_auto_tune_timings_none_during_calibration() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx).gpu_dimming_auto_tune(true);

        assert!(linked.auto_tune_timings().is_none());
    }

    #[test]
    fn linked_selection_gpu_timer_none_by_default() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx);
        assert!(!linked.has_gpu_timer());
    }

    #[test]
    fn linked_selection_gpu_timer_none_before_prepare_render() {
        let shared = SharedSelectionState::<usize>::new();
        let linked: LinkedSelection<f32, crate::Circle, usize> =
            LinkedSelection::new(vec![1.0], shared, |_item, idx| idx).gpu_dimming_auto_tune(true);
        // Timer is lazily created during prepare_render, not at construction.
        assert!(!linked.has_gpu_timer());
    }
}
