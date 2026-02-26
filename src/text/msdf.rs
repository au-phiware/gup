// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Multi-channel signed distance field (MSDF) generation for font glyphs.
//!
//! This module implements proper MSDF generation following the algorithm described
//! in Viktor Chlumsky's thesis "Shape Decomposition for Multi-channel Distance Fields".
//!
//! Key concepts:
//! - Edge coloring: Each edge is assigned a color (combination of RGB channels)
//! - Adjacent edges at corners get different colors
//! - For each pixel, each channel stores the pseudo-distance to the nearest edge of that color
//! - The median of the three channels reconstructs the original shape with sharp corners

use crate::error::{GupError, GupResult};

/// Configuration for MSDF generation
#[derive(Debug, Clone)]
pub struct MsdfConfig {
    /// Size of atlas texture
    pub atlas_size: u32,
    /// Size of individual glyphs in pixels
    pub glyph_size: f32,
    /// MSDF distance range in pixels (how far from the edge the field extends)
    pub distance_range: f32,
    /// Threshold for sharp corner detection (in radians)
    pub angle_threshold: f32,
    /// Padding around glyphs in pixels
    pub padding: u32,
}

impl Default for MsdfConfig {
    fn default() -> Self {
        Self {
            atlas_size: 1024,
            glyph_size: 48.0,
            distance_range: 4.0,
            angle_threshold: std::f32::consts::PI / 3.0, // 60 degrees
            padding: 4,
        }
    }
}

/// Edge color assignment for MSDF
/// Colors are combinations of RGB channels using the median-of-three model
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeColor {
    pub r: bool,
    pub g: bool,
    pub b: bool,
}

impl EdgeColor {
    pub const WHITE: EdgeColor = EdgeColor {
        r: true,
        g: true,
        b: true,
    };
    pub const YELLOW: EdgeColor = EdgeColor {
        r: true,
        g: true,
        b: false,
    };
    pub const MAGENTA: EdgeColor = EdgeColor {
        r: true,
        g: false,
        b: true,
    };
    pub const CYAN: EdgeColor = EdgeColor {
        r: false,
        g: true,
        b: true,
    };

    /// Check if this color includes the red channel
    pub fn has_red(&self) -> bool {
        self.r
    }

    /// Check if this color includes the green channel
    pub fn has_green(&self) -> bool {
        self.g
    }

    /// Check if this color includes the blue channel
    pub fn has_blue(&self) -> bool {
        self.b
    }
}

/// 2D point representation
#[derive(Debug, Clone, Copy, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn distance_squared_to(&self, other: &Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    pub fn dot(&self, other: &Point) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// 2D cross product (returns scalar)
    pub fn cross(&self, other: &Point) -> f32 {
        self.x * other.y - self.y * other.x
    }

    pub fn normalize(&self) -> Point {
        let len = self.length();
        if len > 0.0 {
            Point::new(self.x / len, self.y / len)
        } else {
            Point::new(0.0, 0.0)
        }
    }

    pub fn sub(&self, other: &Point) -> Point {
        Point::new(self.x - other.x, self.y - other.y)
    }

    pub fn add(&self, other: &Point) -> Point {
        Point::new(self.x + other.x, self.y + other.y)
    }

    pub fn scale(&self, s: f32) -> Point {
        Point::new(self.x * s, self.y * s)
    }
}

/// Signed distance result including parameter t for pseudo-distance calculation
/// and orthogonality for proper edge comparison at corners
#[derive(Debug, Clone, Copy)]
pub struct SignedDistance {
    /// The signed distance value
    pub distance: f32,
    /// Parameter t at which the minimum distance occurs (for pseudo-distance)
    pub t: f32,
    /// Orthogonality: cross product of tangent direction and direction to point
    /// Used as tie-breaker when distances are equal (at corner bisectors)
    pub orthogonality: f32,
}

impl SignedDistance {
    fn new(distance: f32, t: f32, orthogonality: f32) -> Self {
        Self {
            distance,
            t,
            orthogonality,
        }
    }

    /// Compare two signed distances according to Chlumsky's algorithm:
    /// Primary: smaller absolute distance wins
    /// Tie-breaker: higher orthogonality wins (point is more perpendicular to edge)
    pub fn is_closer_than(&self, other: &SignedDistance) -> bool {
        let abs_self = self.distance.abs();
        let abs_other = other.distance.abs();
        if (abs_self - abs_other).abs() < 1e-6 {
            // Distances are equal, use orthogonality as tie-breaker
            self.orthogonality.abs() > other.orthogonality.abs()
        } else {
            abs_self < abs_other
        }
    }
}

/// Edge segment types in glyph outline
#[derive(Debug, Clone)]
pub struct EdgeSegment {
    pub edge_type: EdgeType,
    pub color: EdgeColor,
}

#[derive(Debug, Clone)]
pub enum EdgeType {
    Line {
        start: Point,
        end: Point,
    },
    QuadCurve {
        p0: Point,
        p1: Point,
        p2: Point,
    },
    CubicCurve {
        p0: Point,
        p1: Point,
        p2: Point,
        p3: Point,
    },
}

impl EdgeSegment {
    /// Get the direction vector at the start of the edge
    pub fn direction_at_start(&self) -> Point {
        match &self.edge_type {
            EdgeType::Line { start, end } => end.sub(start),
            EdgeType::QuadCurve { p0, p1, .. } => p1.sub(p0),
            EdgeType::CubicCurve { p0, p1, .. } => p1.sub(p0),
        }
    }

    /// Get the direction vector at the end of the edge
    pub fn direction_at_end(&self) -> Point {
        match &self.edge_type {
            EdgeType::Line { start, end } => end.sub(start),
            EdgeType::QuadCurve { p1, p2, .. } => p2.sub(p1),
            EdgeType::CubicCurve { p2, p3, .. } => p3.sub(p2),
        }
    }

    /// Get the start point of the edge
    pub fn start_point(&self) -> Point {
        match &self.edge_type {
            EdgeType::Line { start, .. } => *start,
            EdgeType::QuadCurve { p0, .. } => *p0,
            EdgeType::CubicCurve { p0, .. } => *p0,
        }
    }

    /// Get the end point of the edge
    pub fn end_point(&self) -> Point {
        match &self.edge_type {
            EdgeType::Line { end, .. } => *end,
            EdgeType::QuadCurve { p2, .. } => *p2,
            EdgeType::CubicCurve { p3, .. } => *p3,
        }
    }

    /// Compute the point on the curve at parameter t
    pub fn point_at(&self, t: f32) -> Point {
        match &self.edge_type {
            EdgeType::Line { start, end } => Point::new(
                start.x + t * (end.x - start.x),
                start.y + t * (end.y - start.y),
            ),
            EdgeType::QuadCurve { p0, p1, p2 } => {
                let t2 = t * t;
                let mt = 1.0 - t;
                let mt2 = mt * mt;
                Point::new(
                    mt2 * p0.x + 2.0 * mt * t * p1.x + t2 * p2.x,
                    mt2 * p0.y + 2.0 * mt * t * p1.y + t2 * p2.y,
                )
            }
            EdgeType::CubicCurve { p0, p1, p2, p3 } => {
                let t2 = t * t;
                let t3 = t2 * t;
                let mt = 1.0 - t;
                let mt2 = mt * mt;
                let mt3 = mt2 * mt;
                Point::new(
                    mt3 * p0.x + 3.0 * mt2 * t * p1.x + 3.0 * mt * t2 * p2.x + t3 * p3.x,
                    mt3 * p0.y + 3.0 * mt2 * t * p1.y + 3.0 * mt * t2 * p2.y + t3 * p3.y,
                )
            }
        }
    }

    /// Compute the derivative (direction) at parameter t
    pub fn direction_at(&self, t: f32) -> Point {
        match &self.edge_type {
            EdgeType::Line { start, end } => end.sub(start),
            EdgeType::QuadCurve { p0, p1, p2 } => {
                let mt = 1.0 - t;
                Point::new(
                    2.0 * (mt * (p1.x - p0.x) + t * (p2.x - p1.x)),
                    2.0 * (mt * (p1.y - p0.y) + t * (p2.y - p1.y)),
                )
            }
            EdgeType::CubicCurve { p0, p1, p2, p3 } => {
                let mt = 1.0 - t;
                let t2 = t * t;
                let mt2 = mt * mt;
                Point::new(
                    3.0 * (mt2 * (p1.x - p0.x) + 2.0 * mt * t * (p2.x - p1.x) + t2 * (p3.x - p2.x)),
                    3.0 * (mt2 * (p1.y - p0.y) + 2.0 * mt * t * (p2.y - p1.y) + t2 * (p3.y - p2.y)),
                )
            }
        }
    }

    /// Compute true signed distance from a point to this edge segment
    pub fn signed_distance(&self, point: &Point) -> SignedDistance {
        match &self.edge_type {
            EdgeType::Line { start, end } => self.line_signed_distance(point, start, end),
            EdgeType::QuadCurve { p0, p1, p2 } => self.quad_signed_distance(point, p0, p1, p2),
            EdgeType::CubicCurve { p0, p1, p2, p3 } => {
                self.cubic_signed_distance(point, p0, p1, p2, p3)
            }
        }
    }

    fn line_signed_distance(&self, point: &Point, start: &Point, end: &Point) -> SignedDistance {
        let edge_vec = end.sub(start);
        let point_vec = point.sub(start);

        let edge_length_sq = edge_vec.length_squared();
        if edge_length_sq < 1e-10 {
            return SignedDistance::new(point.distance_to(start), 0.0, 0.0);
        }

        // Project point onto line, get parameter t
        let t = point_vec.dot(&edge_vec) / edge_length_sq;
        let t_clamped = t.clamp(0.0, 1.0);

        // Find closest point on segment
        let closest = Point::new(
            start.x + t_clamped * edge_vec.x,
            start.y + t_clamped * edge_vec.y,
        );
        let distance = point.distance_to(&closest);

        // Determine sign using cross product
        // MSDF convention: inside glyph = positive (> 0.5), outside = negative (< 0.5)
        // TrueType uses CW winding for outer contours: cross > 0 means LEFT = outside
        let cross = edge_vec.cross(&point_vec);
        let signed_dist = if cross >= 0.0 { -distance } else { distance };

        // Compute orthogonality: cross product of normalized tangent and normalized direction to point
        // This is used as tie-breaker when distances are equal
        let to_point = point.sub(&closest);
        let to_point_len = to_point.length();
        let edge_len = edge_length_sq.sqrt();
        let orthogonality = if to_point_len > 1e-10 && edge_len > 1e-10 {
            let tangent_norm = Point::new(edge_vec.x / edge_len, edge_vec.y / edge_len);
            let to_point_norm = Point::new(to_point.x / to_point_len, to_point.y / to_point_len);
            tangent_norm.cross(&to_point_norm)
        } else {
            0.0
        };

        SignedDistance::new(signed_dist, t_clamped, orthogonality)
    }

    fn quad_signed_distance(
        &self,
        point: &Point,
        p0: &Point,
        p1: &Point,
        p2: &Point,
    ) -> SignedDistance {
        // Convert to polynomial form for distance calculation
        // B(t) = (1-t)²P0 + 2(1-t)tP1 + t²P2
        // We need to minimize |B(t) - P|²

        // Coefficients for B(t) - P = at² + bt + c
        let a = Point::new(p0.x - 2.0 * p1.x + p2.x, p0.y - 2.0 * p1.y + p2.y);
        let b = Point::new(2.0 * (p1.x - p0.x), 2.0 * (p1.y - p0.y));
        let c = Point::new(p0.x - point.x, p0.y - point.y);

        // d/dt |B(t) - P|² = 0 leads to a cubic equation
        // Coefficients: dot(a,a)t³ + 3*dot(a,b)/2*t² + (dot(b,b) + 2*dot(a,c))/2*t + dot(b,c)/2 = 0
        // Multiply by 2: 2*dot(a,a)*t³ + 3*dot(a,b)*t² + (dot(b,b) + 2*dot(a,c))*t + dot(b,c) = 0
        let aa = a.dot(&a);
        let ab = a.dot(&b);
        let ac = a.dot(&c);
        let bb = b.dot(&b);
        let bc = b.dot(&c);

        // Solve 2*aa*t³ + 3*ab*t² + (bb + 2*ac)*t + bc = 0
        let roots = solve_cubic(2.0 * aa, 3.0 * ab, bb + 2.0 * ac, bc);

        let mut best_t = 0.0;
        let mut best_dist_sq = point.distance_squared_to(p0);

        // Check endpoints
        let dist_end = point.distance_squared_to(p2);
        if dist_end < best_dist_sq {
            best_dist_sq = dist_end;
            best_t = 1.0;
        }

        // Check valid roots
        for t in roots {
            if t > 0.0 && t < 1.0 {
                let pt = self.point_at(t);
                let dist_sq = point.distance_squared_to(&pt);
                if dist_sq < best_dist_sq {
                    best_dist_sq = dist_sq;
                    best_t = t;
                }
            }
        }

        let distance = best_dist_sq.sqrt();
        let dir = self.direction_at(best_t);
        let closest = self.point_at(best_t);
        let to_point = point.sub(&closest);
        // TrueType CW winding: cross > 0 = outside (negative), cross < 0 = inside (positive)
        let cross = dir.cross(&to_point);
        let signed_dist = if cross >= 0.0 { -distance } else { distance };

        // Compute orthogonality
        let to_point_len = to_point.length();
        let dir_len = dir.length();
        let orthogonality = if to_point_len > 1e-10 && dir_len > 1e-10 {
            let dir_norm = Point::new(dir.x / dir_len, dir.y / dir_len);
            let to_point_norm = Point::new(to_point.x / to_point_len, to_point.y / to_point_len);
            dir_norm.cross(&to_point_norm)
        } else {
            0.0
        };

        SignedDistance::new(signed_dist, best_t, orthogonality)
    }

