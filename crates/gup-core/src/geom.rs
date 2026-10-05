// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Points and rectangles in logical pixels (origin top-left, y down).

/// A point in logical pixels.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Point {
    /// Horizontal position.
    pub x: f32,
    /// Vertical position (down).
    pub y: f32,
}

impl Point {
    /// A point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// An axis-aligned rectangle in logical pixels.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Rect {
    /// A rectangle from its top-left corner and size.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// A rectangle from its edges.
    pub fn from_edges(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
    }

    /// Left edge.
    pub fn left(&self) -> f32 {
        self.x
    }

    /// Right edge.
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Top edge.
    pub fn top(&self) -> f32 {
        self.y
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Whether `other` lies entirely inside this rectangle.
    pub fn contains_rect(&self, other: &Rect) -> bool {
        other.left() >= self.left()
            && other.right() <= self.right()
            && other.top() >= self.top()
            && other.bottom() <= self.bottom()
    }

    /// Grow (positive) or shrink (negative) on every side.
    pub fn inflate(&self, by: f32) -> Rect {
        Rect::from_edges(
            self.left() - by,
            self.top() - by,
            self.right() + by,
            self.bottom() + by,
        )
    }
}

impl From<gup_text::Bounds> for Rect {
    fn from(b: gup_text::Bounds) -> Self {
        Self::new(b.x, b.y, b.width, b.height)
    }
}
