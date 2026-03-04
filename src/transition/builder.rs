// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Transition builder for configuring animated data transitions.
//!
//! The [`TransitionBuilder`] provides a fluent API for specifying how elements
//! should animate when data is rebound. It configures duration, delay, easing,
//! and per-attribute target values for the enter, update, and exit groups.

use std::collections::HashMap;

use crate::mark::Mark;
use crate::selection::{AttrValue, IntoAttrValue};
use crate::shader_function::{EasingFunction, InterpolationMode};
use crate::{MaybeSend, MaybeSync};

// ---------------------------------------------------------------------------
// Easing wrapper enum
// ---------------------------------------------------------------------------

/// Easing function specification for transitions.
///
/// This enum provides a unified set of easing options that map to the
/// existing [`EasingFunction`] variants and [`InterpolationMode`] spline
/// curves.
#[derive(Debug, Clone, Default)]
pub enum EasingFn {
    /// Linear interpolation (no easing).
    #[default]
    Linear,
    /// Quadratic ease-in (slow start).
    EaseIn,
    /// Quadratic ease-out (slow end).
    EaseOut,
    /// Cubic ease-in-out (slow start and end).
    EaseInOut,
    /// Cubic Bezier curve (maps to `EasingFunction::EaseInOutCubic`).
    CubicBezier,
    /// Catmull-Rom spline interpolation with configurable tension.
    CatmullRom {
        /// Tension parameter in `[0.0, 1.0]`.
        tension: f32,
    },
    /// B-spline interpolation for smooth curves.
    BSpline,
}

impl EasingFn {
    /// Convert to the corresponding [`EasingFunction`] for timeline control.
    pub fn to_easing_function(&self) -> EasingFunction {
        match self {
            EasingFn::Linear => EasingFunction::Linear,
            EasingFn::EaseIn => EasingFunction::EaseInQuad,
            EasingFn::EaseOut => EasingFunction::EaseOutQuad,
            EasingFn::EaseInOut => EasingFunction::EaseInOutCubic,
            EasingFn::CubicBezier => EasingFunction::EaseInOutCubic,
            // Spline modes use linear easing; the curve shape comes from
            // InterpolationMode instead.
            EasingFn::CatmullRom { .. } => EasingFunction::Linear,
            EasingFn::BSpline => EasingFunction::Linear,
        }
    }

    /// Convert to an optional [`InterpolationMode`] for spline-based easing.
    ///
    /// Returns `None` for non-spline easing functions.
    pub fn to_interpolation_mode(&self) -> Option<InterpolationMode> {
        match self {
            EasingFn::CatmullRom { tension } => {
                Some(InterpolationMode::CatmullRom { tension: *tension })
            }
            EasingFn::BSpline => Some(InterpolationMode::BSpline),
            _ => None,
        }
    }

    /// Apply the easing function to a normalised time value on the CPU.
    ///
    /// `t` is expected in `[0.0, 1.0]` and the result is clamped to the
    /// same range. This mirrors the GPU easing curves so that
    /// CPU-side interpolation matches GPU-side behaviour.
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            EasingFn::Linear => t,
            EasingFn::EaseIn => t * t,
            EasingFn::EaseOut => t * (2.0 - t),
            EasingFn::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            EasingFn::CubicBezier => {
                // Same as EaseInOutCubic
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            // Spline modes use linear easing; the curve shape comes from
            // InterpolationMode instead.
            EasingFn::CatmullRom { .. } | EasingFn::BSpline => t,
        }
    }
}

// ---------------------------------------------------------------------------
// Transition configuration
// ---------------------------------------------------------------------------

/// Immutable configuration for a committed transition.
///
/// This is the final form of a transition after [`TransitionBuilder::commit`]
/// has been called. It stores the resolved duration, delay, easing, and
/// per-attribute from/to values for each element group.
#[derive(Debug, Clone)]
pub struct TransitionConfig {
    /// Total transition duration in milliseconds.
    pub duration_ms: u64,
    /// Delay before the transition begins, in milliseconds.
    pub delay_ms: u64,
    /// Easing function applied to the normalised time parameter.
    pub easing: EasingFn,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            duration_ms: 250,
            delay_ms: 0,
            easing: EasingFn::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Transition state
// ---------------------------------------------------------------------------

/// Tracks the lifecycle state of a transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionState {
    /// Transition has been configured but not yet committed.
    Pending,
    /// Transition is actively animating.
    Running,
    /// Transition has completed all animations.
    Completed,
}