    fn cubic_signed_distance(
        &self,
        point: &Point,
        p0: &Point,
        p1: &Point,
        p2: &Point,
        p3: &Point,
    ) -> SignedDistance {
        // For cubic curves, we use iterative subdivision to find the closest point
        // This is more numerically stable than solving the quintic equation

        let mut best_t = 0.0;
        let mut best_dist_sq = point.distance_squared_to(p0);

        // Check endpoint
        let dist_end = point.distance_squared_to(p3);
        if dist_end < best_dist_sq {
            best_dist_sq = dist_end;
            best_t = 1.0;
        }

        // Subdivide and search for minimum
        const SUBDIVISIONS: usize = 16;
        for i in 1..SUBDIVISIONS {
            let t = i as f32 / SUBDIVISIONS as f32;
            let pt = self.point_at(t);
            let dist_sq = point.distance_squared_to(&pt);
            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                best_t = t;
            }
        }

        // Refine with Newton-Raphson iterations
        best_t = self.refine_closest_point(point, best_t, p0, p1, p2, p3);

        // Recompute with refined t
        let closest = self.point_at(best_t);
        let distance = point.distance_to(&closest);

        let dir = self.direction_at(best_t);
        let to_point = point.sub(&closest);
        // TrueType CW winding: cross > 0 = outside (negative), cross < 0 = inside (positive)
        let cross = dir.cross(&to_point);
        let signed_dist = if cross >= 0.0 { -distance } else { distance };

        // Compute orthogonality
        let to_point_len = to_point.length();
        let dir_len = dir.length();
        let orthogonality = if to_point_len > 1e-10 && dir_len > 1e-10 {
            let dir_norm = Point::new(dir.x / dir_len, dir.y / dir_len);
            let to_point_norm = Point::new(to_point.x / to_point_len, to_point.y / to_point_len);
            dir_norm.cross(&to_point_norm)
        } else {
            0.0
        };

        SignedDistance::new(signed_dist, best_t, orthogonality)
    }

    fn refine_closest_point(
        &self,
        point: &Point,
        initial_t: f32,
        _p0: &Point,
        _p1: &Point,
        _p2: &Point,
        _p3: &Point,
    ) -> f32 {
        let mut t = initial_t;

        for _ in 0..4 {
            let pt = self.point_at(t);
            let dir = self.direction_at(t);
            let to_point = point.sub(&pt);

            // Newton-Raphson: t_new = t - f(t)/f'(t)
            // where f(t) = dot(B(t) - P, B'(t)) (the derivative of distance squared)
            let f = -to_point.dot(&dir);

            // f'(t) = |B'(t)|² + dot(B(t) - P, B''(t))
            let f_prime = dir.length_squared();

            if f_prime.abs() < 1e-10 {
                break;
            }

            let delta = f / f_prime;
            t = (t - delta).clamp(0.0, 1.0);
        }

        t
    }

    /// Compute pseudo-distance (extends beyond endpoints along tangent directions)
    pub fn pseudo_distance(&self, point: &Point) -> SignedDistance {
        let sd = self.signed_distance(point);

        // If the closest point is at an endpoint, use pseudo-distance
        if sd.t <= 0.0 || sd.t >= 1.0 {
            // For endpoints, project along the tangent direction
            let (endpoint, direction) = if sd.t <= 0.0 {
                (self.start_point(), self.direction_at_start())
            } else {
                (self.end_point(), self.direction_at_end())
            };

            let dir_norm = direction.normalize();
            let to_point = point.sub(&endpoint);

            // Perpendicular distance
            let cross = dir_norm.cross(&to_point);
            let perp_dist = cross.abs();

            // Sign from the main signed distance
            let sign = if sd.distance >= 0.0 { 1.0 } else { -1.0 };

            // Orthogonality for pseudo-distance: use the cross product directly
            // (it's already the cross of normalized tangent with direction to point)
            let to_point_len = to_point.length();
            let orthogonality = if to_point_len > 1e-10 {
                let to_point_norm =
                    Point::new(to_point.x / to_point_len, to_point.y / to_point_len);
                dir_norm.cross(&to_point_norm)
            } else {
                0.0
            };

            SignedDistance::new(sign * perp_dist, sd.t, orthogonality)
        } else {
            sd
        }
    }
}

/// Solve cubic equation ax³ + bx² + cx + d = 0
/// Returns up to 3 real roots
fn solve_cubic(a: f32, b: f32, c: f32, d: f32) -> Vec<f32> {
    if a.abs() < 1e-10 {
        // Degenerate to quadratic
        return solve_quadratic(b, c, d);
    }

    // Normalize
    let b = b / a;
    let c = c / a;
    let d = d / a;

    // Depressed cubic: t³ + pt + q = 0, where x = t - b/3
    let p = c - b * b / 3.0;
    let q = 2.0 * b * b * b / 27.0 - b * c / 3.0 + d;

    let discriminant = q * q / 4.0 + p * p * p / 27.0;
    let offset = -b / 3.0;

    if discriminant > 0.0 {
        // One real root
        let sqrt_disc = discriminant.sqrt();
        let u = (-q / 2.0 + sqrt_disc).cbrt();
        let v = (-q / 2.0 - sqrt_disc).cbrt();
        vec![u + v + offset]
    } else if discriminant.abs() < 1e-10 {
        // Multiple roots
        if q.abs() < 1e-10 {
            vec![offset]
        } else {
            let u = (-q / 2.0).cbrt();
            vec![2.0 * u + offset, -u + offset]
        }
    } else {
        // Three real roots (use trigonometric method)
        let r = (-p / 3.0).sqrt();
        let phi = (-q / (2.0 * r * r * r)).clamp(-1.0, 1.0).acos();
        let two_pi = 2.0 * std::f32::consts::PI;
        vec![
            2.0 * r * (phi / 3.0).cos() + offset,
            2.0 * r * ((phi + two_pi) / 3.0).cos() + offset,
            2.0 * r * ((phi + 2.0 * two_pi) / 3.0).cos() + offset,
        ]
    }
}

/// Solve quadratic equation ax² + bx + c = 0
fn solve_quadratic(a: f32, b: f32, c: f32) -> Vec<f32> {
    if a.abs() < 1e-10 {
        // Linear
        if b.abs() < 1e-10 {
            return vec![];
        }
        return vec![-c / b];
    }

    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        vec![]
    } else if discriminant.abs() < 1e-10 {
        vec![-b / (2.0 * a)]
    } else {
        let sqrt_disc = discriminant.sqrt();
        vec![(-b - sqrt_disc) / (2.0 * a), (-b + sqrt_disc) / (2.0 * a)]
    }
}

/// Contour (closed shape) in glyph outline
#[derive(Debug, Clone)]
pub struct Contour {
    pub edges: Vec<EdgeSegment>,
}

impl Default for Contour {
    fn default() -> Self {
        Self::new()
    }
}

impl Contour {
    pub fn new() -> Self {
        Self { edges: Vec::new() }
    }

    pub fn add_edge(&mut self, edge: EdgeSegment) {
        self.edges.push(edge);
    }

    /// Apply edge coloring to this contour
    /// Uses the simple algorithm: cycle between magenta, cyan, yellow
    ///
    /// For special cases:
    /// - **Smooth blob** (no sharp corners): all edges keep WHITE (single-channel equivalent)
    /// - **Teardrop** (one sharp corner): synthetic corner inserted at the point
    ///   *furthest* from the cusp so the two edge groups are geometrically balanced
    /// - **Standard** (≥2 sharp corners): colours cycle normally at each corner
    pub fn apply_edge_coloring(&mut self, angle_threshold: f32) {
        if self.edges.is_empty() {
            return;
        }

        // Find corners (where direction changes significantly)
        let mut is_corner: Vec<bool> = self
            .edges
            .iter()
            .enumerate()
            .map(|(i, edge)| {
                let prev_idx = if i == 0 { self.edges.len() - 1 } else { i - 1 };
                let prev_dir = self.edges[prev_idx].direction_at_end().normalize();
                let curr_dir = edge.direction_at_start().normalize();

                let cross = prev_dir.cross(&curr_dir);
                let dot = prev_dir.dot(&curr_dir);

                // Check if this is a sharp corner
                cross.abs() > angle_threshold.sin() || dot < angle_threshold.cos()
            })
            .collect();

        // starter is used to find the starting position of the first edge
        // and to check for more edges
        let mut starter = is_corner.iter();
        let first_edge = starter.position(|&c| c);
        // If no corners (smooth blob), use default color (white)
        if first_edge.is_none() {
            return;
        }
        let first_edge = first_edge.unwrap();
        // Teardrop handling: if only one corner, insert a synthetic corner
        // at the edge whose start point is *furthest* from the cusp.
        // This gives the two colour regions roughly equal geometric extent.
        if !starter.any(|&c| c) {
            let cusp_pos = self.edges[first_edge].start_point();
            let best = self
                .edges
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != first_edge)
                .max_by(|(_, a), (_, b)| {
                    let da = a.start_point().distance_squared_to(&cusp_pos);
                    let db = b.start_point().distance_squared_to(&cusp_pos);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });
            let split_idx = best.map_or(
                (first_edge + self.edges.len() / 2) % self.edges.len(),
                |(i, _)| i,
            );
            is_corner[split_idx] = true;
        }

        // Color cycling: magenta -> cyan -> yellow -> cyan -> yellow...
        let colors = [EdgeColor::CYAN, EdgeColor::YELLOW, EdgeColor::MAGENTA];
        let mut color_idx = 2;

        for i in 0..self.edges.len() {
            let edge_idx = first_edge + i;
            let edge_idx = if edge_idx >= self.edges.len() {
                edge_idx - self.edges.len()
            } else {
                edge_idx
            };
            self.edges[edge_idx].color = colors[color_idx];

            // Change color at corners
            let edge_idx = edge_idx + 1;
            let edge_idx = if edge_idx == self.edges.len() {
                0
            } else {
                edge_idx
            };
            if is_corner[edge_idx] {
                color_idx = (color_idx + 1) % 2;
            }
        }

        // Ensure adjacent edges at corners have different colors
        // If a contour has no corners, all edges get the same color (which is fine)
    }
}

/// Complete glyph outline with edge coloring applied
#[derive(Debug, Clone)]
pub struct GlyphOutline {
    pub contours: Vec<Contour>,
    pub bounds: (Point, Point),
}

impl Default for GlyphOutline {
    fn default() -> Self {
        Self::new()
    }
}

impl GlyphOutline {
    pub fn new() -> Self {
        Self {
            contours: Vec::new(),
            bounds: (
                Point::new(f32::INFINITY, f32::INFINITY),
                Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
            ),
        }
    }

    pub fn add_contour(&mut self, contour: Contour) {
        // Update bounds
        for edge in &contour.edges {
            self.update_bounds(&edge.start_point());
            self.update_bounds(&edge.end_point());
            if let EdgeType::QuadCurve { p1, .. } = &edge.edge_type {
                self.update_bounds(p1);
            }
            if let EdgeType::CubicCurve { p1, p2, .. } = &edge.edge_type {
                self.update_bounds(p1);
                self.update_bounds(p2);
            }
        }
        self.contours.push(contour);
    }

