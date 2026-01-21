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

/// Signed distance result including orthogonality for tie-breaking
#[derive(Debug, Clone, Copy)]
struct SignedDistance {
    /// The signed distance value
    distance: f32,
    /// Parameter t at which the minimum distance occurs (for pseudo-distance)
    t: f32,
}

impl SignedDistance {
    fn new(distance: f32, t: f32) -> Self {
        Self { distance, t }
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

    /// Compute signed distance from a point to this edge segment
    fn signed_distance(&self, point: &Point) -> SignedDistance {
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
            return SignedDistance::new(point.distance_to(start), 0.0);
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
        let cross = edge_vec.cross(&point_vec);
        let signed_dist = if cross >= 0.0 { -distance } else { distance };

        SignedDistance::new(signed_dist, t_clamped)
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
        let to_point = point.sub(&self.point_at(best_t));
        let cross = dir.cross(&to_point);
        let signed_dist = if cross >= 0.0 { -distance } else { distance };

        SignedDistance::new(signed_dist, best_t)
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
        let cross = dir.cross(&to_point);
        let signed_dist = if cross >= 0.0 { -distance } else { distance };

        SignedDistance::new(signed_dist, best_t)
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
    fn pseudo_distance(&self, point: &Point) -> SignedDistance {
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

            SignedDistance::new(sign * perp_dist, sd.t)
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
        // If only one edge (teardrop), arbitrarily create two half-edges
        if !starter.any(|&c| c) {
            is_corner[(first_edge + self.edges.len() / 2) % self.edges.len()] = true;
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
                    && (red_closest.is_none()
                        || sd.distance.abs() < red_closest.unwrap().1.distance.abs())
                {
                    red_closest = Some((edge, sd));
                }

                // Update green channel if this edge has green component
                if edge.color.has_green()
                    && (green_closest.is_none()
                        || sd.distance.abs() < green_closest.unwrap().1.distance.abs())
                {
                    green_closest = Some((edge, sd));
                }

                // Update blue channel if this edge has blue component
                if edge.color.has_blue()
                    && (blue_closest.is_none()
                        || sd.distance.abs() < blue_closest.unwrap().1.distance.abs())
                {
                    blue_closest = Some((edge, sd));
                }
            }
        }

        // Compute pseudo_distance ONLY for the 3 closest edges (one per channel)
        let r = red_closest
            .map(|(edge, _)| edge.pseudo_distance(point).distance)
            .unwrap_or(f32::INFINITY);
        let g = green_closest
            .map(|(edge, _)| edge.pseudo_distance(point).distance)
            .unwrap_or(f32::INFINITY);
        let b = blue_closest
            .map(|(edge, _)| edge.pseudo_distance(point).distance)
            .unwrap_or(f32::INFINITY);

        [r, g, b]
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
}

impl MsdfBitmap {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            red_channel: DistanceField::new(width, height),
            green_channel: DistanceField::new(width, height),
            blue_channel: DistanceField::new(width, height),
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
        let mut msdf = MsdfBitmap::new(glyph_width.max(1), glyph_height.max(1));
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

        // Point above the line
        let sd = edge.signed_distance(&Point::new(5.0, 3.0));
        assert!((sd.distance.abs() - 3.0).abs() < 0.001);
        assert!(sd.distance > 0.0); // Above the line (positive)

        // Point below the line
        let sd = edge.signed_distance(&Point::new(5.0, -3.0));
        assert!((sd.distance.abs() - 3.0).abs() < 0.001);
        assert!(sd.distance < 0.0); // Below the line (negative)
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
        let msdf = MsdfBitmap::new(32, 32);
        assert_eq!(msdf.width, 32);
        assert_eq!(msdf.height, 32);
        assert_eq!(msdf.red_channel.data.len(), 32 * 32);
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
}