// ---------------------------------------------------------------------------
// Attr target closure (type-erased)
// ---------------------------------------------------------------------------

/// A type-erased attribute target closure: given a data item reference, it
/// produces the "to" [`AttrValue`].
struct AttrTargetFn<T> {
    #[cfg(not(target_arch = "wasm32"))]
    extractor: Box<dyn Fn(&T) -> AttrValue + Send + Sync>,
    #[cfg(target_arch = "wasm32")]
    extractor: Box<dyn Fn(&T) -> AttrValue>,
}

impl<T> AttrTargetFn<T> {
    fn new<V, F>(f: F) -> Self
    where
        V: IntoAttrValue,
        F: Fn(&T) -> V + MaybeSend + MaybeSync + 'static,
    {
        Self {
            extractor: Box::new(move |item: &T| f(item).into_attr_value()),
        }
    }

    fn eval(&self, item: &T) -> AttrValue {
        (self.extractor)(item)
    }
}

// ---------------------------------------------------------------------------
// Per-element animation snapshot
// ---------------------------------------------------------------------------

/// Snapshot of per-element attribute values for animation.
///
/// Stores the "from" and "to" values for each named attribute on a single
/// element, along with the transition configuration.
#[derive(Debug, Clone)]
pub struct ElementTransition {
    /// Named attribute from→to pairs.
    pub attrs: HashMap<String, (AttrValue, AttrValue)>,
    /// The element group this transition belongs to.
    pub group: TransitionGroup,
}

/// Which group an element belongs to in the transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionGroup {
    /// New element entering the scene.
    Enter,
    /// Existing element updating in place.
    Update,
    /// Element exiting the scene.
    Exit,
}

// ---------------------------------------------------------------------------
// Committed transition result
// ---------------------------------------------------------------------------

/// The result of committing a transition: per-element animation snapshots
/// and the shared transition configuration.
#[derive(Debug, Clone)]
pub struct CommittedTransition {
    /// The transition configuration (duration, delay, easing).
    pub config: TransitionConfig,
    /// Per-element animation data (from/to values per attribute).
    pub elements: Vec<ElementTransition>,
    /// The new data vector after the transition (enter + update items; exit
    /// items are present during animation but marked for removal).
    pub new_data_len: usize,
    /// Number of enter elements.
    pub enter_count: usize,
    /// Number of update elements.
    pub update_count: usize,
    /// Number of exit elements.
    pub exit_count: usize,
    /// Current state of the transition.
    pub state: TransitionState,
    /// Elapsed time in milliseconds since the transition was started.
    pub elapsed_ms: f64,
}

// ---------------------------------------------------------------------------
// Callback types
// ---------------------------------------------------------------------------

/// Callback invoked when the transition starts (after any delay).
#[cfg(not(target_arch = "wasm32"))]
type OnStartFn = Box<dyn Fn() + Send + Sync>;
#[cfg(target_arch = "wasm32")]
type OnStartFn = Box<dyn Fn()>;

/// Callback invoked when the transition ends (all groups finished).
#[cfg(not(target_arch = "wasm32"))]
type OnEndFn = Box<dyn Fn() + Send + Sync>;
#[cfg(target_arch = "wasm32")]
type OnEndFn = Box<dyn Fn()>;