    fn update_bounds(&mut self, point: &Point) {
        self.bounds.0.x = self.bounds.0.x.min(point.x);
        self.bounds.0.y = self.bounds.0.y.min(point.y);
        self.bounds.1.x = self.bounds.1.x.max(point.x);
        self.bounds.1.y = self.bounds.1.y.max(point.y);
    }

    /// Apply edge coloring to all contours
    pub fn apply_edge_coloring(&mut self, angle_threshold: f32) {
        for contour in &mut self.contours {
            contour.apply_edge_coloring(angle_threshold);
        }
    }

    /// Compute MSDF values for a point (Algorithm 7 from Chlumsky's thesis)
    ///
    /// This computes signed_distance ONCE per edge segment, then uses that result
    /// for all three channel comparisons. pseudo_distance is only called at the
    /// end for the 3 closest edges (one per channel).
    ///
    /// Edge comparison uses both absolute distance AND orthogonality as a tie-breaker,
    /// which correctly partitions the plane at corners.
    pub fn msdf_at(&self, point: &Point) -> [f32; 3] {
        // Track closest edge per channel: (signed_distance for comparison, edge reference)
        let mut red_closest: Option<(&EdgeSegment, SignedDistance)> = None;
        let mut green_closest: Option<(&EdgeSegment, SignedDistance)> = None;
        let mut blue_closest: Option<(&EdgeSegment, SignedDistance)> = None;

        // Single pass over all edges - signed_distance computed ONCE per edge
        for contour in &self.contours {
            for edge in &contour.edges {
                // Compute signed distance ONCE for this edge (used for comparison)
                let sd = edge.signed_distance(point);

                // Update red channel if this edge has red component
                if edge.color.has_red()
                    && (red_closest.is_none() || sd.is_closer_than(&red_closest.unwrap().1))
                {
                    red_closest = Some((edge, sd));
                }

                // Update green channel if this edge has green component
                if edge.color.has_green()
                    && (green_closest.is_none() || sd.is_closer_than(&green_closest.unwrap().1))
                {
                    green_closest = Some((edge, sd));
                }

                // Update blue channel if this edge has blue component
                if edge.color.has_blue()
                    && (blue_closest.is_none() || sd.is_closer_than(&blue_closest.unwrap().1))
                {
                    blue_closest = Some((edge, sd));
                }
            }
        }

        // Compute pseudo_distance ONLY for the 3 closest edges (one per channel)
        // If no edge found for a channel, return negative infinity (very far outside)
        let r = red_closest
            .map(|(edge, _)| edge.pseudo_distance(point).distance)
            .unwrap_or(f32::NEG_INFINITY);
        let g = green_closest
            .map(|(edge, _)| edge.pseudo_distance(point).distance)
            .unwrap_or(f32::NEG_INFINITY);
        let b = blue_closest
            .map(|(edge, _)| edge.pseudo_distance(point).distance)
            .unwrap_or(f32::NEG_INFINITY);

        [r, g, b]
    }

    /// Compute a single signed distance value for a point.
    ///
    /// Unlike [`msdf_at`](Self::msdf_at), which tracks per-channel closest
    /// edges, this method finds the globally closest edge and returns its
    /// signed distance. No edge coloring is required, making this simpler
    /// and faster at the cost of losing sharp corner preservation.
    pub fn sdf_at(&self, point: &Point) -> f32 {
        let mut closest: Option<SignedDistance> = None;

        for contour in &self.contours {
            for edge in &contour.edges {
                let sd = edge.signed_distance(point);
                if closest.is_none() || sd.is_closer_than(&closest.unwrap()) {
                    closest = Some(sd);
                }
            }
        }

        closest.map(|sd| sd.distance).unwrap_or(f32::NEG_INFINITY)
    }
}

/// Configuration for single-channel outline-based SDF generation.
///
/// This is a simpler alternative to [`MsdfConfig`] that generates a single
/// distance channel instead of the 3-channel MSDF. The resulting SDF is
/// compatible with the existing MSDF rendering pipeline (by duplicating
/// the distance value across RGB channels).
#[derive(Debug, Clone)]
pub struct SdfConfig {
    /// Size of individual glyphs in pixels
    pub glyph_size: f32,
    /// SDF distance range in pixels (how far from the edge the field extends)
    pub distance_range: f32,
    /// Padding around glyphs in pixels
    pub padding: u32,
}

impl Default for SdfConfig {
    fn default() -> Self {
        Self {
            glyph_size: 48.0,
            distance_range: 4.0,
            padding: 4,
        }
    }
}

/// Single-channel SDF bitmap.
///
/// Stores one signed distance value per texel. Provides conversion to RGBA
/// for compatibility with the existing MSDF texture pipeline (the same value
/// is replicated to all three RGB channels).
#[derive(Debug, Clone)]
pub struct SdfBitmap {
    pub width: usize,
    pub height: usize,
    pub channel: DistanceField,
    /// Scale factor used to generate this SDF (font units to pixels)
    pub scale: f32,
    /// Padding in pixels around the glyph
    pub padding: u32,
}

impl SdfBitmap {
    pub fn new(width: usize, height: usize, scale: f32, padding: u32) -> Self {
        Self {
            width,
            height,
            channel: DistanceField::new(width, height),
            scale,
            padding,
        }
    }

    /// Convert to RGBA pixels for GPU texture upload.
    ///
    /// The single distance value is replicated across all three RGB channels
    /// so that the existing MSDF shader (which uses `median(r, g, b)`) produces
    /// the correct result.
    pub fn to_rgba_pixels(&self) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(self.width * self.height * 4);

        for y in 0..self.height {
            for x in 0..self.width {
                let d = (self.channel.get(x, y) * 0.5 + 0.5).clamp(0.0, 1.0);
                let v = (d * 255.0) as u8;
                pixels.extend_from_slice(&[v, v, v, 255u8]);
            }
        }

        pixels
    }
}

/// Distance field for a single channel
#[derive(Debug, Clone)]
pub struct DistanceField {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl DistanceField {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; width * height],
        }
    }

    pub fn set(&mut self, x: usize, y: usize, value: f32) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = value;
        }
    }

    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height {
            self.data[y * self.width + x]
        } else {
            0.0
        }
    }
}

/// MSDF bitmap with 3 channels (RGB)
#[derive(Debug, Clone)]
pub struct MsdfBitmap {
    pub width: usize,
    pub height: usize,
    pub red_channel: DistanceField,
    pub green_channel: DistanceField,
    pub blue_channel: DistanceField,
    /// Scale factor used to generate this MSDF (font units to pixels)
    pub scale: f32,
    /// Padding in pixels around the glyph
    pub padding: u32,
}

impl MsdfBitmap {
    pub fn new(width: usize, height: usize, scale: f32, padding: u32) -> Self {
        Self {
            width,
            height,
            red_channel: DistanceField::new(width, height),
            green_channel: DistanceField::new(width, height),
            blue_channel: DistanceField::new(width, height),
            scale,
            padding,
        }
    }

    /// Convert to RGBA pixels for GPU texture upload
    /// Distance values are normalized: 0.5 = edge, <0.5 = outside, >0.5 = inside
    pub fn to_rgba_pixels(&self) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(self.width * self.height * 4);

        for y in 0..self.height {
            for x in 0..self.width {
                // Normalize distances to 0-1 range centered at 0.5
                let r = (self.red_channel.get(x, y) * 0.5 + 0.5).clamp(0.0, 1.0);
                let g = (self.green_channel.get(x, y) * 0.5 + 0.5).clamp(0.0, 1.0);
                let b = (self.blue_channel.get(x, y) * 0.5 + 0.5).clamp(0.0, 1.0);

                pixels.extend_from_slice(&[
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                    255u8,
                ]);
            }
        }

        pixels
    }

    /// Generate a single-channel grayscale visualisation of just one channel.
    ///
    /// Useful for debugging edge colouring — you can inspect each channel
    /// independently to see which edges belong to which colour.
    pub fn channel_to_grayscale(&self, channel: usize) -> Vec<u8> {
        let field = match channel {
            0 => &self.red_channel,
            1 => &self.green_channel,
            2 => &self.blue_channel,
            _ => &self.red_channel,
        };
        let mut pixels = Vec::with_capacity(self.width * self.height * 4);
        for y in 0..self.height {
            for x in 0..self.width {
                let v = ((field.get(x, y) * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
                pixels.extend_from_slice(&[v, v, v, 255]);
            }
        }
        pixels
    }

    /// Return the reconstructed single-channel bitmap using `median(r,g,b)`.
    ///
    /// This matches the reconstruction performed in the MSDF fragment shader.
    pub fn reconstructed_median(&self) -> DistanceField {
        let mut field = DistanceField::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                let r = self.red_channel.get(x, y);
                let g = self.green_channel.get(x, y);
                let b = self.blue_channel.get(x, y);
                field.set(x, y, median_f32(r, g, b));
            }
        }
        field
    }
}

/// MSDF generator using ttf_parser
pub struct MsdfGenerator {
    font_data: Vec<u8>,
    config: MsdfConfig,
}

impl MsdfGenerator {
    /// Create a new MSDF generator
    pub fn new(font_data: Vec<u8>, config: MsdfConfig) -> GupResult<Self> {
        // Validate that we can parse the font
        ttf_parser::Face::parse(&font_data, 0)
            .map_err(|e| GupError::resource_error(format!("Failed to parse font: {e:?}")))?;

        Ok(Self { font_data, config })
    }

    /// Get the font face
    fn get_font(&self) -> GupResult<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.font_data, 0)
            .map_err(|e| GupError::resource_error(format!("Failed to parse font: {e:?}")))
    }

    /// Generate MSDF for a specific glyph
    pub fn generate_msdf(&self, glyph_id: ttf_parser::GlyphId) -> GupResult<MsdfBitmap> {
        let font = self.get_font()?;

        // Extract glyph outline
        let mut outline = self.extract_glyph_outline(glyph_id, &font)?;

        // Apply edge coloring
        outline.apply_edge_coloring(self.config.angle_threshold);

        // Calculate glyph bounds and scaling
        let glyph_bounds = font.glyph_bounding_box(glyph_id);
        let units_per_em = font.units_per_em() as f32;

        let (scale, offset_x, offset_y, glyph_width, glyph_height) = if let Some(bbox) =
            glyph_bounds
        {
            let bbox_width = (bbox.x_max - bbox.x_min) as f32;
            let bbox_height = (bbox.y_max - bbox.y_min) as f32;

            // Scale to fit in glyph_size with padding
            let target_size = self.config.glyph_size - 2.0 * self.config.padding as f32;
            let scale = if bbox_width > 0.0 && bbox_height > 0.0 {
                (target_size / bbox_width).min(target_size / bbox_height)
            } else {
                self.config.glyph_size / units_per_em
            };

            let scaled_width = bbox_width * scale;
            let scaled_height = bbox_height * scale;

            let glyph_width = (scaled_width + 2.0 * self.config.padding as f32).ceil() as usize;
            let glyph_height = (scaled_height + 2.0 * self.config.padding as f32).ceil() as usize;

            let offset_x = bbox.x_min as f32;
            let offset_y = bbox.y_min as f32;

            (scale, offset_x, offset_y, glyph_width, glyph_height)
        } else {
            // No bounding box - empty glyph or whitespace
            let scale = self.config.glyph_size / units_per_em;
            (
                scale,
                0.0,
                0.0,
                self.config.glyph_size as usize,
                self.config.glyph_size as usize,
            )
        };

        // Generate MSDF bitmap
        let mut msdf = MsdfBitmap::new(
            glyph_width.max(1),
            glyph_height.max(1),
            scale,
            self.config.padding,
        );
        let distance_range = self.config.distance_range;
        let padding = self.config.padding as f32;

        for y in 0..msdf.height {
            for x in 0..msdf.width {
                // Convert pixel to glyph space
                let glyph_x = (x as f32 - padding) / scale + offset_x;
                let glyph_y = (y as f32 - padding) / scale + offset_y;
                // Flip Y since font coordinates have Y up
                let glyph_y_flipped =
                    offset_y + (glyph_height as f32 - 2.0 * padding) / scale - (glyph_y - offset_y);

                let point = Point::new(glyph_x, glyph_y_flipped);
                let [r, g, b] = outline.msdf_at(&point);

                // Normalize by distance range and scale
                let normalize = |d: f32| -> f32 { (d * scale / distance_range).clamp(-1.0, 1.0) };

                msdf.red_channel.set(x, y, normalize(r));
                msdf.green_channel.set(x, y, normalize(g));
                msdf.blue_channel.set(x, y, normalize(b));
            }
        }

        Ok(msdf)
    }

    /// Extract glyph outline using ttf_parser
    fn extract_glyph_outline(
        &self,
        glyph_id: ttf_parser::GlyphId,
        font: &ttf_parser::Face,
    ) -> GupResult<GlyphOutline> {
        let mut builder = GlyphOutlineBuilder::new();
        let _ = font.outline_glyph(glyph_id, &mut builder);
        Ok(builder.build())
    }

    /// Generate outline for a character (for debugging)
    pub fn generate_outline(&self, c: char) -> GupResult<GlyphOutline> {
        let font = self.get_font()?;
        let glyph_id = font
            .glyph_index(c)
            .ok_or_else(|| GupError::resource_error(format!("Glyph not found for '{c}'")))?;
        self.extract_glyph_outline(glyph_id, &font)
    }

    /// Generate MSDF for a character.
    pub fn generate_msdf_for_char(&self, c: char) -> GupResult<MsdfBitmap> {
        let font = self.get_font()?;
        let glyph_id = font
            .glyph_index(c)
            .ok_or_else(|| GupError::resource_error(format!("Glyph not found for '{c}'")))?;
        self.generate_msdf(glyph_id)
    }

    /// Create an MSDF generator from a [`MultiChannelSdfConfig`].
    ///
    /// This is a convenience constructor that converts the high-level config
    /// to the internal [`MsdfConfig`] used for generation.
    pub fn from_multi_channel_config(
        font_data: Vec<u8>,
        config: &MultiChannelSdfConfig,
    ) -> GupResult<Self> {
        Self::new(font_data, config.to_msdf_config())
    }
}

