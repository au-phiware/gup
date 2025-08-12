# GUP-090: Automatic Tick Generation Algorithm

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Automatic Scale and Axis System  
**Priority**: High  
**Story Points**: 8  
**Status**: 📋 Planned

## Problem Statement

Users creating data visualizations need professionally-spaced tick marks that
automatically adapt to data ranges and display sizes. Current axis
infrastructure (GUP-089) provides basic tick rendering but lacks the intelligent
algorithms needed to determine optimal tick intervals, positions, and densities.
Without smart tick generation, charts appear unprofessional with either too few
ticks (losing precision) or too many ticks (creating visual clutter).

## Business Context

Professional data visualization requires tick marks that follow established
cartographic and statistical conventions. Users expect charts to "just work"
with appropriate tick spacing without manual configuration, similar to how
Excel, matplotlib, and D3.js handle automatic tick generation.

## Acceptance Criteria

### Automatic Tick Generation

- [ ] **Linear scale tick generation** with nice intervals (1, 2, 5, 10, etc.
      multiples)
- [ ] **Logarithmic scale tick handling** with appropriate base-10 intervals
- [ ] **Time scale intelligent ticking** (seconds, minutes, hours, days, months,
      years)
- [ ] **Density-aware algorithms** that prevent over-crowding in small spaces
- [ ] **Major/minor tick coordination** with configurable subdivision ratios

### Algorithm Quality

- [ ] **Nice number selection** following Wilkinson's algorithm or similar best
      practices
- [ ] **Viewport adaptation** - fewer ticks on mobile, more on desktop displays
- [ ] **Scale range handling** from micro-values (0.001) to large values (1M+)
- [ ] **Edge case robustness** (zero ranges, infinite values, NaN handling)
- [ ] **Consistent spacing** that maintains visual rhythm across different
      scales

### Performance Requirements

- [ ] **<1ms tick calculation** for any reasonable scale range and viewport size
- [ ] **Deterministic results** - same inputs always produce same tick positions
- [ ] **Memory efficient** - minimal allocation during tick generation
- [ ] **Thread-safe algorithms** for concurrent chart generation

### Integration Requirements

- [ ] **Scale trait integration** working with all scale types (linear, log,
      time, ordinal)
- [ ] **Axis system compatibility** providing tick positions to GUP-089
      infrastructure
- [ ] **Chart builder integration** automatic tick generation when axes are
      enabled
- [ ] **Override capability** allowing manual tick specification when needed

## Technical Requirements

### Tick Generation Interface

```rust
pub trait TickGenerator: Send + Sync + 'static {
    /// Generate major tick positions for given scale and display constraints
    fn generate_major_ticks(
        &self,
        scale: &dyn Scale,
        pixel_range: f32,
        target_tick_count: Option<usize>
    ) -> Vec<f64>;

    /// Generate minor tick positions between major ticks
    fn generate_minor_ticks(
        &self,
        scale: &dyn Scale,
        major_ticks: &[f64],
        subdivisions: usize
    ) -> Vec<f64>;

    /// Calculate optimal tick density for given display size
    fn calculate_target_density(&self, pixel_range: f32) -> usize;
}
```

### Linear Scale Algorithm

```rust
pub struct LinearTickGenerator {
    /// Minimum pixels between major ticks
    min_tick_spacing: f32,
    /// Maximum number of major ticks
    max_tick_count: usize,
    /// Preferred nice numbers for intervals
    nice_numbers: &'static [f64],
}

impl LinearTickGenerator {
    /// Wilkinson's extended algorithm for nice tick intervals
    fn calculate_nice_interval(&self, range: f64, target_count: usize) -> f64 {
        let raw_step = range / target_count as f64;
        let magnitude = 10f64.powf(raw_step.log10().floor());
        let normalized = raw_step / magnitude;

        // Select closest nice number
        let nice_normalized = self.find_closest_nice_number(normalized);
        nice_normalized * magnitude
    }

    fn find_closest_nice_number(&self, value: f64) -> f64 {
        self.nice_numbers
            .iter()
            .min_by(|&a, &b| (a - value).abs().partial_cmp(&(b - value).abs()).unwrap())
            .copied()
            .unwrap_or(1.0)
    }
}

const NICE_NUMBERS: &[f64] = &[1.0, 2.0, 2.5, 5.0, 10.0];
```