// ---------------------------------------------------------------------------
// TransitionBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for configuring data transitions on a selection.
///
/// Created by calling `transition()` on a selection that has a pending
/// diff result from `data_keyed()`. The builder records target attribute
/// values, duration, easing, and callbacks, then commits the transition
/// to schedule GPU animations.
///
/// # Example
///
/// ```rust,ignore
/// selection
///     .data_keyed(new_data, |p| p.id)
///     .transition()
///     .duration(800)
///     .ease(EasingFn::EaseInOut)
///     .attr("cx", |p| p.x)
///     .attr("cy", |p| p.y)
///     .commit();
/// ```
pub struct TransitionBuilder<'a, T, M: Mark> {
    /// Reference to the selection this transition operates on.
    selection: &'a mut crate::selection::Selection<T, M>,
    /// Duration in milliseconds.
    duration_ms: u64,
    /// Delay in milliseconds.
    delay_ms: u64,
    /// Easing function.
    easing: EasingFn,
    /// Named attribute target closures for update/enter elements.
    attr_targets: HashMap<String, AttrTargetFn<T>>,
    /// Custom enter attribute initial value closures (overrides defaults).
    enter_attr_overrides: HashMap<String, AttrTargetFn<T>>,
    /// Custom exit attribute final value closures (overrides defaults).
    exit_attr_overrides: HashMap<String, AttrTargetFn<T>>,
    /// Callback fired when the transition starts.
    on_start: Option<OnStartFn>,
    /// Callback fired when the transition ends.
    on_end: Option<OnEndFn>,
}

