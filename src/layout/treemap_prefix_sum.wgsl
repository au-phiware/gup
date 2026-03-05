// Blelloch-style exclusive prefix sum (scan) for treemap layout.
//
// Three entry points implement a multi-workgroup scan:
//   1. workgroup_scan  — per-workgroup Blelloch scan; stores block totals.
//   2. scan_block_sums — scans the block totals array (single workgroup).
//   3. add_block_sums  — adds scanned block totals back into per-element results.
//
// For inputs that fit in a single workgroup (≤ BLOCK_SIZE elements), only
// `workgroup_scan` is needed.

const BLOCK_SIZE: u32 = 256u;

// Input values (subtree sums).
@group(0) @binding(0) var<storage, read>       input:      array<f32>;
// Output prefix sums (exclusive).
@group(0) @binding(1) var<storage, read_write> output:     array<f32>;
// Per-workgroup block totals (length = ceil(n / BLOCK_SIZE)).
@group(0) @binding(2) var<storage, read_write> block_sums: array<f32>;
// Params: [0] = element count.
@group(0) @binding(3) var<uniform>             params:     vec4<u32>;

var<workgroup> wg_scratch: array<f32, 256>;

// ---------- Pass 1: per-workgroup Blelloch exclusive scan ----------

@compute @workgroup_size(256)
fn workgroup_scan(@builtin(global_invocation_id) gid: vec3<u32>,
                  @builtin(local_invocation_id)  lid: vec3<u32>,
                  @builtin(workgroup_id)         wid: vec3<u32>) {
    let n = params.x;
    let idx = gid.x;
    let local = lid.x;

    // Load input into wg_scratch memory (0 for out-of-bounds).
    if (idx < n) {
        wg_scratch[local] = input[idx];
    } else {
        wg_scratch[local] = 0.0;
    }
    workgroupBarrier();

    // Up-sweep (reduce) phase.
    for (var stride = 1u; stride < BLOCK_SIZE; stride *= 2u) {
        let index = (local + 1u) * stride * 2u - 1u;
        if (index < BLOCK_SIZE) {
            wg_scratch[index] += wg_scratch[index - stride];
        }
        workgroupBarrier();
    }

    // Store block total and clear last element.
    if (local == 0u) {
        block_sums[wid.x] = wg_scratch[BLOCK_SIZE - 1u];
        wg_scratch[BLOCK_SIZE - 1u] = 0.0;
    }
    workgroupBarrier();

    // Down-sweep phase.
    for (var stride = BLOCK_SIZE / 2u; stride >= 1u; stride /= 2u) {
        let index = (local + 1u) * stride * 2u - 1u;
        if (index < BLOCK_SIZE) {
            let temp = wg_scratch[index - stride];
            wg_scratch[index - stride] = wg_scratch[index];
            wg_scratch[index] += temp;
        }
        workgroupBarrier();
    }

    // Write result.
    if (idx < n) {
        output[idx] = wg_scratch[local];
    }
}

// ---------- Pass 2: scan the block_sums array (single workgroup) ----------

// Reuses the same bindings; we set `params.x = num_blocks` and
// swap input/output to point at block_sums.  However, for simplicity
// we use a separate entry that reads/writes block_sums in-place.

@group(1) @binding(0) var<storage, read_write> block_sums_rw: array<f32>;
@group(1) @binding(1) var<uniform>             block_params:  vec4<u32>;

var<workgroup> wg_scratch2: array<f32, 256>;

@compute @workgroup_size(256)
fn scan_block_sums(@builtin(local_invocation_id) lid: vec3<u32>) {
    let n = block_params.x; // number of blocks
    let local = lid.x;

    if (local < n) {
        wg_scratch2[local] = block_sums_rw[local];
    } else {
        wg_scratch2[local] = 0.0;
    }
    workgroupBarrier();

    // Up-sweep.
    for (var stride = 1u; stride < BLOCK_SIZE; stride *= 2u) {
        let index = (local + 1u) * stride * 2u - 1u;
        if (index < BLOCK_SIZE) {
            wg_scratch2[index] += wg_scratch2[index - stride];
        }
        workgroupBarrier();
    }

    if (local == 0u) {
        wg_scratch2[BLOCK_SIZE - 1u] = 0.0;
    }
    workgroupBarrier();

    // Down-sweep.
    for (var stride = BLOCK_SIZE / 2u; stride >= 1u; stride /= 2u) {
        let index = (local + 1u) * stride * 2u - 1u;
        if (index < BLOCK_SIZE) {
            let temp = wg_scratch2[index - stride];
            wg_scratch2[index - stride] = wg_scratch2[index];
            wg_scratch2[index] += temp;
        }
        workgroupBarrier();
    }

    if (local < n) {
        block_sums_rw[local] = wg_scratch2[local];
    }
}

// ---------- Pass 3: add scanned block sums back ----------

@compute @workgroup_size(256)
fn add_block_sums(@builtin(global_invocation_id) gid: vec3<u32>,
                  @builtin(workgroup_id)         wid: vec3<u32>) {
    let n = params.x;
    let idx = gid.x;
    if (idx < n) {
        output[idx] += block_sums[wid.x];
    }
}
