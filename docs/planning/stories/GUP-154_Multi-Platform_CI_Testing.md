# GUP-154: Multi-Platform CI Testing

**Priority**: Medium  
**Complexity**: High  
**Created**: 2025-02-22  
**Status**: 💡 New  
**Dependencies**: GUP-082 (Debug Tool Integration with CI/CD)

## Problem Statement

GUP-082 provides CI/CD performance testing, but only tests on a single GPU
platform. GPU performance characteristics vary significantly across vendors
(NVIDIA, AMD, Intel) and even across different models within a vendor's lineup.

## Motivation

To ensure Gup performs well across the entire ecosystem, we need to:

- Test on multiple GPU vendors and models
- Detect vendor-specific performance regressions
- Identify optimization opportunities for specific hardware
- Provide performance comparisons across platforms

## Proposed Solution

### Multi-Platform Testing Matrix

```yaml
# GitHub Actions matrix strategy
matrix:
  platform:
    - name: NVIDIA RTX
      runner: nvidia-gpu
    - name: AMD Radeon
      runner: amd-gpu
    - name: Intel Arc
      runner: intel-gpu
    - name: Software (CPU fallback)
      runner: ubuntu-latest
```

### Platform-Specific Baselines

```rust
pub struct PlatformBaseline {
    platform_id: String,
    gpu_vendor: GpuVendor,
    gpu_model: String,
    baseline: PerformanceBaseline,
}

pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Software, // CPU fallback
}
```

## Acceptance Criteria

- [ ] Test across NVIDIA, AMD, and Intel GPUs
- [ ] Maintain separate baselines per platform
- [ ] Detect platform-specific regressions
- [ ] Generate cross-platform performance comparison reports
- [ ] Support software (CPU) fallback testing

## Success Metrics

- **Platform Coverage**: Test on 3+ GPU vendors
- **Test Consistency**: Same test suite runs on all platforms
- **Detection Rate**: Catch 90%+ of platform-specific issues
- **Execution Time**: Complete multi-platform testing in <15 minutes

## Implementation Strategy

1. **Phase 1**: Extend baseline storage to support platform identification
   - Add `platform_id` to baseline files
   - Modify `BaselineStorage` to filter by platform
2. **Phase 2**: Configure GitHub Actions matrix for multiple platforms
   - Set up self-hosted runners with different GPUs
   - Add platform detection to test suite
3. **Phase 3**: Cross-platform comparison reporting
   - Generate comparison tables
   - Identify platform-specific optimizations

## Technical Approach

### Platform Detection

```rust
pub struct PlatformInfo {
    pub vendor: GpuVendor,
    pub model: String,
    pub driver_version: String,
    pub memory_gb: u32,
}

impl PlatformInfo {
    pub fn detect() -> GupResult<Self> {
        // Use wgpu adapter info to detect GPU
        let adapter_info = device.adapter.get_info();
        // Parse vendor, model from adapter info
    }

    pub fn platform_id(&self) -> String {
        format!("{:?}_{}", self.vendor, self.model)
    }
}
```

### Baseline Organization

```
baselines/performance/
├── nvidia_rtx3080/
│   ├── rendering/
│   │   ├── basic_rendering.json
│   │   └── large_dataset_rendering.json
│   └── compilation/
│       └── shader_compilation.json
├── amd_rx6800/
│   └── ...
└── intel_arc_a770/
    └── ...
```

### Cross-Platform Reporting

```markdown
## Performance by Platform

| Test                    | NVIDIA RTX 3080 | AMD RX 6800 | Intel Arc A770 | Software |
| ----------------------- | --------------- | ----------- | -------------- | -------- |
| basic_rendering         | 5.1ms           | 5.8ms       | 6.2ms          | 45ms     |
| large_dataset_rendering | 15.0ms          | 16.2ms      | 17.5ms         | 180ms    |
| shader_compilation      | 8.0ms           | 9.1ms       | 8.5ms          | N/A      |
```

## Infrastructure Requirements

### Self-Hosted Runners

- **NVIDIA Runner**: Linux machine with RTX 3070 or better
- **AMD Runner**: Linux machine with RX 6000 series
- **Intel Runner**: Linux machine with Arc A-series
- **Software Runner**: Standard GitHub-hosted runner (no GPU)

### Costs and Alternatives

**Option A: Self-hosted runners**

- Pros: Full control, consistent hardware, no usage limits
- Cons: Requires dedicated machines, maintenance overhead

**Option B: Cloud GPU instances**

- Pros: No hardware management, flexible scaling
- Cons: Usage costs, potential inconsistency across runs

**Option C: Hybrid approach**

- Common platforms (NVIDIA) on cloud
- Less common platforms (AMD, Intel) on self-hosted
- Software fallback on GitHub-hosted runners

## Dependencies

- GUP-082 (Debug Tool Integration with CI/CD) - Required
- Infrastructure: Self-hosted runners or cloud GPU instances

## Follow-up Opportunities

- Vendor-specific optimization profiles
- Performance heatmaps across GPU generations
- Automatic GPU feature detection and fallback
- Cost-performance analysis tools