### Logarithmic Scale Algorithm

```rust
pub struct LogarithmicTickGenerator {
    base: f64,
    /// Whether to include intermediate ticks (2,3,4,5,6,7,8,9) between powers
    include_intermediate: bool,
}

impl LogarithmicTickGenerator {
    fn generate_log_ticks(&self, min_exp: i32, max_exp: i32) -> Vec<f64> {
        let mut ticks = Vec::new();

        for exp in min_exp..=max_exp {
            let base_value = self.base.powi(exp);
            ticks.push(base_value);

            if self.include_intermediate {
                for i in 2..=(self.base as i32) {
                    if exp < max_exp || i as f64 * base_value <= self.base.powi(max_exp) {
                        ticks.push(i as f64 * base_value);
                    }
                }
            }
        }

        ticks
    }
}
```

### Time Scale Algorithm

```rust
pub struct TimeTickGenerator {
    /// Available time intervals in ascending order
    intervals: &'static [TimeInterval],
}

#[derive(Debug, Clone, Copy)]
pub struct TimeInterval {
    pub unit: TimeUnit,
    pub count: u32,
    pub milliseconds: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum TimeUnit {
    Millisecond, Second, Minute, Hour, Day, Week, Month, Year
}

const TIME_INTERVALS: &[TimeInterval] = &[
    TimeInterval { unit: TimeUnit::Millisecond, count: 1, milliseconds: 1 },
    TimeInterval { unit: TimeUnit::Millisecond, count: 10, milliseconds: 10 },
    TimeInterval { unit: TimeUnit::Millisecond, count: 100, milliseconds: 100 },
    TimeInterval { unit: TimeUnit::Second, count: 1, milliseconds: 1000 },
    TimeInterval { unit: TimeUnit::Second, count: 5, milliseconds: 5000 },
    TimeInterval { unit: TimeUnit::Second, count: 15, milliseconds: 15000 },
    TimeInterval { unit: TimeUnit::Second, count: 30, milliseconds: 30000 },
    TimeInterval { unit: TimeUnit::Minute, count: 1, milliseconds: 60000 },
    // ... continuing with hours, days, weeks, months, years
];
```

## Dependencies

### Required Stories (Must Complete First)

- **GUP-089**: Core Axis System Infrastructure (provides Axis trait and basic
  rendering)

### Strongly Related Stories

- **GUP-093**: Scale-Axis Integration System (consumes tick positions from this
  story)
- **GUP-005**: Shader Function Trait ✅ (scale implementations need to
  integrate)

## User Stories

### As a Data Analyst

> "I want my charts to automatically show the right number of tick marks so that
> I can quickly read approximate values without the axes looking cluttered or
> sparse."

**Scenario**: Creating quarterly sales charts with varying value ranges  
**Expected**: Tick marks automatically space appropriately (e.g., $0, $50K,
$100K, $150K)  
**Acceptance**: Charts display professional-looking tick spacing regardless of
data range

### As a Financial Dashboard Developer

> "I want time-series charts to show meaningful time intervals so that users can
> easily correlate events with timestamps."

**Scenario**: Stock price charts covering different time periods (1 day, 1
month, 1 year)  
**Expected**: Time ticks adapt intelligently (minutes for 1 day, days for 1
month, months for 1 year)  
**Acceptance**: Time axis shows appropriate intervals that make temporal
patterns clear

### As a Scientific Researcher

> "I want logarithmic scales to show proper decade markers so that exponential
> data patterns are clearly visible."

**Scenario**: Plotting concentration data spanning 6 orders of magnitude  
**Expected**: Ticks at 10^-6, 10^-4, 10^-2, 10^0, 10^2, 10^4 with optional
intermediate marks  
**Acceptance**: Log scales display standard scientific notation intervals

## Implementation Approach

### Phase 1: Linear Scale Algorithm (3 days)

