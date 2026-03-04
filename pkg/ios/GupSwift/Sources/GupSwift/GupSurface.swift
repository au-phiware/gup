// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

/// Represents an attached Metal surface within a `GupContext`.

import Foundation

/// A handle to a Metal surface attached to a `GupContext`.
///
/// Created via `GupContext` and tied to a specific `UIView`.
public final class GupSurface: @unchecked Sendable {
    private let context: GupContext
    private let surfaceId: UInt64

    /// Whether this surface has been detached.
    private var isDetached = false

    internal init(context: GupContext, surfaceId: UInt64) {
        self.context = context
        self.surfaceId = surfaceId
    }

    deinit {
        detach()
    }

    /// Detach this surface from the context.
    ///
    /// After calling this method the surface ID is invalid and
    /// `renderFrame()` will do nothing.
    public func detach() {
        guard !isDetached else { return }
        isDetached = true
        gup_surface_detach(context.rawHandle, surfaceId)
    }

    /// Render one frame on this surface.
    ///
    /// - Returns: `true` if the frame rendered successfully.
    @discardableResult
    public func renderFrame() -> Bool {
        guard !isDetached else { return false }
        return gup_render_frame(context.rawHandle, surfaceId)
    }

    /// Notify the surface of a new drawable size (e.g. after rotation).
    @discardableResult
    public func resize(width: UInt32, height: UInt32) -> Bool {
        guard !isDetached else { return false }
        return gup_surface_resize(context.rawHandle, surfaceId, width, height)
    }

    /// The raw surface ID (for debugging or advanced use).
    public var id: UInt64 { surfaceId }
}