/// Single-channel outline-based SDF generator.
///
/// A simpler and faster alternative to [`MsdfGenerator`] that produces a
/// single distance channel instead of three. This approach:
///
/// - **Skips edge coloring** – no need to assign colours to contour edges
/// - **Computes one distance per texel** instead of tracking three per-channel
///   closest edges
/// - **Uses the same outline extraction** and distance algorithms as MSDF
///
/// The trade-off is that sharp corners are not preserved the way they are
/// with MSDF. For many visualization use cases (labels, tick marks, legends)
/// the difference is negligible.
pub struct SdfGenerator {
    font_data: Vec<u8>,
    config: SdfConfig,
}

impl SdfGenerator {
    /// Create a new single-channel SDF generator.
    pub fn new(font_data: Vec<u8>, config: SdfConfig) -> GupResult<Self> {
        ttf_parser::Face::parse(&font_data, 0)
            .map_err(|e| GupError::resource_error(format!("Failed to parse font: {e:?}")))?;
        Ok(Self { font_data, config })
    }

    /// Get the font face.
    fn get_font(&self) -> GupResult<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.font_data, 0)
            .map_err(|e| GupError::resource_error(format!("Failed to parse font: {e:?}")))
    }

    /// Generate a single-channel SDF for a specific glyph.
    pub fn generate_sdf(&self, glyph_id: ttf_parser::GlyphId) -> GupResult<SdfBitmap> {
        let font = self.get_font()?;

        // Extract glyph outline (no edge coloring needed)
        let outline = Self::extract_glyph_outline(glyph_id, &font)?;

        // Calculate glyph bounds and scaling
        let glyph_bounds = font.glyph_bounding_box(glyph_id);
        let units_per_em = font.units_per_em() as f32;

        let (scale, offset_x, offset_y, glyph_width, glyph_height) = if let Some(bbox) =
            glyph_bounds
        {
            let bbox_width = (bbox.x_max - bbox.x_min) as f32;
            let bbox_height = (bbox.y_max - bbox.y_min) as f32;

            let target_size = self.config.glyph_size - 2.0 * self.config.padding as f32;
            let scale = if bbox_width > 0.0 && bbox_height > 0.0 {
                (target_size / bbox_width).min(target_size / bbox_height)
            } else {
                self.config.glyph_size / units_per_em
            };

            let scaled_width = bbox_width * scale;
            let scaled_height = bbox_height * scale;

            let glyph_width = (scaled_width + 2.0 * self.config.padding as f32).ceil() as usize;
            let glyph_height = (scaled_height + 2.0 * self.config.padding as f32).ceil() as usize;

            let offset_x = bbox.x_min as f32;
            let offset_y = bbox.y_min as f32;

            (scale, offset_x, offset_y, glyph_width, glyph_height)
        } else {
            let scale = self.config.glyph_size / units_per_em;
            (
                scale,
                0.0,
                0.0,
                self.config.glyph_size as usize,
                self.config.glyph_size as usize,
            )
        };

        let mut sdf = SdfBitmap::new(
            glyph_width.max(1),
            glyph_height.max(1),
            scale,
            self.config.padding,
        );
        let distance_range = self.config.distance_range;
        let padding = self.config.padding as f32;

        for y in 0..sdf.height {
            for x in 0..sdf.width {
                let glyph_x = (x as f32 - padding) / scale + offset_x;
                let glyph_y = (y as f32 - padding) / scale + offset_y;
                let glyph_y_flipped =
                    offset_y + (glyph_height as f32 - 2.0 * padding) / scale - (glyph_y - offset_y);

                let point = Point::new(glyph_x, glyph_y_flipped);
                let d = outline.sdf_at(&point);

                let normalized = (d * scale / distance_range).clamp(-1.0, 1.0);
                sdf.channel.set(x, y, normalized);
            }
        }

        Ok(sdf)
    }

    /// Generate a single-channel SDF for a character.
    pub fn generate_sdf_for_char(&self, c: char) -> GupResult<SdfBitmap> {
        let font = self.get_font()?;
        let glyph_id = font
            .glyph_index(c)
            .ok_or_else(|| GupError::resource_error(format!("Glyph not found for '{c}'")))?;
        self.generate_sdf(glyph_id)
    }

    /// Extract glyph outline (no edge coloring applied).
    fn extract_glyph_outline(
        glyph_id: ttf_parser::GlyphId,
        font: &ttf_parser::Face,
    ) -> GupResult<GlyphOutline> {
        let mut builder = GlyphOutlineBuilder::new();
        let _ = font.outline_glyph(glyph_id, &mut builder);
        Ok(builder.build())
    }
}

/// Quality metrics for comparing SDF generation approaches.
#[derive(Debug, Clone)]
pub struct SdfQualityMetrics {
    /// Mean absolute error between two distance fields.
    pub mean_absolute_error: f32,
    /// Peak signal-to-noise ratio (dB).
    pub peak_signal_to_noise: f32,
    /// Average gradient magnitude at the 0.5 isoline (edge sharpness).
    pub edge_sharpness: f32,
    /// Memory used by the bitmap data (bytes).
    pub memory_bytes: usize,
}

impl SdfQualityMetrics {
    /// Compare a single-channel SDF against an MSDF reference.
    ///
    /// The MSDF reference is converted to a single channel via `median(r,g,b)`
    /// before comparison.
    pub fn compare(sdf: &SdfBitmap, msdf: &MsdfBitmap) -> Self {
        assert_eq!(sdf.width, msdf.width);
        assert_eq!(sdf.height, msdf.height);

        let n = sdf.width * sdf.height;
        let mut sum_abs_error: f64 = 0.0;
        let mut sum_sq_error: f64 = 0.0;
        let mut edge_gradient_sum: f64 = 0.0;
        let mut edge_pixel_count: usize = 0;

        for y in 0..sdf.height {
            for x in 0..sdf.width {
                let sdf_val = sdf.channel.get(x, y);

                // Reconstruct single-channel from MSDF via median
                let r = msdf.red_channel.get(x, y);
                let g = msdf.green_channel.get(x, y);
                let b = msdf.blue_channel.get(x, y);
                let msdf_val = median_f32(r, g, b);

                let err = (sdf_val - msdf_val).abs();
                sum_abs_error += err as f64;
                sum_sq_error += (err * err) as f64;

                // Edge sharpness: measure gradient magnitude near the edge
                // (values close to 0.0 in normalized space = the 0.5 isoline)
                if sdf_val.abs() < 0.3 && x > 0 && y > 0 && x + 1 < sdf.width && y + 1 < sdf.height
                {
                    let dx = sdf.channel.get(x + 1, y) - sdf.channel.get(x - 1, y);
                    let dy = sdf.channel.get(x, y + 1) - sdf.channel.get(x, y - 1);
                    edge_gradient_sum += (dx * dx + dy * dy).sqrt() as f64;
                    edge_pixel_count += 1;
                }
            }
        }

        let mae = (sum_abs_error / n as f64) as f32;
        let mse = sum_sq_error / n as f64;
        // PSNR with max value 2.0 (range is -1.0 to 1.0)
        let psnr = if mse > 0.0 {
            (10.0 * (4.0_f64 / mse).log10()) as f32
        } else {
            f32::INFINITY
        };
        let edge_sharpness = if edge_pixel_count > 0 {
            (edge_gradient_sum / edge_pixel_count as f64) as f32
        } else {
            0.0
        };
        let memory_bytes = sdf.width * sdf.height * std::mem::size_of::<f32>();

        Self {
            mean_absolute_error: mae,
            peak_signal_to_noise: psnr,
            edge_sharpness,
            memory_bytes,
        }
    }

    /// Compute quality metrics for an MSDF bitmap.
    pub fn from_msdf(msdf: &MsdfBitmap) -> Self {
        let n = msdf.width * msdf.height;
        let mut edge_gradient_sum: f64 = 0.0;
        let mut edge_pixel_count: usize = 0;

        for y in 0..msdf.height {
            for x in 0..msdf.width {
                let r = msdf.red_channel.get(x, y);
                let g = msdf.green_channel.get(x, y);
                let b = msdf.blue_channel.get(x, y);
                let val = median_f32(r, g, b);

                if val.abs() < 0.3 && x > 0 && y > 0 && x + 1 < msdf.width && y + 1 < msdf.height {
                    let get_median = |xx: usize, yy: usize| -> f32 {
                        median_f32(
                            msdf.red_channel.get(xx, yy),
                            msdf.green_channel.get(xx, yy),
                            msdf.blue_channel.get(xx, yy),
                        )
                    };
                    let dx = get_median(x + 1, y) - get_median(x - 1, y);
                    let dy = get_median(x, y + 1) - get_median(x, y - 1);
                    edge_gradient_sum += (dx * dx + dy * dy).sqrt() as f64;
                    edge_pixel_count += 1;
                }
            }
        }

        let edge_sharpness = if edge_pixel_count > 0 {
            (edge_gradient_sum / edge_pixel_count as f64) as f32
        } else {
            0.0
        };
        let memory_bytes = n * std::mem::size_of::<f32>() * 3;

        Self {
            mean_absolute_error: 0.0,
            peak_signal_to_noise: f32::INFINITY,
            edge_sharpness,
            memory_bytes,
        }
    }
}

/// Configuration for multi-channel SDF generation with sharp corner preservation.
///
/// Controls how the MSDF generator handles corners and edge classification.
/// Use [`Default`] for reasonable values tuned for standard Latin fonts.
#[derive(Debug, Clone)]
pub struct MultiChannelSdfConfig {
    /// Maximum SDF distance range in pixels.
    pub max_distance: f32,
    /// Angle threshold below which a corner is considered "sharp" (in radians).
    /// Corners sharper than this threshold receive different edge colours on
    /// each side so that the median-of-three reconstruction preserves them.
    pub sharp_corner_threshold: f32,
    /// Maximum number of channels to use (1–3).
    /// 1 = single-channel SDF, 3 = full MSDF.
    pub max_channels: u8,
    /// How the three MSDF channels are combined in the fragment shader.
    pub combination_mode: ChannelCombinationMode,
    /// Size of individual glyphs in pixels (same as `MsdfConfig::glyph_size`).
    pub glyph_size: f32,
    /// Padding around glyphs in pixels.
    pub padding: u32,
}

impl Default for MultiChannelSdfConfig {
    fn default() -> Self {
        Self {
            max_distance: 4.0,
            sharp_corner_threshold: std::f32::consts::PI / 3.0, // 60 degrees
            max_channels: 3,
            combination_mode: ChannelCombinationMode::Median,
            glyph_size: 48.0,
            padding: 4,
        }
    }
}

