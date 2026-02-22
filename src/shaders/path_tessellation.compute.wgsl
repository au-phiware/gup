// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GPU Path Tessellation Compute Shader
//
// Tessellates path commands into triangle vertices on the GPU.
// Supports MoveTo, LineTo, QuadraticCurveTo, CubicCurveTo commands.

// Path command types (matches PathCommandType in Rust)
const CMD_MOVE_TO: u32 = 0u;
const CMD_LINE_TO: u32 = 1u;
const CMD_QUADRATIC_TO: u32 = 2u;
const CMD_CUBIC_TO: u32 = 3u;
const CMD_CLOSE: u32 = 4u;

// Path command data structure
struct PathCommand {
    cmd_type: u32,
    padding1: u32,
    // For all commands: p0 = endpoint
    // For quadratic: p1 = control point
    // For cubic: p1 = control1, p2 = control2
    p0: vec2<f32>,
    p1: vec2<f32>,
    p2: vec2<f32>,
}

// Output vertex structure
struct PathVertex {
    position: vec2<f32>,
    tex_coords: vec2<f32>,
}

// Uniforms for tessellation control
struct TessellationUniforms {
    command_count: u32,
    tolerance: f32,          // Curve flattening tolerance
    max_vertices: u32,
    vertex_count: atomic<u32>,  // Output vertex counter
    index_count: atomic<u32>,   // Output index counter
    padding: array<u32, 3>,
}

// Input buffers
@group(0) @binding(0)
var<storage, read> commands: array<PathCommand>;

@group(0) @binding(1)
var<storage, read_write> vertices: array<PathVertex>;

@group(0) @binding(2)
var<storage, read_write> indices: array<u32>;

// Uniforms
@group(0) @binding(3)
var<storage, read_write> uniforms: TessellationUniforms;

// Evaluate a point on a quadratic Bezier curve at parameter t [0, 1]
fn eval_quadratic(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, t: f32) -> vec2<f32> {
    let t1 = 1.0 - t;
    return t1 * t1 * p0 + 2.0 * t1 * t * p1 + t * t * p2;
}

// Evaluate a point on a cubic Bezier curve at parameter t [0, 1]
fn eval_cubic(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>, t: f32) -> vec2<f32> {
    let t1 = 1.0 - t;
    let t1_sq = t1 * t1;
    let t_sq = t * t;
    return t1_sq * t1 * p0 + 3.0 * t1_sq * t * p1 + 3.0 * t1 * t_sq * p2 + t_sq * t * p3;
}

// Calculate number of segments needed for quadratic curve
fn calc_quadratic_segments(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, tolerance: f32) -> u32 {
    let chord = distance(p0, p2);
    let control_dist = distance(p0, p1) + distance(p1, p2);
    let curvature = control_dist / max(chord, 0.001);
    let segments = u32(max(2.0, ceil(curvature * 10.0 / tolerance)));
    return min(segments, 32u);
}

// Calculate number of segments needed for cubic curve
fn calc_cubic_segments(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>, tolerance: f32) -> u32 {
    let chord = distance(p0, p3);
    let control_dist = distance(p0, p1) + distance(p1, p2) + distance(p2, p3);
    let curvature = control_dist / max(chord, 0.001);
    let segments = u32(max(2.0, ceil(curvature * 15.0 / tolerance)));
    return min(segments, 48u);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let cmd_idx = global_id.x;
    
    if (cmd_idx >= uniforms.command_count) {
        return;
    }
    
    let cmd = commands[cmd_idx];
    
    // Get the start position from previous command
    var start_pos: vec2<f32>;
    if (cmd_idx > 0u) {
        start_pos = commands[cmd_idx - 1u].p0;
    } else {
        start_pos = vec2<f32>(0.0, 0.0);
    }
    
    // Process each command type
    switch (cmd.cmd_type) {
        case CMD_MOVE_TO: {
            // MoveTo sets position but generates no geometry
        }
        case CMD_LINE_TO: {
            // LineTo: generate a single line segment (2 vertices)
            let idx = atomicAdd(&uniforms.vertex_count, 2u);
            if (idx + 1u < uniforms.max_vertices) {
                vertices[idx].position = start_pos;
                vertices[idx].tex_coords = vec2<f32>(0.0, 0.0);
                vertices[idx + 1u].position = cmd.p0;
                vertices[idx + 1u].tex_coords = vec2<f32>(1.0, 0.0);
            }
        }
        case CMD_QUADRATIC_TO: {
            // Tessellate quadratic Bezier
            let segments = calc_quadratic_segments(start_pos, cmd.p1, cmd.p0, uniforms.tolerance);
            
            for (var i = 0u; i <= segments; i = i + 1u) {
                let t = f32(i) / f32(segments);
                let pos = eval_quadratic(start_pos, cmd.p1, cmd.p0, t);
                let tex = vec2<f32>(t, 0.0);
                
                let idx = atomicAdd(&uniforms.vertex_count, 1u);
                if (idx < uniforms.max_vertices) {
                    vertices[idx].position = pos;
                    vertices[idx].tex_coords = tex;
                }
            }
        }
        case CMD_CUBIC_TO: {
            // Tessellate cubic Bezier
            let segments = calc_cubic_segments(start_pos, cmd.p1, cmd.p2, cmd.p0, uniforms.tolerance);
            
            for (var i = 0u; i <= segments; i = i + 1u) {
                let t = f32(i) / f32(segments);
                let pos = eval_cubic(start_pos, cmd.p1, cmd.p2, cmd.p0, t);
                let tex = vec2<f32>(t, 0.0);
                
                let idx = atomicAdd(&uniforms.vertex_count, 1u);
                if (idx < uniforms.max_vertices) {
                    vertices[idx].position = pos;
                    vertices[idx].tex_coords = tex;
                }
            }
        }
        case CMD_CLOSE: {
            // Close path connects back to the start
            // Generate line back to first vertex if needed
        }
        default: {}
    }
}
