// GPU compute shader for binary treemap layout.
//
// Binary subdivision: children of a parent are recursively split into two
// groups of roughly equal total value, alternating the cut direction.
//
// Each thread processes one node at the current depth.  It determines
// its bounding rectangle by iteratively narrowing via binary splits
// using prefix-sum lookups.  The number of split levels is
// ceil(log2(child_count)).

const WORKGROUP_SIZE: u32 = 256u;

struct TreeNode {
    parent:      u32,
    child_start: u32,
    child_count: u32,
    depth:       u32,
}

struct Cell {
    x:          f32,
    y:          f32,
    width:      f32,
    height:     f32,
    depth:      u32,
    value:      f32,
    node_index: u32,
    _pad:       u32,
}

struct Params {
    viewport_x: f32,
    viewport_y: f32,
    viewport_w: f32,
    viewport_h: f32,
    node_count: u32,
    max_depth:  u32,
    algorithm:  u32,
    padding:    f32,
    current_depth: u32,
    _pad1:      u32,
    _pad2:      u32,
    _pad3:      u32,
}

@group(0) @binding(0) var<storage, read>       nodes:       array<TreeNode>;
@group(0) @binding(1) var<storage, read>       values:      array<f32>;
@group(0) @binding(2) var<storage, read>       prefix_sums: array<f32>;
@group(0) @binding(3) var<storage, read_write> cells:       array<Cell>;
@group(0) @binding(4) var<uniform>             params:      Params;

// Compute the sum of values[lo..hi] using the exclusive prefix sum.
fn range_sum(lo: u32, hi: u32) -> f32 {
    // prefix_sums is exclusive: prefix_sums[i] = sum of values[0..i].
    // So sum(values[lo..hi]) = prefix_sums[hi] - prefix_sums[lo].
    return prefix_sums[hi] - prefix_sums[lo];
}

// Find the split index within [lo, hi) that best divides total value in half.
// Returns the split point such that [lo, split) and [split, hi) are the two groups.
fn find_split(lo: u32, hi: u32) -> u32 {
    let count = hi - lo;
    if (count <= 1u) {
        return hi;
    }

    let total = range_sum(lo, hi);
    let half = total * 0.5;

    var best_split = lo + 1u;
    var best_diff = abs(range_sum(lo, lo + 1u) - half);

    for (var k = lo + 2u; k < hi; k++) {
        let left_sum = range_sum(lo, k);
        let diff = abs(left_sum - half);
        if (diff < best_diff) {
            best_diff = diff;
            best_split = k;
        }
    }

    return best_split;
}

@compute @workgroup_size(256)
fn binary_layout(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let n = params.node_count;
    if (idx >= n) {
        return;
    }

    let depth = params.current_depth;

    // Depth 0: root cell = viewport.
    if (depth == 0u && idx == 0u) {
        cells[0] = Cell(
            params.viewport_x,
            params.viewport_y,
            params.viewport_w,
            params.viewport_h,
            0u,
            values[0],
            0u,
            0u,
        );
        return;
    }

    let node = nodes[idx];
    if (node.depth != depth) {
        return;
    }

    let parent_idx = node.parent;
    if (parent_idx >= n) {
        return;
    }

    let pc = cells[parent_idx];
    let pad = params.padding;

    let pr_x = pc.x + pad;
    let pr_y = pc.y + pad;
    let pr_w = max(pc.width - 2.0 * pad, 0.0);
    let pr_h = max(pc.height - 2.0 * pad, 0.0);

    let parent_node = nodes[parent_idx];
    let cs = parent_node.child_start;
    let cc = parent_node.child_count;

    let total = values[parent_idx];
    if (total <= 0.0 || cc == 0u) {
        cells[idx] = Cell(pr_x, pr_y, 0.0, 0.0, depth, values[idx], idx, 0u);
        return;
    }

    if (cc == 1u) {
        cells[idx] = Cell(pr_x, pr_y, pr_w, pr_h, depth, values[idx], idx, 0u);
        return;
    }

    // Iterative binary subdivision: narrow the range [lo, hi) and
    // corresponding rectangle until this node is the sole element.
    var lo = cs;
    var hi = cs + cc;
    var rx = pr_x;
    var ry = pr_y;
    var rw = pr_w;
    var rh = pr_h;
    var horizontal = true;

    for (var iter = 0u; iter < 32u; iter++) {
        let count = hi - lo;
        if (count <= 1u) {
            break;
        }

        let split = find_split(lo, hi);
        let range_total = range_sum(lo, hi);
        if (range_total <= 0.0) {
            break;
        }
        let left_sum = range_sum(lo, split);
        let left_frac = left_sum / range_total;

        // Determine which half this node falls into.
        let in_left = idx < split;

        if (horizontal) {
            let lw = rw * left_frac;
            if (in_left) {
                rw = lw;
                hi = split;
            } else {
                rx = rx + lw;
                rw = rw - lw;
                lo = split;
            }
        } else {
            let lh = rh * left_frac;
            if (in_left) {
                rh = lh;
                hi = split;
            } else {
                ry = ry + lh;
                rh = rh - lh;
                lo = split;
            }
        }

        horizontal = !horizontal;
    }

    cells[idx] = Cell(rx, ry, rw, rh, depth, values[idx], idx, 0u);
}
