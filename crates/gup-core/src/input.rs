// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Input a host passes to a [`Chart`](crate::Chart).

use crate::geom::Point;
use crate::pick::RowId;

/// Pointer and wheel input in chart coordinates: logical pixels from the
/// chart's top-left corner, y down (the coordinates of
/// [`Layout`](crate::Layout)).
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Input {
    /// The pointer moved over the chart.
    PointerMoved {
        /// Where, in chart coordinates.
        at: Point,
    },
    /// The pointer left the chart.
    PointerLeft,
    /// A button was pressed.
    PointerDown {
        /// Where, in chart coordinates.
        at: Point,
        /// Which button.
        button: Button,
    },
    /// A button was released.
    PointerUp {
        /// Where, in chart coordinates.
        at: Point,
        /// Which button.
        button: Button,
    },
    /// The wheel turned.
    Wheel {
        /// Where the pointer is, in chart coordinates.
        at: Point,
        /// `lines > 0` scrolls up (away from the user), which zooms in.
        lines: f32,
    },
}

/// A pointer button.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Button {
    /// The left button (or a touch).
    Primary,
    /// The right button.
    Secondary,
    /// The middle button.
    Middle,
}

/// What the host should do after [`Chart::handle`](crate::Chart::handle).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct Response {
    /// The chart looks different: resolve and render again.
    pub redraw: bool,
}

/// What happens to a row a handler registered with
/// [`Selection::on`](crate::Selection::on) listens for.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Event {
    /// The pointer moved onto a row.
    Enter,
    /// The pointer left a row (or the chart, or a zoom moved it away).
    Leave,
    /// The primary button went down and up on a row, with the pointer
    /// moving no more than a few pixels between.
    Click,
}

/// What a handler is told about an [`Event`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EventInfo {
    /// What happened.
    pub event: Event,
    /// Where the pointer was, in chart coordinates (for `Leave`, where the
    /// pointer was when the row was left; for a zoom or `PointerLeft`, the
    /// last position the plot knew).
    pub at: Point,
    /// The row.
    pub row: RowId,
}
