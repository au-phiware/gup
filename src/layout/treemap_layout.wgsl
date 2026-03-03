// Treemap layout compute shader (placeholder).
//
// The current implementation dispatches treemap layout on the CPU.
// This shader is reserved for future GPU-accelerated treemap computation.
// The SliceDice and Binary algorithms are embarrassingly parallel and
// are prime candidates for GPU migration; the Squarified algorithm has
// sequential row-building dependencies that limit per-parent parallelism.

// Placeholder entry point — does nothing.
@compute @workgroup_size(256)
fn treemap_noop(@builtin(global_invocation_id) id: vec3<u32>) {
    // No-op: treemap layout is currently computed on the CPU.
}
