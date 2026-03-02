// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data transition system for animated enter/update/exit patterns.
//!
//! This module implements key-based data diffing and GPU-interpolated transitions
//! for smooth data rebinding animations. When a dataset changes, elements are
//! partitioned into three groups:
//!
//! - **Enter**: new elements that appear (fade/grow in)
//! - **Update**: existing elements that animate to new values
//! - **Exit**: removed elements that disappear (fade/shrink out)
//!
//! The [`TransitionBuilder`] provides a fluent API for configuring transitions
//! with duration, delay, easing, and per-attribute target values.

pub mod builder;
pub mod diff;

pub use builder::{TransitionBuilder, TransitionConfig, TransitionState};
pub use diff::{DiffResult, diff_by_key};