1. **Implement Wilkinson's algorithm** for nice number selection
2. **Density calculation** based on pixel spacing and target counts
3. **Edge case handling** for zero ranges, negative values, very large/small
   numbers
4. **Unit testing** with comprehensive test cases

### Phase 2: Logarithmic and Time Scales (3 days)

1. **Logarithmic tick generation** with base-10 and configurable bases
2. **Time scale intervals** with intelligent unit selection
3. **Minor tick subdivision** for all scale types
4. **Integration testing** with scale implementations

### Phase 3: Performance and Integration (2 days)

1. **Performance optimization** to meet <1ms target
2. **Integration with axis system** providing tick positions
3. **Chart builder integration** for automatic behavior
4. **Cross-platform validation** ensuring consistent results

## Testing Strategy

### Unit Tests

- Nice number calculation accuracy
- Edge cases (zero range, infinite values, NaN)
- Time interval selection logic
- Logarithmic decade calculation
- Performance benchmarks

### Algorithm Validation Tests

- Comparison with established libraries (D3.js, matplotlib)
- Visual inspection of generated tick patterns
- Density validation across different viewport sizes
- Cross-platform result consistency

### Integration Tests

- Scale system integration
- Axis rendering with generated ticks
- Chart builder automatic behavior
- Multi-axis coordination

## Success Metrics

### Algorithm Quality Metrics

- ✅ **Nice interval selection** matches or exceeds D3.js quality
- ✅ **Density appropriateness** - not too sparse (<3 ticks) or dense (>15 major
  ticks)
- ✅ **Visual rhythm consistency** across different scales and ranges
- ✅ **Edge case robustness** handles all reasonable input ranges

### Performance Targets

- ✅ **<1ms calculation time** for any scale range and viewport
- ✅ **<100 bytes allocation** during tick generation
- ✅ **Deterministic results** - same inputs produce identical outputs
- ✅ **Thread safety** for concurrent chart generation

### Integration Success

- ✅ **Automatic behavior** - works out-of-box without configuration
- ✅ **Override capability** - manual tick specification remains possible
- ✅ **Scale compatibility** - works with all current and future scale types
- ✅ **Cross-platform consistency** - identical results on all targets

## Risks and Mitigations

### Algorithm Complexity Risk

**Risk**: Wilkinson's algorithm implementation becomes complex and error-prone  
**Likelihood**: Medium  
**Impact**: Medium  
**Mitigation**: Start with simplified nice number approach, add complexity
incrementally with extensive testing

### Algorithm Performance Risk

**Risk**: Complex calculations slow down chart generation  
**Likelihood**: Low  
**Impact**: Medium  
**Mitigation**: Profile early and often, use lookup tables and caching where
appropriate

### Cross-Platform Consistency Risk

**Risk**: Floating-point calculations produce different results on different
platforms  
**Likelihood**: Medium  
**Impact**: High  
**Mitigation**: Use deterministic algorithms, comprehensive cross-platform
testing, consider fixed-point arithmetic for critical calculations

## Follow-up Stories

This story enables:

- **GUP-091**: Grid Line Rendering System (uses tick positions for grid
  alignment)
- **GUP-092**: Label Formatting and Positioning (uses tick positions for label
  placement)
- **GUP-093**: Scale-Axis Integration System (coordinates tick generation with
  scales)

## Definition of Done

- [ ] All acceptance criteria verified through automated tests
- [ ] Algorithm quality validated against established benchmarks (D3.js,
      matplotlib)
- [ ] Performance targets met with comprehensive benchmarking
- [ ] Integration complete with axis system and chart builders
- [ ] Edge case handling verified through stress testing
- [ ] Cross-platform consistency validated
- [ ] Documentation with algorithm explanations and examples
- [ ] Code review completed with team approval

---

**Business Value**: Enables professional-quality automatic tick generation that
eliminates user configuration burden while ensuring charts meet established
visualization standards.

**Technical Value**: Provides reusable, high-performance algorithms that
integrate seamlessly with the scale and axis systems while maintaining
deterministic behavior across platforms.
