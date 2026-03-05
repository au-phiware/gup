// GPU compute shader for slice-and-dice treemap layout.
//
// Each thread processes one node.  If the node's depth matches the
// current processing depth (set in params), the thread computes the
// node's bounding rectangle from its parent's rectangle and the
// prefix-sum offsets of sibling values.
//
// The shader is dispatched once per depth level (0, 1, …, max_depth).

const WORKGROUP_SIZE: u32 = 256u;

// Tree node (matches Rust `GpuTreeNode`).
struct TreeNode {
    parent:      u32, // u32::MAX for root
    child_start: u32,
    child_count: u32,
    depth:       u32,
}

// Output cell (matches Rust `TreemapCell`, 32 bytes).
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

// Layout parameters.
struct Params {
    viewport_x: f32,
    viewport_y: f32,
    viewport_w: f32,
    viewport_h: f32,
    node_count: u32,
    max_depth:  u32, // u32::MAX = unlimited
    algorithm:  u32, // unused here (always slice-dice)
    padding:    f32,
    current_depth: u32,
    _pad1:      u32,
    _pad2:      u32,
    _pad3:      u32,
}

@group(0) @binding(0) var<storage, read>       nodes:       array<TreeNode>;
@group(0) @binding(1) var<storage, read>       values:      array<f32>;      // subtree sums
@group(0) @binding(2) var<storage, read>       prefix_sums: array<f32>;      // exclusive prefix sum of values
@group(0) @binding(3) var<storage, read_write> cells:       array<Cell>;
@group(0) @binding(4) var<uniform>             params:      Params;

@compute @workgroup_size(256)
fn slice_dice_layout(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let n = params.node_count;
    if (idx >= n) {
        return;
    }

    let depth = params.current_depth;

    // Depth 0: set root cell to viewport.
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

    // Only process nodes at the current depth.
    let node = nodes[idx];
    if (node.depth != depth) {
        return;
    }

    let parent_idx = node.parent;
    if (parent_idx >= n) {
        return;
    }

    // Read parent cell (written in previous depth dispatch).
    let pc = cells[parent_idx];
    let pad = params.padding;

    // Padded parent rect.
    let pr_x = pc.x + pad;
    let pr_y = pc.y + pad;
    let pr_w = max(pc.width - 2.0 * pad, 0.0);
    let pr_h = max(pc.height - 2.0 * pad, 0.0);

    let parent_node = nodes[parent_idx];
    let cs = parent_node.child_start;
    let cc = parent_node.child_count;

    // Total value of parent's children = subtree sum of parent.
    let total = values[parent_idx];
    if (total <= 0.0 || cc == 0u) {
        cells[idx] = Cell(pr_x, pr_y, 0.0, 0.0, depth, values[idx], idx, 0u);
        return;
    }

    // Prefix sum offset for this child among siblings.
    // prefix_sums is an exclusive prefix sum, so:
    //   offset = prefix_sums[idx] - prefix_sums[cs]
    let offset_value = prefix_sums[idx] - prefix_sums[cs];
    let my_value = values[idx];

    // Slice-dice: alternate horizontal/vertical based on parent depth.
    let parent_depth = parent_node.depth;
    let horizontal = (parent_depth % 2u) == 0u;

    var cell_x: f32;
    var cell_y: f32;
    var cell_w: f32;
    var cell_h: f32;

    if (horizontal) {
        let frac = my_value / total;
        let offset_frac = offset_value / total;
        cell_x = pr_x + offset_frac * pr_w;
        cell_y = pr_y;
        cell_w = frac * pr_w;
        cell_h = pr_h;
    } else {
        let frac = my_value / total;
        let offset_frac = offset_value / total;
        cell_x = pr_x;
        cell_y = pr_y + offset_frac * pr_h;
        cell_w = pr_w;
        cell_h = frac * pr_h;
    }

    cells[idx] = Cell(cell_x, cell_y, cell_w, cell_h, depth, my_value, idx, 0u);
}
