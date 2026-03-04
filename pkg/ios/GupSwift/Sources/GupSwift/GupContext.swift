// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

/// Idiomatic Swift wrapper around the Gup GPU context.
///
/// Usage:
/// ```swift
/// let ctx = GupContext()
/// // attach surfaces, render, etc.
/// ```
///
/// The context is destroyed automatically when this object is deallocated.

import Foundation

/// Manages the lifecycle of the Rust-side GPU context.
public final class GupContext: @unchecked Sendable {
    /// The raw opaque pointer to the Rust context.
    private let handle: GupContextHandle

    /// Create a new GPU context.
    ///
    /// - Throws: `GupError.contextCreationFailed` if the GPU device
    ///   cannot be initialised.
    public init() throws {
        guard let h = gup_context_create() else {
            throw GupError.contextCreationFailed
        }
        self.handle = h
    }

    deinit {
        gup_context_destroy(handle)
    }

    // MARK: - Internal

    /// Access the raw handle (used by `GupSurface`).
    internal var rawHandle: GupContextHandle { handle }
}

// MARK: - Errors

/// Errors thrown by the GupSwift wrapper.
public enum GupError: Error, Sendable {
    /// The GPU context could not be created.
    case contextCreationFailed
    /// A surface could not be attached.
    case surfaceAttachFailed
    /// A frame render failed.
    case renderFailed
}