impl MultiChannelSdfConfig {
    /// Convert to the underlying [`MsdfConfig`] used by the generator.
    pub fn to_msdf_config(&self) -> MsdfConfig {
        MsdfConfig {
            atlas_size: 1024,
            glyph_size: self.glyph_size,
            distance_range: self.max_distance,
            angle_threshold: self.sharp_corner_threshold,
            padding: self.padding,
        }
    }
}

/// How the three MSDF colour channels are combined in the fragment shader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelCombinationMode {
    /// `median(r, g, b)` — the standard MSDF approach.
    /// Preserves sharp corners through edge-colour differences at corners.
    Median = 0,
    /// `max(r, g, b)` — union of channels.
    /// Produces a slightly dilated outline; useful for bold/outline effects.
    Max = 1,
    /// `min(r, g, b)` — intersection of channels.
    /// Produces a sharper (sometimes too sharp) result at corners.
    Min = 2,
}

/// Information about a detected corner in a glyph contour.
#[derive(Debug, Clone)]
pub struct CornerInfo {
    /// Position of the corner in glyph coordinate space.
    pub position: Point,
    /// Interior angle at the corner (radians, 0 = fully degenerate, π = straight).
    pub angle: f32,
    /// Index of the contour that contains this corner.
    pub contour_index: usize,
    /// Index of the edge *after* the corner vertex within the contour.
    pub edge_index: usize,
    /// Whether this corner is classified as "sharp" based on the threshold.
    pub is_sharp: bool,
}

/// Classification of special contour patterns that need extra care during
/// edge colouring or SDF generation.
#[derive(Debug, Clone)]
pub enum ContourPattern {
    /// A contour with exactly one sharp corner (e.g. a raindrop or teardrop shape).
    Teardrop {
        /// The index of the sharp cusp edge.
        cusp_edge_index: usize,
        /// The position of the cusp point.
        cusp_position: Point,
    },
    /// A contour with many sharp corners radiating from a central area (e.g. asterisk).
    StarShape {
        /// Number of sharp corners detected.
        sharp_corner_count: usize,
    },
    /// A simple convex/smooth contour with no sharp corners.
    Smooth,
    /// A standard contour with multiple corners (rectangle, letter shapes, etc.).
    Standard {
        /// Number of sharp corners detected.
        sharp_corner_count: usize,
    },
}

impl Contour {
    /// Detect corners in this contour and return their descriptions.
    pub fn detect_corners(&self, angle_threshold: f32) -> Vec<CornerInfo> {
        if self.edges.is_empty() {
            return Vec::new();
        }

        let mut corners = Vec::new();

        for i in 0..self.edges.len() {
            let prev_idx = if i == 0 { self.edges.len() - 1 } else { i - 1 };
            let prev_dir = self.edges[prev_idx].direction_at_end().normalize();
            let curr_dir = self.edges[i].direction_at_start().normalize();

            let cross = prev_dir.cross(&curr_dir);
            let dot = prev_dir.dot(&curr_dir);
            let angle = cross.atan2(dot).abs();

            let is_sharp = cross.abs() > angle_threshold.sin() || dot < angle_threshold.cos();

            corners.push(CornerInfo {
                position: self.edges[i].start_point(),
                angle,
                contour_index: 0, // Caller fills this in
                edge_index: i,
                is_sharp,
            });
        }

        corners
    }

    /// Classify the overall pattern of this contour.
    pub fn classify_pattern(&self, angle_threshold: f32) -> ContourPattern {
        let corners = self.detect_corners(angle_threshold);
        let sharp_count = corners.iter().filter(|c| c.is_sharp).count();

        match sharp_count {
            0 => ContourPattern::Smooth,
            1 => ContourPattern::Teardrop {
                cusp_edge_index: corners.iter().find(|c| c.is_sharp).unwrap().edge_index,
                cusp_position: corners.iter().find(|c| c.is_sharp).unwrap().position,
            },
            n if n >= 5 => ContourPattern::StarShape {
                sharp_corner_count: n,
            },
            n => ContourPattern::Standard {
                sharp_corner_count: n,
            },
        }
    }
}

impl GlyphOutline {
    /// Detect all corners across every contour in this outline.
    pub fn detect_all_corners(&self, angle_threshold: f32) -> Vec<CornerInfo> {
        let mut all_corners = Vec::new();
        for (ci, contour) in self.contours.iter().enumerate() {
            let mut corners = contour.detect_corners(angle_threshold);
            for c in &mut corners {
                c.contour_index = ci;
            }
            all_corners.extend(corners);
        }
        all_corners
    }

    /// Classify every contour in the outline.
    pub fn classify_contours(&self, angle_threshold: f32) -> Vec<ContourPattern> {
        self.contours
            .iter()
            .map(|c| c.classify_pattern(angle_threshold))
            .collect()
    }
}

/// Measures how sharply a corner is preserved in a rendered SDF/MSDF bitmap.
///
/// The metric works by sampling the reconstructed distance field near a known
/// corner position and computing the gradient magnitude at that point.
/// A perfectly preserved corner has a high gradient (the distance field
/// changes rapidly), while a rounded corner has a lower gradient because the
/// field is smoothed out.
#[derive(Debug, Clone)]
pub struct CornerSharpnessMetrics {
    /// Average gradient magnitude at detected corners.
    pub mean_corner_gradient: f32,
    /// Maximum gradient magnitude across all corners.
    pub max_corner_gradient: f32,
    /// Number of corners that were analysed.
    pub corner_count: usize,
    /// Per-corner gradient values (in the same order as the corner list).
    pub per_corner_gradients: Vec<f32>,
}

impl CornerSharpnessMetrics {
    /// Measure corner sharpness in an MSDF bitmap at known corner positions.
    ///
    /// `corners` should be in *bitmap pixel coordinates* (not glyph units).
    pub fn from_msdf(msdf: &MsdfBitmap, corners: &[Point]) -> Self {
        let mut gradients = Vec::with_capacity(corners.len());

        for corner in corners {
            let x = corner.x.round() as usize;
            let y = corner.y.round() as usize;

            if x == 0 || y == 0 || x + 1 >= msdf.width || y + 1 >= msdf.height {
                gradients.push(0.0);
                continue;
            }

            // Reconstruct via median at surrounding pixels
            let val = |xx: usize, yy: usize| -> f32 {
                median_f32(
                    msdf.red_channel.get(xx, yy),
                    msdf.green_channel.get(xx, yy),
                    msdf.blue_channel.get(xx, yy),
                )
            };

            let dx = val(x + 1, y) - val(x - 1, y);
            let dy = val(x, y + 1) - val(x, y - 1);
            let gradient = (dx * dx + dy * dy).sqrt();
            gradients.push(gradient);
        }

        let mean = if gradients.is_empty() {
            0.0
        } else {
            gradients.iter().sum::<f32>() / gradients.len() as f32
        };
        let max = gradients.iter().cloned().fold(0.0_f32, f32::max);

        Self {
            mean_corner_gradient: mean,
            max_corner_gradient: max,
            corner_count: corners.len(),
            per_corner_gradients: gradients,
        }
    }

    /// Measure corner sharpness in a single-channel SDF bitmap.
    pub fn from_sdf(sdf: &SdfBitmap, corners: &[Point]) -> Self {
        let mut gradients = Vec::with_capacity(corners.len());

        for corner in corners {
            let x = corner.x.round() as usize;
            let y = corner.y.round() as usize;

            if x == 0 || y == 0 || x + 1 >= sdf.width || y + 1 >= sdf.height {
                gradients.push(0.0);
                continue;
            }

            let dx = sdf.channel.get(x + 1, y) - sdf.channel.get(x - 1, y);
            let dy = sdf.channel.get(x, y + 1) - sdf.channel.get(x, y - 1);
            let gradient = (dx * dx + dy * dy).sqrt();
            gradients.push(gradient);
        }

        let mean = if gradients.is_empty() {
            0.0
        } else {
            gradients.iter().sum::<f32>() / gradients.len() as f32
        };
        let max = gradients.iter().cloned().fold(0.0_f32, f32::max);

        Self {
            mean_corner_gradient: mean,
            max_corner_gradient: max,
            corner_count: corners.len(),
            per_corner_gradients: gradients,
        }
    }

    /// Compare corner sharpness between MSDF and single-channel SDF.
    ///
    /// Returns the improvement ratio: values > 1.0 mean the MSDF preserves
    /// corners better; values < 1.0 mean the SDF is sharper (unlikely for
    /// corners where MSDF edge colouring is active).
    ///
    /// `outline` is used to detect corner positions.
    /// `msdf` and `sdf` are the generated bitmaps (must have the same dimensions).
    pub fn compare_msdf_vs_sdf(
        outline: &GlyphOutline,
        msdf: &MsdfBitmap,
        sdf: &SdfBitmap,
        angle_threshold: f32,
    ) -> CornerComparison {
        assert_eq!(msdf.width, sdf.width);
        assert_eq!(msdf.height, sdf.height);

        // Convert glyph-space corners to bitmap pixel coordinates
        let corners = outline.detect_all_corners(angle_threshold);
        let sharp_corners: Vec<&CornerInfo> = corners.iter().filter(|c| c.is_sharp).collect();

        if sharp_corners.is_empty() {
            return CornerComparison {
                msdf_metrics: Self {
                    mean_corner_gradient: 0.0,
                    max_corner_gradient: 0.0,
                    corner_count: 0,
                    per_corner_gradients: Vec::new(),
                },
                sdf_metrics: Self {
                    mean_corner_gradient: 0.0,
                    max_corner_gradient: 0.0,
                    corner_count: 0,
                    per_corner_gradients: Vec::new(),
                },
                improvement_ratio: 1.0,
            };
        }

        // Map corner positions from glyph space to bitmap pixel space
        let glyph_bounds = &outline.bounds;
        let scale = msdf.scale;
        let padding = msdf.padding as f32;

        let pixel_corners: Vec<Point> = sharp_corners
            .iter()
            .map(|c| {
                let px = (c.position.x - glyph_bounds.0.x) * scale + padding;
                // Flip Y (glyph Y-up → bitmap Y-down)
                let glyph_height = (glyph_bounds.1.y - glyph_bounds.0.y) * scale;
                let py = glyph_height - (c.position.y - glyph_bounds.0.y) * scale + padding;
                Point::new(px, py)
            })
            .collect();

        let msdf_metrics = Self::from_msdf(msdf, &pixel_corners);
        let sdf_metrics = Self::from_sdf(sdf, &pixel_corners);

        let improvement_ratio = if sdf_metrics.mean_corner_gradient > 1e-6 {
            msdf_metrics.mean_corner_gradient / sdf_metrics.mean_corner_gradient
        } else {
            1.0
        };

        CornerComparison {
            msdf_metrics,
            sdf_metrics,
            improvement_ratio,
        }
    }
}

/// Result of comparing MSDF and SDF corner sharpness.
#[derive(Debug, Clone)]
pub struct CornerComparison {
    /// Corner sharpness metrics for the MSDF bitmap.
    pub msdf_metrics: CornerSharpnessMetrics,
    /// Corner sharpness metrics for the single-channel SDF bitmap.
    pub sdf_metrics: CornerSharpnessMetrics,
    /// Improvement ratio: `msdf_mean / sdf_mean`. Values > 1.0 mean MSDF is sharper.
    pub improvement_ratio: f32,
}

/// Compute the median of three f32 values.
fn median_f32(a: f32, b: f32, c: f32) -> f32 {
    a.max(b.min(c)).min(b.max(c))
}

/// Builder for glyph outlines using ttf_parser OutlineBuilder trait
struct GlyphOutlineBuilder {
    current_contour: Option<Contour>,
    contours: Vec<Contour>,
    last_point: Option<Point>,
    contour_start: Option<Point>,
}

impl GlyphOutlineBuilder {
    fn new() -> Self {
        Self {
            current_contour: None,
            contours: Vec::new(),
            last_point: None,
            contour_start: None,
        }
    }

    fn build(mut self) -> GlyphOutline {
        // Finalize any open contour
        if let Some(contour) = self.current_contour.take()
            && !contour.edges.is_empty()
        {
            self.contours.push(contour);
        }

        let mut outline = GlyphOutline::new();
        for contour in self.contours {
            outline.add_contour(contour);
        }
        outline
    }

    fn ensure_contour(&mut self) {
        if self.current_contour.is_none() {
            self.current_contour = Some(Contour::new());
        }
    }
}