impl<'a, T, M: Mark> TransitionBuilder<'a, T, M>
where
    T: Clone + MaybeSend + MaybeSync + 'static,
{
    /// Create a new `TransitionBuilder` for the given selection.
    pub(crate) fn new(selection: &'a mut crate::selection::Selection<T, M>) -> Self {
        Self {
            selection,
            duration_ms: 250,
            delay_ms: 0,
            easing: EasingFn::default(),
            attr_targets: HashMap::new(),
            enter_attr_overrides: HashMap::new(),
            exit_attr_overrides: HashMap::new(),
            on_start: None,
            on_end: None,
        }
    }

    /// Set the total transition duration in milliseconds.
    pub fn duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Set a delay before the transition begins, in milliseconds.
    pub fn delay(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }

    /// Set the easing function for the transition.
    pub fn ease(mut self, easing: EasingFn) -> Self {
        self.easing = easing;
        self
    }

    /// Declare the target ("to") value for a named attribute.
    ///
    /// For update elements, the attribute animates from its current value to
    /// the value produced by `value_fn`. For enter elements, the attribute
    /// animates from the default enter state (e.g., opacity 0) to this value.
    pub fn attr<V, F>(mut self, name: &str, value_fn: F) -> Self
    where
        V: IntoAttrValue,
        F: Fn(&T) -> V + MaybeSend + MaybeSync + 'static,
    {
        self.attr_targets
            .insert(name.to_string(), AttrTargetFn::new(value_fn));
        self
    }

    /// Override the initial ("from") value for enter elements on a named
    /// attribute.
    ///
    /// By default, enter elements start at `opacity = 0.0`. Use this to
    /// specify a different starting value.
    pub fn enter_attr<V, F>(mut self, name: &str, initial_fn: F) -> Self
    where
        V: IntoAttrValue,
        F: Fn(&T) -> V + MaybeSend + MaybeSync + 'static,
    {
        self.enter_attr_overrides
            .insert(name.to_string(), AttrTargetFn::new(initial_fn));
        self
    }

    /// Override the final ("to") value for exit elements on a named attribute.
    ///
    /// By default, exit elements animate to `opacity = 0.0`. Use this to
    /// specify a different ending value.
    pub fn exit_attr<V, F>(mut self, name: &str, final_fn: F) -> Self
    where
        V: IntoAttrValue,
        F: Fn(&T) -> V + MaybeSend + MaybeSync + 'static,
    {
        self.exit_attr_overrides
            .insert(name.to_string(), AttrTargetFn::new(final_fn));
        self
    }

    /// Register a callback that fires when the transition starts (after delay).
    pub fn on_start<F>(mut self, callback: F) -> Self
    where
        F: Fn() + MaybeSend + MaybeSync + 'static,
    {
        self.on_start = Some(Box::new(callback));
        self
    }

    /// Register a callback that fires when all groups have finished animating.
    pub fn on_end<F>(mut self, callback: F) -> Self
    where
        F: Fn() + MaybeSend + MaybeSync + 'static,
    {
        self.on_end = Some(Box::new(callback));
        self
    }

    /// Commit the transition: capture from-state, compute to-state, and
    /// schedule animations.
    ///
    /// If no `.attr()` calls were made, this is a no-op and a warning is
    /// emitted via `tracing`.
    ///
    /// Returns a [`CommittedTransition`] describing the scheduled animations,
    /// or `None` if the transition was a no-op.
    pub fn commit(self) -> Option<CommittedTransition> {
        if self.attr_targets.is_empty() {
            log::warn!(
                "TransitionBuilder::commit() called with no attr() bindings; \
                 transition is a no-op"
            );
            return None;
        }

        let diff = self.selection.take_diff_result();
        let current_attrs = self.snapshot_current_attrs();

        let config = TransitionConfig {
            duration_ms: self.duration_ms,
            delay_ms: self.delay_ms,
            easing: self.easing,
        };

        let mut elements = Vec::new();

        match diff {
            Some(diff) => {
                // --- Update elements ---
                for (old_item, new_item) in &diff.update {
                    let mut attrs = HashMap::new();
                    for (name, target_fn) in &self.attr_targets {
                        let from = current_attrs
                            .get(name)
                            .map(|_vals| target_fn.eval(old_item))
                            .unwrap_or(AttrValue::Float(0.0));
                        let to = target_fn.eval(new_item);
                        attrs.insert(name.clone(), (from, to));
                    }
                    elements.push(ElementTransition {
                        attrs,
                        group: TransitionGroup::Update,
                    });
                }

                // --- Enter elements ---
                for item in &diff.enter {
                    let mut attrs = HashMap::new();
                    for (name, target_fn) in &self.attr_targets {
                        let from = if let Some(enter_fn) = self.enter_attr_overrides.get(name) {
                            enter_fn.eval(item)
                        } else if name == "opacity" || name == "fill_opacity" {
                            AttrValue::Float(0.0)
                        } else {
                            // Default: start at the target value (no animation
                            // on this attr for enter unless opacity).
                            target_fn.eval(item)
                        };
                        let to = target_fn.eval(item);
                        attrs.insert(name.clone(), (from, to));
                    }
                    elements.push(ElementTransition {
                        attrs,
                        group: TransitionGroup::Enter,
                    });
                }

                // --- Exit elements ---
                for item in &diff.exit {
                    let mut attrs = HashMap::new();
                    for (name, target_fn) in &self.attr_targets {
                        let from = target_fn.eval(item);
                        let to = if let Some(exit_fn) = self.exit_attr_overrides.get(name) {
                            exit_fn.eval(item)
                        } else if name == "opacity" || name == "fill_opacity" {
                            AttrValue::Float(0.0)
                        } else {
                            // Default: end at the current value (no animation
                            // on this attr for exit unless opacity).
                            from
                        };
                        attrs.insert(name.clone(), (from, to));
                    }
                    elements.push(ElementTransition {
                        attrs,
                        group: TransitionGroup::Exit,
                    });
                }

                let enter_count = diff.enter.len();
                let update_count = diff.update.len();
                let exit_count = diff.exit.len();

                // Update the selection's data: new data is update (new values)
                // + enter. Exit elements are kept during animation but tracked
                // for removal.
                let mut new_data: Vec<T> = diff
                    .update
                    .iter()
                    .map(|(_, new_item)| new_item.clone())
                    .collect();
                new_data.extend(diff.enter.iter().cloned());
                let new_data_len = new_data.len();
                // Append exit elements at the end (they'll be removed after
                // the transition completes).
                new_data.extend(diff.exit.iter().cloned());
                self.selection.set_data(new_data);

                // Fire on_start callback.
                if let Some(on_start) = &self.on_start {
                    on_start();
                }

                // Store transition metadata on the selection.
                let committed = CommittedTransition {
                    config,
                    elements,
                    new_data_len,
                    enter_count,
                    update_count,
                    exit_count,
                    state: TransitionState::Running,
                    elapsed_ms: 0.0,
                };

                // Store on_end callback in selection for later invocation.
                self.selection.set_transition_end_callback(self.on_end);
                self.selection
                    .set_committed_transition(Some(committed.clone()));

                Some(committed)
            }
            None => {
                // No diff result — treat the entire selection as the update
                // group (transition without prior data_keyed call).
                let data = self.selection.data().to_vec();
                for item in &data {
                    let mut attrs = HashMap::new();
                    for (name, target_fn) in &self.attr_targets {
                        let from = current_attrs
                            .get(name)
                            .map(|_| target_fn.eval(item))
                            .unwrap_or_else(|| AttrValue::Float(0.0));
                        let to = target_fn.eval(item);
                        attrs.insert(name.clone(), (from, to));
                    }
                    elements.push(ElementTransition {
                        attrs,
                        group: TransitionGroup::Update,
                    });
                }

                let update_count = data.len();

                // Fire on_start callback.
                if let Some(on_start) = &self.on_start {
                    on_start();
                }

                let committed = CommittedTransition {
                    config,
                    elements,
                    new_data_len: update_count,
                    enter_count: 0,
                    update_count,
                    exit_count: 0,
                    state: TransitionState::Running,
                    elapsed_ms: 0.0,
                };

                self.selection.set_transition_end_callback(self.on_end);
                self.selection
                    .set_committed_transition(Some(committed.clone()));

                Some(committed)
            }
        }
    }

    /// Snapshot current attribute values from the selection's existing attr
    /// bindings (CPU-side).
    fn snapshot_current_attrs(&self) -> HashMap<String, Vec<AttrValue>> {
        let mut result = HashMap::new();
        let bound = self.selection.bound_attributes();
        for name in bound {
            // We record that the attribute exists; actual per-element from
            // values are computed from the target_fn evaluated on old data
            // (for update/exit) rather than from stored CPU values.
            result.insert(name.to_string(), Vec::new());
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_fn_default() {
        let e = EasingFn::default();
        assert!(matches!(e, EasingFn::Linear));
    }

    #[test]
    fn test_easing_fn_to_easing_function() {
        assert!(matches!(
            EasingFn::Linear.to_easing_function(),
            EasingFunction::Linear
        ));
        assert!(matches!(
            EasingFn::EaseIn.to_easing_function(),
            EasingFunction::EaseInQuad
        ));
        assert!(matches!(
            EasingFn::EaseOut.to_easing_function(),
            EasingFunction::EaseOutQuad
        ));
        assert!(matches!(
            EasingFn::EaseInOut.to_easing_function(),
            EasingFunction::EaseInOutCubic
        ));
    }

    #[test]
    fn test_easing_fn_to_interpolation_mode() {
        assert!(EasingFn::Linear.to_interpolation_mode().is_none());
        assert!(EasingFn::EaseInOut.to_interpolation_mode().is_none());

        let cr = EasingFn::CatmullRom { tension: 0.5 };
        let mode = cr.to_interpolation_mode().unwrap();
        assert!(
            matches!(mode, InterpolationMode::CatmullRom { tension } if (tension - 0.5).abs() < f32::EPSILON)
        );

        let bs = EasingFn::BSpline;
        assert!(matches!(
            bs.to_interpolation_mode().unwrap(),
            InterpolationMode::BSpline
        ));
    }

    #[test]
    fn test_transition_config_default() {
        let config = TransitionConfig::default();
        assert_eq!(config.duration_ms, 250);
        assert_eq!(config.delay_ms, 0);
        assert!(matches!(config.easing, EasingFn::Linear));
    }

    #[test]
    fn test_transition_state_variants() {
        assert_eq!(TransitionState::Pending, TransitionState::Pending);
        assert_ne!(TransitionState::Running, TransitionState::Completed);
    }

    #[test]
    fn test_element_transition_groups() {
        let et = ElementTransition {
            attrs: HashMap::new(),
            group: TransitionGroup::Enter,
        };
        assert_eq!(et.group, TransitionGroup::Enter);

        let et2 = ElementTransition {
            attrs: HashMap::new(),
            group: TransitionGroup::Exit,
        };
        assert_eq!(et2.group, TransitionGroup::Exit);
    }

    #[test]
    fn test_committed_transition_counts() {
        let ct = CommittedTransition {
            config: TransitionConfig::default(),
            elements: vec![],
            new_data_len: 10,
            enter_count: 3,
            update_count: 5,
            exit_count: 2,
            state: TransitionState::Running,
            elapsed_ms: 0.0,
        };
        assert_eq!(ct.enter_count, 3);
        assert_eq!(ct.update_count, 5);
        assert_eq!(ct.exit_count, 2);
        assert_eq!(ct.new_data_len, 10);
    }
}
