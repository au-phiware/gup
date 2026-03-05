// Barnes-Hut tree-traversal repulsion compute shader.
//
// Replaces the O(n²) pairwise repulsion with O(n log n) tree traversal.
// The quadtree is built on the CPU and uploaded each iteration.
// Each invocation traverses the tree for one node using a stack.

// ---------------------------------------------------------------------------
// Types (must match Rust-side #[repr(C)] structs)
// ---------------------------------------------------------------------------

struct GpuNode {
    pos_x: f32,
    pos_y: f32,
    vel_x: f32,
    vel_y: f32,
}

struct SimParams {
    repulsion_strength: f32,
    spring_strength:    f32,
    spring_rest_length: f32,
    gravity:            f32,
    damping:            f32,
    node_count:         u32,
    edge_count:         u32,
    theta:              f32,
}

struct BHCell {
    com_x:      f32,
    com_y:      f32,
    mass:       f32,
    half_width: f32,
    child0:     i32,
    child1:     i32,
    child2:     i32,
    child3:     i32,
}

// ---------------------------------------------------------------------------
// Bind groups
// ---------------------------------------------------------------------------

// Group 0 — shared layout with the other force-layout passes.
@group(0) @binding(0) var<storage, read_write> nodes:  array<GpuNode>;
@group(0) @binding(2) var<storage, read_write> forces: array<f32>;
@group(0) @binding(3) var<uniform>             params: SimParams;

// Group 1 — Barnes-Hut tree (read-only, rebuilt each iteration).
@group(1) @binding(0) var<storage, read>       tree:   array<BHCell>;

// ---------------------------------------------------------------------------
// Barnes-Hut repulsion pass
//
// For each node, walk the quadtree.  At each cell:
//   - If the cell is a single body or width/distance < theta, treat the
//     cell as a point mass at its centre of mass.
//   - Otherwise, open the cell and push its children onto the stack.
// ---------------------------------------------------------------------------

const MAX_STACK: u32 = 64u;

@compute @workgroup_size(256)
fn bh_repulsion_pass(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.node_count {
        return;
    }

    let pi = vec2<f32>(nodes[i].pos_x, nodes[i].pos_y);
    var force = vec2<f32>(0.0, 0.0);

    // Explicit stack for iterative tree traversal.
    var stack: array<i32, 64>;
    var sp: u32 = 0u;

    // Push root (index 0).
    stack[0] = 0;
    sp = 1u;

    while sp > 0u {
        sp -= 1u;
        let cell_idx = u32(stack[sp]);
        let cell = tree[cell_idx];

        if cell.mass <= 0.0 {
            continue;
        }

        let com = vec2<f32>(cell.com_x, cell.com_y);
        let diff = pi - com;
        let dist_sq = max(dot(diff, diff), 0.01);

        // Decide: approximate or open.
        let is_leaf = cell.child0 < 0 && cell.child1 < 0 && cell.child2 < 0 && cell.child3 < 0;
        let cell_width = cell.half_width * 2.0;
        let ratio = cell_width / sqrt(dist_sq);

        if is_leaf || ratio < params.theta {
            // Use centre-of-mass approximation.
            // Coulomb-like: F = strength * mass / dist²  (away from COM)
            if dist_sq > 0.01 {
                let dir = diff / sqrt(dist_sq);  // normalise
                force += dir * (params.repulsion_strength * cell.mass / dist_sq);
            }
        } else {
            // Open the cell — push non-empty children.
            if cell.child3 >= 0 && sp < MAX_STACK {
                stack[sp] = cell.child3;
                sp += 1u;
            }
            if cell.child2 >= 0 && sp < MAX_STACK {
                stack[sp] = cell.child2;
                sp += 1u;
            }
            if cell.child1 >= 0 && sp < MAX_STACK {
                stack[sp] = cell.child1;
                sp += 1u;
            }
            if cell.child0 >= 0 && sp < MAX_STACK {
                stack[sp] = cell.child0;
                sp += 1u;
            }
        }
    }

    // Accumulate into force buffer (2 floats per node).
    forces[i * 2u]     += force.x;
    forces[i * 2u + 1u] += force.y;
}