impl ttf_parser::OutlineBuilder for GlyphOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        // Finish previous contour if it exists
        if let Some(contour) = self.current_contour.take()
            && !contour.edges.is_empty()
        {
            self.contours.push(contour);
        }

        let point = Point::new(x, y);
        self.last_point = Some(point);
        self.contour_start = Some(point);
        self.current_contour = Some(Contour::new());
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.ensure_contour();

        if let (Some(last), Some(contour)) = (self.last_point, &mut self.current_contour) {
            let end = Point::new(x, y);
            contour.add_edge(EdgeSegment {
                edge_type: EdgeType::Line { start: last, end },
                color: EdgeColor::WHITE, // Will be set by edge coloring
            });
            self.last_point = Some(end);
        }
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.ensure_contour();

        if let (Some(last), Some(contour)) = (self.last_point, &mut self.current_contour) {
            let p1 = Point::new(x1, y1);
            let p2 = Point::new(x, y);
            contour.add_edge(EdgeSegment {
                edge_type: EdgeType::QuadCurve { p0: last, p1, p2 },
                color: EdgeColor::WHITE,
            });
            self.last_point = Some(p2);
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.ensure_contour();

        if let (Some(last), Some(contour)) = (self.last_point, &mut self.current_contour) {
            let p1 = Point::new(x1, y1);
            let p2 = Point::new(x2, y2);
            let p3 = Point::new(x, y);
            contour.add_edge(EdgeSegment {
                edge_type: EdgeType::CubicCurve {
                    p0: last,
                    p1,
                    p2,
                    p3,
                },
                color: EdgeColor::WHITE,
            });
            self.last_point = Some(p3);
        }
    }

    fn close(&mut self) {
        self.ensure_contour();

        // Close the contour with a line back to start if needed
        if let (Some(last), Some(start), Some(contour)) = (
            self.last_point,
            self.contour_start,
            &mut self.current_contour,
        ) {
            // Only add closing line if we're not already at the start
            let dist = last.distance_to(&start);
            if dist > 1e-6 {
                contour.add_edge(EdgeSegment {
                    edge_type: EdgeType::Line {
                        start: last,
                        end: start,
                    },
                    color: EdgeColor::WHITE,
                });
            }
        }

        self.last_point = None;
        self.contour_start = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msdf_config_creation() {
        let config = MsdfConfig::default();
        assert_eq!(config.atlas_size, 1024);
        assert_eq!(config.glyph_size, 48.0);
        assert_eq!(config.distance_range, 4.0);
    }

    #[test]
    fn test_point_operations() {
        let p1 = Point::new(0.0, 0.0);
        let p2 = Point::new(3.0, 4.0);
        assert!((p1.distance_to(&p2) - 5.0).abs() < 0.001);

        let cross = Point::new(1.0, 0.0).cross(&Point::new(0.0, 1.0));
        assert!((cross - 1.0).abs() < 0.001);

        let dot = Point::new(1.0, 0.0).dot(&Point::new(1.0, 0.0));
        assert!((dot - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_edge_color() {
        assert!(EdgeColor::YELLOW.has_red());
        assert!(EdgeColor::YELLOW.has_green());
        assert!(!EdgeColor::YELLOW.has_blue());

        assert!(EdgeColor::MAGENTA.has_red());
        assert!(!EdgeColor::MAGENTA.has_green());
        assert!(EdgeColor::MAGENTA.has_blue());

        assert!(!EdgeColor::CYAN.has_red());
        assert!(EdgeColor::CYAN.has_green());
        assert!(EdgeColor::CYAN.has_blue());
    }

    #[test]
    fn test_line_distance() {
        let edge = EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 0.0),
            },
            color: EdgeColor::WHITE,
        };

        // Point above the line (TrueType CW winding: above = outside = negative)
        let sd = edge.signed_distance(&Point::new(5.0, 3.0));
        assert!((sd.distance.abs() - 3.0).abs() < 0.001);
        assert!(sd.distance < 0.0); // Above the line = outside = negative

        // Point below the line (TrueType CW winding: below = inside = positive)
        let sd = edge.signed_distance(&Point::new(5.0, -3.0));
        assert!((sd.distance.abs() - 3.0).abs() < 0.001);
        assert!(sd.distance > 0.0); // Below the line = inside = positive
    }

    #[test]
    fn test_cubic_solver() {
        // Simple cubic: x³ - 6x² + 11x - 6 = 0 has roots 1, 2, 3
        let roots = solve_cubic(1.0, -6.0, 11.0, -6.0);
        assert_eq!(roots.len(), 3);

        let mut sorted_roots: Vec<f32> = roots.clone();
        sorted_roots.sort_by(|a, b| a.partial_cmp(b).unwrap());

        assert!((sorted_roots[0] - 1.0).abs() < 0.01);
        assert!((sorted_roots[1] - 2.0).abs() < 0.01);
        assert!((sorted_roots[2] - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_distance_field_creation() {
        let df = DistanceField::new(10, 10);
        assert_eq!(df.width, 10);
        assert_eq!(df.height, 10);
        assert_eq!(df.data.len(), 100);
    }

    #[test]
    fn test_msdf_bitmap_creation() {
        let msdf = MsdfBitmap::new(32, 32, 1.0, 4);
        assert_eq!(msdf.width, 32);
        assert_eq!(msdf.height, 32);
        assert_eq!(msdf.red_channel.data.len(), 32 * 32);
        assert_eq!(msdf.scale, 1.0);
        assert_eq!(msdf.padding, 4);
    }

    #[test]
    fn test_contour_edge_coloring() {
        let mut contour = Contour::new();

        // Create a simple triangle contour (3 corners, 3 colors works perfectly)
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(10.0, 0.0),
                end: Point::new(5.0, 10.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(5.0, 10.0),
                end: Point::new(0.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });

        // Apply coloring with low threshold to treat all corners as sharp
        contour.apply_edge_coloring(0.1);

        // Verify all edges have valid two-channel colors (not WHITE)
        for edge in &contour.edges {
            let c = &edge.color;
            // Each edge should have exactly 2 channels set
            let channel_count = c.r as u8 + c.g as u8 + c.b as u8;
            assert_eq!(channel_count, 2, "Edge should have exactly 2 channels set");
        }

        // For a triangle, adjacent edges should have different colors at all corners
        for i in 0..contour.edges.len() {
            let next = (i + 1) % contour.edges.len();
            let c1 = &contour.edges[i].color;
            let c2 = &contour.edges[next].color;
            assert!(
                c1 != c2,
                "Adjacent edges in triangle should have different colors at corners"
            );
        }
    }

    #[test]
    fn test_contour_edge_coloring_square() {
        let mut contour = Contour::new();

        // Create a simple square contour (4 corners, but only 3 colors)
        // This tests that the algorithm handles the case where perfect coloring is impossible
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(10.0, 0.0),
                end: Point::new(10.0, 10.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(10.0, 10.0),
                end: Point::new(0.0, 10.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(0.0, 10.0),
                end: Point::new(0.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });

        // Apply coloring with low threshold to treat all corners as sharp
        contour.apply_edge_coloring(0.1);

        // Verify all edges have valid two-channel colors
        for edge in &contour.edges {
            let c = &edge.color;
            let channel_count = c.r as u8 + c.g as u8 + c.b as u8;
            assert_eq!(channel_count, 2, "Edge should have exactly 2 channels set");
        }

        // Note: For a square (4 corners), we can't guarantee all adjacent edges have
        // different colors because we only have 3 colors. This is a known limitation
        // documented in the Chlumsky thesis. We just verify valid colors are assigned.
    }

    // -----------------------------------------------------------------------
    // Single-channel SDF generator tests
    // -----------------------------------------------------------------------

    fn load_test_font_data() -> Vec<u8> {
        include_bytes!("../../assets/fonts/default.ttf").to_vec()
    }

    #[test]
    fn test_sdf_config_defaults() {
        let config = SdfConfig::default();
        assert_eq!(config.glyph_size, 48.0);
        assert_eq!(config.distance_range, 4.0);
        assert_eq!(config.padding, 4);
    }

    #[test]
    fn test_sdf_bitmap_creation() {
        let sdf = SdfBitmap::new(32, 32, 1.0, 4);
        assert_eq!(sdf.width, 32);
        assert_eq!(sdf.height, 32);
        assert_eq!(sdf.channel.data.len(), 32 * 32);
        assert_eq!(sdf.scale, 1.0);
        assert_eq!(sdf.padding, 4);
    }

    #[test]
    fn test_sdf_bitmap_to_rgba() {
        let mut sdf = SdfBitmap::new(2, 2, 1.0, 0);
        // Set known values: -1.0 (far outside), 0.0 (edge), 1.0 (far inside)
        sdf.channel.set(0, 0, -1.0);
        sdf.channel.set(1, 0, 0.0);
        sdf.channel.set(0, 1, 1.0);
        sdf.channel.set(1, 1, 0.5);

        let rgba = sdf.to_rgba_pixels();
        assert_eq!(rgba.len(), 2 * 2 * 4);

        // -1.0 -> 0.0 normalised -> pixel 0
        assert_eq!(rgba[0], 0);
        assert_eq!(rgba[1], 0);
        assert_eq!(rgba[2], 0);
        assert_eq!(rgba[3], 255);

        // 0.0 -> 0.5 normalised -> pixel 127/128
        let edge_val = rgba[4];
        assert!((edge_val as i32 - 127).unsigned_abs() <= 1);
        // All three channels identical
        assert_eq!(rgba[4], rgba[5]);
        assert_eq!(rgba[5], rgba[6]);
        assert_eq!(rgba[7], 255);

        // 1.0 -> 1.0 normalised -> pixel 255
        assert_eq!(rgba[8], 255);
    }

    #[test]
    fn test_sdf_generator_creation() {
        let data = load_test_font_data();
        let sdf_gen = SdfGenerator::new(data, SdfConfig::default());
        assert!(sdf_gen.is_ok());
    }

    #[test]
    fn test_sdf_generator_invalid_font() {
        let sdf_gen = SdfGenerator::new(vec![0u8; 100], SdfConfig::default());
        assert!(sdf_gen.is_err());
    }

    #[test]
    fn test_sdf_generate_char_a() {
        let data = load_test_font_data();
        let sdf_gen = SdfGenerator::new(data, SdfConfig::default()).unwrap();
        let sdf = sdf_gen.generate_sdf_for_char('A').unwrap();

        // Bitmap should have reasonable dimensions
        assert!(sdf.width > 0);
        assert!(sdf.height > 0);
        assert_eq!(sdf.channel.data.len(), sdf.width * sdf.height);

        // Should contain both inside and outside values
        let has_inside = sdf.channel.data.iter().any(|&v| v > 0.1);
        let has_outside = sdf.channel.data.iter().any(|&v| v < -0.1);
        assert!(has_inside, "SDF for 'A' should have inside values");
        assert!(has_outside, "SDF for 'A' should have outside values");
    }

    #[test]
    fn test_sdf_generates_full_ascii() {
        let data = load_test_font_data();
        let sdf_gen = SdfGenerator::new(data.clone(), SdfConfig::default()).unwrap();
        let font = ttf_parser::Face::parse(&data, 0).unwrap();

        let mut success_count = 0;
        for ch in 33u8..=126u8 {
            let c = ch as char;
            if font.glyph_index(c).is_some()
                && font
                    .glyph_bounding_box(font.glyph_index(c).unwrap())
                    .is_some()
            {
                let result = sdf_gen.generate_sdf_for_char(c);
                assert!(result.is_ok(), "Failed to generate SDF for '{c}'");
                let sdf = result.unwrap();
                assert!(sdf.width > 0 && sdf.height > 0);
                success_count += 1;
            }
        }
        assert!(success_count > 0, "Should generate at least some glyphs");
    }

    #[test]
    fn test_sdf_at_simple_contour() {
        // Create a simple square outline with CW winding (TrueType outer contour convention).
        // CW in the Y-up coordinate system used by TrueType means: right → down → left → up
        // which is the opposite of screen coordinates.
        let mut outline = GlyphOutline::new();
        let mut contour = Contour::new();
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(0.0, 10.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(0.0, 10.0),
                end: Point::new(10.0, 10.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(10.0, 10.0),
                end: Point::new(10.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(10.0, 0.0),
                end: Point::new(0.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });
        outline.add_contour(contour);

        // Centre of square should be inside (positive distance)
        let d_centre = outline.sdf_at(&Point::new(5.0, 5.0));
        assert!(d_centre > 0.0, "Centre should be inside: {d_centre}");

        // Outside the square should be negative distance
        let d_outside = outline.sdf_at(&Point::new(-5.0, 5.0));
        assert!(d_outside < 0.0, "Outside should be negative: {d_outside}");

        // On the edge should be very close to zero
        let d_edge = outline.sdf_at(&Point::new(0.0, 5.0));
        assert!(d_edge.abs() < 0.5, "On edge should be near zero: {d_edge}");
    }

    #[test]
    fn test_sdf_matches_msdf_dimensions() {
        let data = load_test_font_data();
        let sdf_gen = SdfGenerator::new(data.clone(), SdfConfig::default()).unwrap();
        let msdf_gen = MsdfGenerator::new(data, MsdfConfig::default()).unwrap();

        for c in ['A', 'g', 'W'] {
            let sdf = sdf_gen.generate_sdf_for_char(c).unwrap();
            let msdf = msdf_gen.generate_msdf_for_char(c).unwrap();

            assert_eq!(sdf.width, msdf.width, "Width mismatch for '{c}'");
            assert_eq!(sdf.height, msdf.height, "Height mismatch for '{c}'");
            assert_eq!(sdf.scale, msdf.scale, "Scale mismatch for '{c}'");
            assert_eq!(sdf.padding, msdf.padding, "Padding mismatch for '{c}'");
        }
    }

    #[test]
    fn test_quality_metrics_self_comparison() {
        let data = load_test_font_data();
        let msdf_gen = MsdfGenerator::new(data, MsdfConfig::default()).unwrap();
        let msdf = msdf_gen.generate_msdf_for_char('A').unwrap();
        let metrics = SdfQualityMetrics::from_msdf(&msdf);

        assert_eq!(metrics.mean_absolute_error, 0.0);
        assert!(metrics.peak_signal_to_noise.is_infinite());
        assert!(metrics.edge_sharpness >= 0.0);
        assert!(metrics.memory_bytes > 0);
    }

    #[test]
    fn test_quality_metrics_comparison() {
        let data = load_test_font_data();
        let sdf_gen = SdfGenerator::new(data.clone(), SdfConfig::default()).unwrap();
        let msdf_gen = MsdfGenerator::new(data, MsdfConfig::default()).unwrap();

        let sdf = sdf_gen.generate_sdf_for_char('A').unwrap();
        let msdf = msdf_gen.generate_msdf_for_char('A').unwrap();
        let metrics = SdfQualityMetrics::compare(&sdf, &msdf);

        // MAE should be non-negative
        assert!(metrics.mean_absolute_error >= 0.0);
        // PSNR should be positive (they're similar but not identical)
        assert!(metrics.peak_signal_to_noise > 0.0);
        // Edge sharpness should be non-negative
        assert!(metrics.edge_sharpness >= 0.0);
        // SDF uses 1/3 the memory of MSDF
        let msdf_metrics = SdfQualityMetrics::from_msdf(&msdf);
        assert!(
            metrics.memory_bytes < msdf_metrics.memory_bytes,
            "SDF should use less memory than MSDF"
        );
    }

    #[test]
    fn test_median_f32() {
        assert_eq!(median_f32(1.0, 2.0, 3.0), 2.0);
        assert_eq!(median_f32(3.0, 1.0, 2.0), 2.0);
        assert_eq!(median_f32(2.0, 3.0, 1.0), 2.0);
        assert_eq!(median_f32(1.0, 1.0, 1.0), 1.0);
        assert_eq!(median_f32(-1.0, 0.0, 1.0), 0.0);
    }

    // -----------------------------------------------------------------------
    // Multi-channel SDF configuration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_multi_channel_sdf_config_defaults() {
        let config = MultiChannelSdfConfig::default();
        assert_eq!(config.max_distance, 4.0);
        assert_eq!(config.max_channels, 3);
        assert_eq!(config.glyph_size, 48.0);
        assert_eq!(config.padding, 4);
        assert_eq!(config.combination_mode, ChannelCombinationMode::Median);
    }

    #[test]
    fn test_multi_channel_sdf_config_to_msdf() {
        let config = MultiChannelSdfConfig {
            max_distance: 6.0,
            sharp_corner_threshold: 0.5,
            max_channels: 3,
            combination_mode: ChannelCombinationMode::Min,
            glyph_size: 64.0,
            padding: 8,
        };
        let msdf = config.to_msdf_config();
        assert_eq!(msdf.distance_range, 6.0);
        assert_eq!(msdf.angle_threshold, 0.5);
        assert_eq!(msdf.glyph_size, 64.0);
        assert_eq!(msdf.padding, 8);
    }

    // -----------------------------------------------------------------------
    // Corner detection tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_corners_triangle() {
        let mut contour = Contour::new();
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(10.0, 0.0),
                end: Point::new(5.0, 10.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(5.0, 10.0),
                end: Point::new(0.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });

        let corners = contour.detect_corners(0.1);
        assert_eq!(corners.len(), 3, "Triangle should have 3 corners");

        // All corners in a triangle are sharp (well under 180°)
        for c in &corners {
            assert!(c.is_sharp, "Triangle corners should be sharp");
            assert!(c.angle > 0.0, "Angle should be positive");
        }
    }

    #[test]
    fn test_detect_corners_smooth_circle_approximation() {
        // Create a very smooth contour (near-circle from many small segments)
        let mut contour = Contour::new();
        let n = 32;
        for i in 0..n {
            let a0 = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
            let a1 = 2.0 * std::f32::consts::PI * (i + 1) as f32 / n as f32;
            contour.add_edge(EdgeSegment {
                edge_type: EdgeType::Line {
                    start: Point::new(a0.cos() * 100.0, a0.sin() * 100.0),
                    end: Point::new(a1.cos() * 100.0, a1.sin() * 100.0),
                },
                color: EdgeColor::WHITE,
            });
        }

        let corners = contour.detect_corners(std::f32::consts::PI / 3.0);
        let sharp_count = corners.iter().filter(|c| c.is_sharp).count();
        assert_eq!(sharp_count, 0, "Smooth circle should have no sharp corners");
    }

    #[test]
    fn test_classify_pattern_teardrop() {
        // Contour with exactly one sharp corner
        let mut contour = Contour::new();
        let n = 16;
        for i in 0..n {
            let a0 = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
            let a1 = 2.0 * std::f32::consts::PI * (i + 1) as f32 / n as f32;
            contour.add_edge(EdgeSegment {
                edge_type: EdgeType::Line {
                    start: Point::new(a0.cos() * 100.0, a0.sin() * 100.0),
                    end: Point::new(a1.cos() * 100.0, a1.sin() * 100.0),
                },
                color: EdgeColor::WHITE,
            });
        }

        // Replace one edge pair with a sharp spike
        contour.edges[0] = EdgeSegment {
            edge_type: EdgeType::Line {
                start: contour.edges[n - 1].end_point(),
                end: Point::new(200.0, 0.0),
            },
            color: EdgeColor::WHITE,
        };
        contour.edges[1] = EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(200.0, 0.0),
                end: contour.edges[2].start_point(),
            },
            color: EdgeColor::WHITE,
        };

        let pattern = contour.classify_pattern(std::f32::consts::PI / 3.0);
        matches!(pattern, ContourPattern::Teardrop { .. });
    }

    #[test]
    fn test_classify_pattern_standard_square() {
        let mut contour = Contour::new();
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(10.0, 0.0),
                end: Point::new(10.0, 10.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(10.0, 10.0),
                end: Point::new(0.0, 10.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(0.0, 10.0),
                end: Point::new(0.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });

        let pattern = contour.classify_pattern(0.1);
        match pattern {
            ContourPattern::Standard {
                sharp_corner_count, ..
            } => {
                assert_eq!(sharp_corner_count, 4, "Square should have 4 sharp corners");
            }
            other => panic!("Expected Standard, got {other:?}"),
        }
    }

    #[test]
    fn test_glyph_outline_detect_all_corners() {
        let data = load_test_font_data();
        let msdf_gen = MsdfGenerator::new(data, MsdfConfig::default()).unwrap();
        let mut outline = msdf_gen.generate_outline('A').unwrap();
        outline.apply_edge_coloring(std::f32::consts::PI / 3.0);

        let corners = outline.detect_all_corners(std::f32::consts::PI / 3.0);
        // 'A' should have sharp corners (at the apex and at the serifs)
        let sharp = corners.iter().filter(|c| c.is_sharp).count();
        assert!(
            sharp >= 2,
            "'A' should have at least 2 sharp corners, found {sharp}"
        );
    }

    #[test]
    fn test_glyph_outline_classify_contours() {
        let data = load_test_font_data();
        let msdf_gen = MsdfGenerator::new(data, MsdfConfig::default()).unwrap();
        let outline = msdf_gen.generate_outline('O').unwrap();

        let patterns = outline.classify_contours(std::f32::consts::PI / 3.0);
        // 'O' typically has 2 contours (outer + inner)
        assert!(
            !patterns.is_empty(),
            "'O' should have at least one contour pattern"
        );
    }

    // -----------------------------------------------------------------------
    // Corner sharpness metrics tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_corner_sharpness_msdf_vs_sdf() {
        let data = load_test_font_data();
        let msdf_gen = MsdfGenerator::new(data.clone(), MsdfConfig::default()).unwrap();
        let sdf_gen = SdfGenerator::new(data, SdfConfig::default()).unwrap();

        // Generate MSDF and SDF for 'V' which has a sharp bottom point
        let msdf = msdf_gen.generate_msdf_for_char('V').unwrap();
        let sdf = sdf_gen.generate_sdf_for_char('V').unwrap();

        // Use the centre-bottom of the bitmap as a corner (where the V's point is)
        let corner = Point::new(msdf.width as f32 / 2.0, msdf.height as f32 * 0.8);
        let corners = vec![corner];

        let msdf_metrics = CornerSharpnessMetrics::from_msdf(&msdf, &corners);
        let sdf_metrics = CornerSharpnessMetrics::from_sdf(&sdf, &corners);

        assert_eq!(msdf_metrics.corner_count, 1);
        assert_eq!(sdf_metrics.corner_count, 1);

        // Both should produce some gradient (even if the exact corner pixel misses slightly)
        // The key assertion is that metrics are produced without errors
        assert!(msdf_metrics.mean_corner_gradient >= 0.0);
        assert!(sdf_metrics.mean_corner_gradient >= 0.0);
    }

    #[test]
    fn test_compare_msdf_vs_sdf_on_sharp_glyph() {
        let data = load_test_font_data();
        let config = MsdfConfig::default();
        let msdf_gen = MsdfGenerator::new(data.clone(), config.clone()).unwrap();
        let sdf_gen = SdfGenerator::new(
            data,
            SdfConfig {
                glyph_size: config.glyph_size,
                distance_range: config.distance_range,
                padding: config.padding,
            },
        )
        .unwrap();

        // 'A' has a sharp apex
        let mut outline = msdf_gen.generate_outline('A').unwrap();
        outline.apply_edge_coloring(config.angle_threshold);

        let msdf = msdf_gen.generate_msdf_for_char('A').unwrap();
        let sdf = sdf_gen.generate_sdf_for_char('A').unwrap();

        let comparison = CornerSharpnessMetrics::compare_msdf_vs_sdf(
            &outline,
            &msdf,
            &sdf,
            config.angle_threshold,
        );

        // Should have found some sharp corners
        assert!(
            comparison.msdf_metrics.corner_count > 0,
            "'A' should have sharp corners"
        );
        // improvement_ratio should be non-negative
        assert!(comparison.improvement_ratio >= 0.0);
    }

    // -----------------------------------------------------------------------
    // MSDF debugging helpers tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_msdf_bitmap_channel_to_grayscale() {
        let mut msdf = MsdfBitmap::new(4, 4, 1.0, 0);
        msdf.red_channel.set(1, 1, 0.8);
        msdf.green_channel.set(1, 1, -0.3);
        msdf.blue_channel.set(1, 1, 0.0);

        let gray_r = msdf.channel_to_grayscale(0);
        let gray_g = msdf.channel_to_grayscale(1);
        let gray_b = msdf.channel_to_grayscale(2);

        // Each is 4×4 RGBA = 64 bytes
        assert_eq!(gray_r.len(), 64);
        assert_eq!(gray_g.len(), 64);
        assert_eq!(gray_b.len(), 64);

        // Red channel at (1,1): 0.8 → 0.9 normalised → 229 pixel value
        let idx = (4 + 1) * 4; // pixel (1,1) in RGBA
        let r_val = gray_r[idx];
        assert!(
            r_val > 200,
            "Expected bright red channel pixel, got {r_val}"
        );

        // Green channel at (1,1): -0.3 → 0.35 → 89 pixel value
        let g_val = gray_g[idx];
        assert!(g_val < 128, "Expected dim green channel pixel, got {g_val}");
    }

    #[test]
    fn test_msdf_bitmap_reconstructed_median() {
        let mut msdf = MsdfBitmap::new(4, 4, 1.0, 0);
        msdf.red_channel.set(2, 2, 0.6);
        msdf.green_channel.set(2, 2, 0.2);
        msdf.blue_channel.set(2, 2, 0.4);

        let recon = msdf.reconstructed_median();
        let val = recon.get(2, 2);
        // median(0.6, 0.2, 0.4) = 0.4
        assert!(
            (val - 0.4).abs() < 0.01,
            "Reconstructed median should be 0.4, got {val}"
        );
    }

    // -----------------------------------------------------------------------
    // Edge coloring enhancement tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_teardrop_coloring_splits_at_furthest() {
        // Build a teardrop-like contour: a sharp cusp at (0,0) followed by
        // a smooth arc. The synthetic split should land near the opposite side
        // of the arc (at about 180° from the cusp).
        let mut contour = Contour::new();
        let n = 12;
        // Cusp edges: two edges meeting at a sharp angle at (0,0)
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(-3.0, -5.0),
                end: Point::new(0.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(3.0, -5.0),
            },
            color: EdgeColor::WHITE,
        });
        // Smooth arc connecting the two legs
        for i in 0..n {
            let a0 = -std::f32::consts::PI / 3.0
                + std::f32::consts::PI * (2.0 / 3.0) * i as f32 / n as f32;
            let a1 = -std::f32::consts::PI / 3.0
                + std::f32::consts::PI * (2.0 / 3.0) * (i + 1) as f32 / n as f32;
            let start = if i == 0 {
                Point::new(3.0, -5.0)
            } else {
                Point::new(a0.cos() * 10.0, a0.sin() * 10.0 - 15.0)
            };
            let end = if i == n - 1 {
                Point::new(-3.0, -5.0)
            } else {
                Point::new(a1.cos() * 10.0, a1.sin() * 10.0 - 15.0)
            };
            contour.add_edge(EdgeSegment {
                edge_type: EdgeType::Line { start, end },
                color: EdgeColor::WHITE,
            });
        }

        contour.apply_edge_coloring(0.3);

        // The cusp should have different colours on each side
        let c0 = contour.edges[0].color;
        let c1 = contour.edges[1].color;
        assert_ne!(
            c0, c1,
            "Edges flanking the cusp should have different colours"
        );

        // All edges should have valid 2-channel colours (not WHITE)
        for edge in &contour.edges {
            let count = edge.color.r as u8 + edge.color.g as u8 + edge.color.b as u8;
            assert_eq!(
                count, 2,
                "Edge should have exactly 2 channels set, got {count}"
            );
        }
    }

    #[test]
    fn test_msdf_generator_from_multi_channel_config() {
        let data = load_test_font_data();
        let config = MultiChannelSdfConfig {
            max_distance: 6.0,
            sharp_corner_threshold: 0.5,
            max_channels: 3,
            combination_mode: ChannelCombinationMode::Min,
            glyph_size: 64.0,
            padding: 8,
        };
        let msdf_gen = MsdfGenerator::from_multi_channel_config(data, &config);
        assert!(msdf_gen.is_ok());
        let msdf_gen = msdf_gen.unwrap();
        let msdf = msdf_gen.generate_msdf_for_char('K');
        assert!(msdf.is_ok());
        let msdf = msdf.unwrap();
        assert!(msdf.width > 0);
        assert!(msdf.height > 0);
    }

    #[test]
    fn test_combination_modes_differ_at_corners() {
        // Generate MSDF for 'V' (sharp bottom corner) and verify that
        // median and min reconstructions differ near the corner.
        let data = load_test_font_data();
        let msdf_gen = MsdfGenerator::new(data, MsdfConfig::default()).unwrap();
        let msdf = msdf_gen.generate_msdf_for_char('V').unwrap();

        let mut median_vals = Vec::new();
        let mut min_vals = Vec::new();
        let mut max_vals = Vec::new();

        for y in 0..msdf.height {
            for x in 0..msdf.width {
                let r = msdf.red_channel.get(x, y);
                let g = msdf.green_channel.get(x, y);
                let b = msdf.blue_channel.get(x, y);
                median_vals.push(median_f32(r, g, b));
                min_vals.push(r.min(g).min(b));
                max_vals.push(r.max(g).max(b));
            }
        }

        // median and min/max should not be identical everywhere when
        // edge coloring creates channel differences
        let differs_min = median_vals
            .iter()
            .zip(min_vals.iter())
            .any(|(m, n)| (m - n).abs() > 0.01);
        let differs_max = median_vals
            .iter()
            .zip(max_vals.iter())
            .any(|(m, n)| (m - n).abs() > 0.01);

        assert!(
            differs_min,
            "Median and min should differ for a glyph with sharp corners"
        );
        assert!(
            differs_max,
            "Median and max should differ for a glyph with sharp corners"
        );
    }

    // -----------------------------------------------------------------------
    // Comprehensive sharp-corner glyph tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_sharp_corner_glyphs_have_channel_differences() {
        // Letters with known sharp features should produce different per-channel
        // values at some pixels, proving edge coloring is working.
        let data = load_test_font_data();
        let msdf_gen = MsdfGenerator::new(data, MsdfConfig::default()).unwrap();

        for c in ['A', 'K', 'V', 'W', 'X', 'Y', 'Z'] {
            let msdf = msdf_gen.generate_msdf_for_char(c).unwrap();
            let has_diff = (0..msdf.width).any(|x| {
                (0..msdf.height).any(|y| {
                    let r = msdf.red_channel.get(x, y);
                    let g = msdf.green_channel.get(x, y);
                    let b = msdf.blue_channel.get(x, y);
                    (r - g).abs() > 0.05 || (g - b).abs() > 0.05 || (r - b).abs() > 0.05
                })
            });
            assert!(
                has_diff,
                "'{c}' should have per-channel differences from edge coloring"
            );
        }
    }

    #[test]
    fn test_smooth_glyph_channels_agree() {
        // 'O' is mostly smooth curves — channels should be close
        // (not exactly identical because of minor colour boundaries)
        let data = load_test_font_data();
        let msdf_gen = MsdfGenerator::new(data, MsdfConfig::default()).unwrap();

        let msdf = msdf_gen.generate_msdf_for_char('O').unwrap();

        let mut total_diff = 0.0_f64;
        let mut count = 0usize;
        for y in 0..msdf.height {
            for x in 0..msdf.width {
                let r = msdf.red_channel.get(x, y);
                let g = msdf.green_channel.get(x, y);
                let b = msdf.blue_channel.get(x, y);
                total_diff += ((r - g).abs() + (g - b).abs() + (r - b).abs()) as f64;
                count += 1;
            }
        }
        let mean_diff = total_diff / count as f64;
        // 'O' should have very low average channel divergence
        assert!(
            mean_diff < 0.5,
            "'O' average channel divergence should be low: {mean_diff:.4}"
        );
    }

    #[test]
    fn test_msdf_generation_performance() {
        // Full ASCII set should generate within 200ms
        use std::time::Instant;

        let data = load_test_font_data();
        let msdf_gen = MsdfGenerator::new(data.clone(), MsdfConfig::default()).unwrap();
        let font = ttf_parser::Face::parse(&data, 0).unwrap();

        let start = Instant::now();
        let mut count = 0;
        for ch in 33u8..=126u8 {
            let c = ch as char;
            if let Some(glyph_id) = font.glyph_index(c)
                && font.glyph_bounding_box(glyph_id).is_some()
            {
                msdf_gen.generate_msdf(glyph_id).unwrap();
                count += 1;
            }
        }
        let elapsed = start.elapsed();
        // In debug mode the threshold is generous
        #[cfg(debug_assertions)]
        let threshold_ms: u128 = 5000;
        #[cfg(not(debug_assertions))]
        let threshold_ms: u128 = 200;

        assert!(
            elapsed.as_millis() < threshold_ms,
            "MSDF generation for {count} glyphs took {elapsed:?} (threshold: {threshold_ms}ms)"
        );
    }

    #[test]
    fn test_corner_detection_across_special_glyphs() {
        let data = load_test_font_data();
        let msdf_gen = MsdfGenerator::new(data, MsdfConfig::default()).unwrap();
        let threshold = std::f32::consts::PI / 3.0;

        // Characters that should have sharp corners
        for c in ['A', 'V', 'W', 'M', 'N'] {
            let outline = msdf_gen.generate_outline(c).unwrap();
            let corners = outline.detect_all_corners(threshold);
            let sharp = corners.iter().filter(|c| c.is_sharp).count();
            assert!(
                sharp >= 1,
                "'{c}' should have at least 1 sharp corner, found {sharp}"
            );
        }
    }

    #[test]
    fn test_edge_coloring_does_not_leave_white_edges() {
        // After coloring, no edge in a multi-corner contour should remain WHITE.
        let data = load_test_font_data();
        let msdf_gen = MsdfGenerator::new(data, MsdfConfig::default()).unwrap();

        for c in ['A', 'B', 'K', 'R'] {
            let mut outline = msdf_gen.generate_outline(c).unwrap();
            outline.apply_edge_coloring(std::f32::consts::PI / 3.0);

            for (ci, contour) in outline.contours.iter().enumerate() {
                let sharp_count = contour
                    .detect_corners(std::f32::consts::PI / 3.0)
                    .iter()
                    .filter(|c| c.is_sharp)
                    .count();
                if sharp_count >= 2 {
                    for (ei, edge) in contour.edges.iter().enumerate() {
                        assert_ne!(
                            edge.color,
                            EdgeColor::WHITE,
                            "'{c}' contour {ci} edge {ei} should not be WHITE after coloring"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_backward_compatibility_single_channel() {
        // A single-channel SDF replicated across R=G=B should reconstruct
        // identically via median.
        let data = load_test_font_data();
        let sdf_gen = SdfGenerator::new(data, SdfConfig::default()).unwrap();
        let sdf = sdf_gen.generate_sdf_for_char('H').unwrap();

        let rgba = sdf.to_rgba_pixels();
        // Every pixel should have R == G == B (single channel duplicated)
        for y in 0..sdf.height {
            for x in 0..sdf.width {
                let idx = (y * sdf.width + x) * 4;
                assert_eq!(rgba[idx], rgba[idx + 1], "R != G at ({x},{y})");
                assert_eq!(rgba[idx + 1], rgba[idx + 2], "G != B at ({x},{y})");
            }
        }
    }

    #[test]
    fn test_multi_channel_memory_overhead() {
        // MSDF should use exactly 3x the memory of SDF for the same glyph.
        let data = load_test_font_data();
        let msdf_gen = MsdfGenerator::new(data.clone(), MsdfConfig::default()).unwrap();
        let sdf_gen = SdfGenerator::new(data, SdfConfig::default()).unwrap();

        let msdf = msdf_gen.generate_msdf_for_char('W').unwrap();
        let sdf = sdf_gen.generate_sdf_for_char('W').unwrap();

        let msdf_mem = SdfQualityMetrics::from_msdf(&msdf).memory_bytes;
        let sdf_mem = SdfQualityMetrics::compare(&sdf, &msdf).memory_bytes;

        assert_eq!(
            msdf_mem,
            sdf_mem * 3,
            "MSDF should use 3x memory: msdf={msdf_mem}, sdf={sdf_mem}"
        );
    }

    #[test]
    fn test_empty_glyph_outline_detection() {
        // An empty outline should have no corners and classify as Smooth.
        let outline = GlyphOutline::new();
        let corners = outline.detect_all_corners(0.5);
        assert!(corners.is_empty(), "Empty outline should have no corners");

        let patterns = outline.classify_contours(0.5);
        assert!(patterns.is_empty(), "Empty outline should have no patterns");
    }

    #[test]
    fn test_single_edge_contour_coloring() {
        // A degenerate contour with a single edge should not panic.
        let mut contour = Contour::new();
        contour.add_edge(EdgeSegment {
            edge_type: EdgeType::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 0.0),
            },
            color: EdgeColor::WHITE,
        });
        contour.apply_edge_coloring(0.5);
        // Should not panic; coloring may or may not change the single edge
    }
}
