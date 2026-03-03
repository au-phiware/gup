// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Core math types used throughout the Gup library.
//!
//! This module provides the canonical 2D vector type [`Vec2`] that is used
//! across all subsystems — interaction, event handling, shader functions, and
//! the public API surface. The type is GPU-compatible (`#[repr(C)]`,
//! `bytemuck::Pod`) and implements arithmetic operators for ergonomic
//! coordinate math.

use std::ops::{Add, Div, Mul, Sub};

/// A 2D vector type used throughout Gup for positions, sizes, offsets, and
/// other two-component quantities.
///
/// `Vec2` is the single canonical 2D vector type in the library. It is
/// GPU-compatible (C layout, `bytemuck::Pod`), supports component-wise
/// arithmetic, and converts freely to/from `[f32; 2]`.
///
/// # Examples
///
/// ```
/// use gup::math::Vec2;
///
/// let a = Vec2::new(1.0, 2.0);
/// let b = Vec2::new(3.0, 4.0);
///
/// // Component-wise arithmetic
/// assert_eq!(a + b, Vec2::new(4.0, 6.0));
/// assert_eq!(b - a, Vec2::new(2.0, 2.0));
/// assert_eq!(a * b, Vec2::new(3.0, 8.0));
///
/// // Scalar multiply / divide
/// assert_eq!(a * 2.0, Vec2::new(2.0, 4.0));
/// assert_eq!(b / 2.0, Vec2::new(1.5, 2.0));
///
/// // Conversions
/// let arr: [f32; 2] = a.into();
/// assert_eq!(arr, [1.0, 2.0]);
/// let v: Vec2 = [5.0, 6.0].into();
/// assert_eq!(v, Vec2::new(5.0, 6.0));
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

impl Vec2 {
    /// Creates a new 2D vector.
    ///
    /// # Example
    /// ```
    /// use gup::math::Vec2;
    /// let v = Vec2::new(1.0, 2.0);
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 2.0);
    /// ```
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Creates a vector with both components set to zero.
    ///
    /// # Example
    /// ```
    /// use gup::math::Vec2;
    /// let v = Vec2::zero();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 0.0);
    /// ```
    #[inline]
    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Creates a vector with both components set to one.
    ///
    /// # Example
    /// ```
    /// use gup::math::Vec2;
    /// let v = Vec2::one();
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 1.0);
    /// ```
    #[inline]
    pub const fn one() -> Self {
        Self { x: 1.0, y: 1.0 }
    }
}

// ---------------------------------------------------------------------------
// Conversions: [f32; 2] <-> Vec2
// ---------------------------------------------------------------------------

impl From<[f32; 2]> for Vec2 {
    #[inline]
    fn from(array: [f32; 2]) -> Self {
        Self {
            x: array[0],
            y: array[1],
        }
    }
}

impl From<Vec2> for [f32; 2] {
    #[inline]
    fn from(vec: Vec2) -> Self {
        [vec.x, vec.y]
    }
}

// ---------------------------------------------------------------------------
// Component-wise arithmetic: Vec2 op Vec2
// ---------------------------------------------------------------------------

impl Add for Vec2 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul for Vec2 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
        }
    }
}

impl Div for Vec2 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        Self {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
        }
    }
}

// ---------------------------------------------------------------------------
// Scalar arithmetic: Vec2 op f32  and  f32 op Vec2
// ---------------------------------------------------------------------------

impl Mul<f32> for Vec2 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Mul<Vec2> for f32 {
    type Output = Vec2;
    #[inline]
    fn mul(self, rhs: Vec2) -> Vec2 {
        Vec2 {
            x: self * rhs.x,
            y: self * rhs.y,
        }
    }
}

impl Div<f32> for Vec2 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors() {
        let v = Vec2::new(1.0, 2.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);

        let z = Vec2::zero();
        assert_eq!(z, Vec2::new(0.0, 0.0));

        let o = Vec2::one();
        assert_eq!(o, Vec2::new(1.0, 1.0));
    }

    #[test]
    fn from_array() {
        let v: Vec2 = [3.0, 4.0].into();
        assert_eq!(v, Vec2::new(3.0, 4.0));
    }

    #[test]
    fn into_array() {
        let arr: [f32; 2] = Vec2::new(5.0, 6.0).into();
        assert_eq!(arr, [5.0, 6.0]);
    }

    #[test]
    fn add_vec2() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        assert_eq!(a + b, Vec2::new(4.0, 6.0));
    }

    #[test]
    fn sub_vec2() {
        let a = Vec2::new(5.0, 8.0);
        let b = Vec2::new(2.0, 3.0);
        assert_eq!(a - b, Vec2::new(3.0, 5.0));
    }

    #[test]
    fn mul_vec2() {
        let a = Vec2::new(2.0, 3.0);
        let b = Vec2::new(4.0, 5.0);
        assert_eq!(a * b, Vec2::new(8.0, 15.0));
    }

    #[test]
    fn div_vec2() {
        let a = Vec2::new(10.0, 20.0);
        let b = Vec2::new(2.0, 5.0);
        assert_eq!(a / b, Vec2::new(5.0, 4.0));
    }

    #[test]
    fn mul_scalar() {
        let v = Vec2::new(3.0, 4.0);
        assert_eq!(v * 2.0, Vec2::new(6.0, 8.0));
        assert_eq!(2.0 * v, Vec2::new(6.0, 8.0));
    }

    #[test]
    fn div_scalar() {
        let v = Vec2::new(10.0, 20.0);
        assert_eq!(v / 2.0, Vec2::new(5.0, 10.0));
    }

    #[test]
    fn zero_arithmetic() {
        let v = Vec2::new(5.0, 10.0);
        let z = Vec2::zero();
        assert_eq!(v + z, v);
        assert_eq!(v - z, v);
        assert_eq!(v * z, z);
    }

    #[test]
    fn negative_values() {
        let a = Vec2::new(-1.0, -2.0);
        let b = Vec2::new(3.0, 4.0);
        assert_eq!(a + b, Vec2::new(2.0, 2.0));
        assert_eq!(a * b, Vec2::new(-3.0, -8.0));
    }

    #[test]
    fn bytemuck_pod() {
        // Verify the type is Pod-compatible (this is a compile-time check
        // that also exercises the runtime layout).
        let v = Vec2::new(1.0, 2.0);
        let bytes: &[u8] = bytemuck::bytes_of(&v);
        assert_eq!(bytes.len(), 8);

        let roundtrip: &Vec2 = bytemuck::from_bytes(bytes);
        assert_eq!(*roundtrip, v);
    }

    #[test]
    fn repr_c_layout() {
        assert_eq!(std::mem::size_of::<Vec2>(), 8);
        assert_eq!(std::mem::align_of::<Vec2>(), 4);
        assert_eq!(std::mem::offset_of!(Vec2, x), 0);
        assert_eq!(std::mem::offset_of!(Vec2, y), 4);
    }
}
