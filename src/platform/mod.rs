// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Platform-specific integration modules.
//!
//! Each sub-module is feature-gated and target-gated so that it only compiles
//! on the relevant platform.
//!
//! The [`ios_touch`] sub-module is available on *all* platforms (behind the
//! `ios-shim` feature) so that its pure-logic touch translation can be unit
//! tested without cross-compilation.  The full iOS surface module
//! ([`ios`]) additionally requires `target_os = "ios"`.
//!
//! Likewise, [`android_touch`] is available everywhere (behind the
//! `android-shim` feature) while the full Android surface module
//! ([`android`]) additionally requires `target_os = "android"`.

#[cfg(feature = "ios-shim")]
pub mod ios_touch;

#[cfg(all(feature = "ios-shim", target_os = "ios"))]
pub mod ios;

#[cfg(feature = "android-shim")]
pub mod android_touch;

#[cfg(all(feature = "android-shim", target_os = "android"))]
pub mod android;
