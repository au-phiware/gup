// Force-directed graph layout compute shaders.
//
// Four entry points that are dispatched in sequence each iteration:
//   1. repulsion_pass  — pairwise Coulomb repulsion  O(n²)
//   2. spring_pass     — Hooke-law edge springs
//   3. integrate_pass  — Euler integration + gravity + damping
//   4. convergence_pass — parallel max-displacement reduction

// ---------------------------------------------------------------------------
// Types (must match Rust-side #[repr(C)] structs)
// ---------------------------------------------------------------------------

struct GpuNode {
    pos_x: f32,
    pos_y: f32,
    vel_x: f32,
    vel_y: f32,
}

struct GpuEdge {
    source: u32,
    target: u32,
}

struct SimParams {
    repulsion_strength: f32,
    spring_strength:    f32,
    spring_rest_length: f32,
    gravity:            f32,
    damping:            f32,
    node_count:         u32,
    edge_count:         u32,
    _pad:               u32,
}

// ---------------------------------------------------------------------------
// Bind group (shared across all passes)
// ---------------------------------------------------------------------------

@group(0) @binding(0) var<storage, read_write> nodes:        array<GpuNode>;
@group(0) @binding(1) var<storage, read>       edges:        array<GpuEdge>;
@group(0) @binding(2) var<storage, read_write> forces:       array<f32>;  // 2 floats per node (fx, fy)
@group(0) @binding(3) var<uniform>             params:       SimParams;
@group(0) @binding(4) var<storage, read_write> convergence:  array<atomic<u32>>;

// ---------------------------------------------------------------------------
// 1. Repulsion pass — O(n²) pairwise
//
// Each invocation (one per node) loops over every other node and
// accumulates a Coulomb-like repulsive force: F = strength / d²
// in the direction away from the other node.
// ---------------------------------------------------------------------------

@compute @workgroup_size(256)
fn repulsion_pass(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.node_count {
        return;
    }

    let pi = vec2<f32>(nodes[i].pos_x, nodes[i].pos_y);
    var force = vec2<f32>(0.0, 0.0);

    for (var j = 0u; j < params.node_count; j = j + 1u) {
        if j == i {
            continue;
        }
        let pj = vec2<f32>(nodes[j].pos_x, nodes[j].pos_y);
        var diff = pi - pj;
        let dist_sq = max(dot(diff, diff), 0.01);  // avoid division by zero
        // Coulomb: F = strength / dist²  (direction: away from j)
        force += normalize(diff) * (params.repulsion_strength / dist_sq);
    }

    // Accumulate into force buffer (2 floats per node)
    forces[i * 2u]     += force.x;
    forces[i * 2u + 1u] += force.y;
}

// ---------------------------------------------------------------------------
// 2. Spring pass — Hooke-law attraction along edges
//
// Each invocation (one per edge) computes a spring force and adds it
// to both endpoints atomically.
// ---------------------------------------------------------------------------

@compute @workgroup_size(256)
fn spring_pass(@builtin(global_invocation_id) gid: vec3<u32>) {
    let e = gid.x;
    if e >= params.edge_count {
        return;
    }

    let src = edges[e].source;
    let tgt = edges[e].target;

    let ps = vec2<f32>(nodes[src].pos_x, nodes[src].pos_y);
    let pt = vec2<f32>(nodes[tgt].pos_x, nodes[tgt].pos_y);

    let diff = pt - ps;
    let dist = max(length(diff), 0.001);
    let displacement = dist - params.spring_rest_length;

    // Hooke: F = k * (dist - rest) in the direction of the other node
    let f = normalize(diff) * (params.spring_strength * displacement);

    // Add to source (toward target)
    forces[src * 2u]     += f.x;
    forces[src * 2u + 1u] += f.y;
    // Add to target (toward source = negative)
    forces[tgt * 2u]     -= f.x;
    forces[tgt * 2u + 1u] -= f.y;
}

// ---------------------------------------------------------------------------
// 3. Integration pass — Euler step with gravity and damping
//
// - Add gravity toward centre (0,0)
// - Add accumulated forces to velocity
// - Apply damping
// - Update position
// ---------------------------------------------------------------------------

@compute @workgroup_size(256)
fn integrate_pass(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.node_count {
        return;
    }

    let pos = vec2<f32>(nodes[i].pos_x, nodes[i].pos_y);
    var vel = vec2<f32>(nodes[i].vel_x, nodes[i].vel_y);

    // Read accumulated forces
    let f = vec2<f32>(forces[i * 2u], forces[i * 2u + 1u]);

    // Gravity toward origin
    let grav = -pos * params.gravity;

    // Update velocity
    vel = (vel + f + grav) * params.damping;

    // Clamp velocity to avoid explosion
    let max_vel = 50.0;
    let speed = length(vel);
    if speed > max_vel {
        vel = vel * (max_vel / speed);
    }

    // Update position
    let new_pos = pos + vel;

    nodes[i].pos_x = new_pos.x;
    nodes[i].pos_y = new_pos.y;
    nodes[i].vel_x = vel.x;
    nodes[i].vel_y = vel.y;
}

// ---------------------------------------------------------------------------
// 4. Convergence pass — compute max displacement (length of velocity)
//
// Uses atomicMax on the convergence buffer (slot 0) with the float bits
// of the displacement.  This works because positive IEEE 754 floats sort
// correctly as unsigned integers.
// ---------------------------------------------------------------------------

@compute @workgroup_size(256)
fn convergence_pass(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.node_count {
        return;
    }

    let vel = vec2<f32>(nodes[i].vel_x, nodes[i].vel_y);
    let disp = length(vel);
    let bits = bitcast<u32>(disp);

    atomicMax(&convergence[0], bits);
}
