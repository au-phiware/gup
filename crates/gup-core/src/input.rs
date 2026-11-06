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
    /// A key went down.
    KeyDown {
        /// Which key, as a [`Key`] (semantic, not physical).
        key: Key,
        /// The modifiers held.
        modifiers: Modifiers,
    },
}

impl Input {
    /// The pointer moved to `at`.
    pub fn pointer_moved(at: impl Into<Point>) -> Self {
        Self::PointerMoved { at: at.into() }
    }

    /// `button` went down at `at`.
    pub fn pointer_down(at: impl Into<Point>, button: Button) -> Self {
        Self::PointerDown {
            at: at.into(),
            button,
        }
    }

    /// `button` went up at `at`.
    pub fn pointer_up(at: impl Into<Point>, button: Button) -> Self {
        Self::PointerUp {
            at: at.into(),
            button,
        }
    }

    /// The wheel turned `lines` with the pointer at `at`.
    pub fn wheel(at: impl Into<Point>, lines: f32) -> Self {
        Self::Wheel {
            at: at.into(),
            lines,
        }
    }

    /// `key` went down with no modifiers.
    pub fn key(key: Key) -> Self {
        Self::key_with(key, Modifiers::NONE)
    }

    /// `key` went down with `modifiers` held.
    pub fn key_with(key: Key, modifiers: Modifiers) -> Self {
        Self::KeyDown { key, modifiers }
    }
}

/// A key a chart understands. Keys are semantic, not physical: a host maps
/// its keyboard to them (`+` to `ZoomIn`, `-` to `ZoomOut`, `0` to
/// `ResetView`, Space to `Enter`).
///
/// With no modifiers, the arrows, `Home` and `End` move keyboard focus
/// over the chart's rows (see [`Plot::focused`](crate::Plot::focused)):
/// `Right` and `Left` walk the focused layer in x order, `Up` and `Down`
/// move to the next and previous layer, `Home` and `End` jump to its ends,
/// `Enter` clicks the focused row and `Escape` clears focus. `ZoomIn`,
/// `ZoomOut` and `ResetView` do nothing yet.
///
/// ```
/// use gup_core::prelude::*;
/// use gup_core::{Input, Key, RowId};
///
/// # fn main() -> gup_core::Result<()> {
/// let cx = Context::new_blocking()?;
/// let mut plot = Plot::new();
/// plot.circles(vec![(1.0, 2.0), (3.0, 1.0)])
///     .x(|r: &(f64, f64)| r.0)
///     .y(|r: &(f64, f64)| r.1);
/// plot.resolve(&cx, 320.0, 200.0)?;
///
/// let response = plot.handle(Input::key(Key::Right));
/// assert!(response.redraw);
/// assert_eq!(plot.focused(), Some(RowId { layer: 0, row: 0 }));
/// # Ok(())
/// # }
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Enter (hosts also map Space).
    Enter,
    /// Escape.
    Escape,
    /// Zoom in (`+`).
    ZoomIn,
    /// Zoom out (`-`).
    ZoomOut,
    /// Reset the view (`0`).
    ResetView,
}

/// The modifier keys held with a [`Key`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Shift.
    pub shift: bool,
    /// Control.
    pub ctrl: bool,
    /// Alt (Option).
    pub alt: bool,
    /// Meta (Command, Windows).
    pub meta: bool,
}

impl Modifiers {
    /// No modifier.
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    };
    /// Shift alone.
    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };
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

/// Which axes pointer input may zoom and pan.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Axes {
    /// Both axes.
    #[default]
    Both,
    /// Only the x axis.
    X,
    /// Only the y axis.
    Y,
    /// Neither: input leaves the view alone.
    Neither,
}

impl Axes {
    /// Whether the x and y axes (in that order) may move.
    pub(crate) fn flags(self) -> [bool; 2] {
        match self {
            Axes::Both => [true, true],
            Axes::X => [true, false],
            Axes::Y => [false, true],
            Axes::Neither => [false, false],
        }
    }
}

/// What a primary-button drag does.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Drag {
    /// Move the view, so the content follows the pointer.
    #[default]
    Pan,
    /// Nothing.
    Off,
    /// Select the rows whose marks lie inside the dragged rectangle (see
    /// [`Plot::brush`](crate::Plot::brush)).
    Brush,
    /// Zoom the view to the dragged rectangle.
    Zoom,
}

/// A rectangle selection, in data units.
///
/// A row is brushed when its mark's position lies inside the rectangle,
/// edges included: a disc's or a box's centre, a segment's midpoint, a
/// line's vertex. A large mark that merely overlaps the rectangle is not
/// brushed.
#[derive(Clone, Debug, PartialEq)]
pub struct Brush {
    /// The x extent, (min, max) in data units (codes on a band axis).
    pub x: (f64, f64),
    /// The y extent, (min, max).
    pub y: (f64, f64),
    /// Rows whose position is inside, sorted by (layer, row).
    pub rows: Vec<crate::RowId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_equal_the_literals() {
        let at = Point::new(3.0, 4.0);
        assert_eq!(Input::pointer_moved((3.0, 4.0)), Input::PointerMoved { at });
        assert_eq!(
            Input::pointer_down([3.0, 4.0], Button::Primary),
            Input::PointerDown {
                at,
                button: Button::Primary
            }
        );
        assert_eq!(
            Input::pointer_up(at, Button::Middle),
            Input::PointerUp {
                at,
                button: Button::Middle
            }
        );
        assert_eq!(
            Input::wheel((3.0, 4.0), 1.5),
            Input::Wheel { at, lines: 1.5 }
        );
        assert_eq!(
            Input::key(Key::Up),
            Input::KeyDown {
                key: Key::Up,
                modifiers: Modifiers::default()
            }
        );
        assert_eq!(
            Input::key_with(Key::Up, Modifiers::SHIFT),
            Input::KeyDown {
                key: Key::Up,
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::NONE
                }
            }
        );
    }
}
