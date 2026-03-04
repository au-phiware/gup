# CI Accessibility Checks

This document describes the automated accessibility checks integrated into Gup's
CI pipeline and how to run them locally.

## Overview

Gup's CI pipeline includes accessibility validation that runs on every pull
request. A failing accessibility check blocks merge. The checks are organised
into two tiers:

| Tier | Target | Tool | Runs On |
| --- | --- | --- | --- |
| **Rust unit tests** | All platforms | `cargo test` | Every PR |
| **AT-SPI2 validation** | Linux native | `at-spi2-core` introspection | Linux CI runners |

## Tier 1: Rust Accessibility Unit Tests

The primary accessibility gate is the Rust test suite. These tests validate:

- **Contrast ratio correctness** — sRGB linearization, WCAG AA thresholds
  (4.5:1 normal text, 3:1 large text / non-text UI)
- **ARIA tree integrity** — Node creation, role mapping, label presence, live
  region urgency levels
- **Keyboard navigation** — No keyboard traps, wrap-around behaviour, all
  navigation modes (sequential, spatial, data dimension)
- **Focus management** — Focus ring styles, focus history, context exit
- **Platform bridge** — Announcement delivery, focus delegation
- **Pattern rendering** — Distinct pattern types for non-colour encoding

### Running Locally

```bash
# Run all accessibility tests (single-threaded for GPU tests)
cargo test accessibility -- --test-threads=1

# Run only the WCAG regression tests
cargo test accessibility::high_contrast -- --test-threads=1

# Run ARIA tree tests
cargo test accessibility::aria -- --test-threads=1

# Run keyboard navigation tests
cargo test accessibility::keyboard -- --test-threads=1
```

### Interpreting Failures

A test failure indicates a WCAG regression. The test name and assertion message
identify which criterion is affected:

| Test prefix | WCAG Criterion |
| --- | --- |
| `test_wcag_contrast_ratio_*` | 1.4.3 Contrast (Minimum) |
| `test_meets_wcag_aa*` | 1.4.3, 1.4.11 Non-text Contrast |
| `test_srgb_linearization*` | 1.4.3 (formula correctness) |
| `test_high_contrast_mode*` | 1.4.1 Use of Color |
| `test_aria_*` | 4.1.2 Name, Role, Value |
| `test_keyboard_navigation*` | 2.1.1 Keyboard, 2.1.2 No Trap |
| `test_live_region*` | 4.1.3 Status Messages |
| `test_each_pattern*` | 1.4.1 Use of Color |

### CI Configuration

In the GitHub Actions workflow, the accessibility tests are part of the standard
test job:

```yaml
- name: Run tests
  run: cargo test -- --test-threads=1
```

No additional configuration is needed — the accessibility tests are compiled into
the main test binary.

## Tier 2: AT-SPI2 Validation (Linux)

On Linux CI runners, the AT-SPI2 accessibility bus can be used to validate that
Gup's platform bridge correctly exposes the ARIA tree to assistive technologies.

### Prerequisites

```bash
# Install AT-SPI2 tools (Ubuntu/Debian)
sudo apt-get install at-spi2-core libatspi2.0-dev

# Verify the bus is available
busctl --user list | grep org.a11y
```

### Running the Validation

```bash
# Start an AT-SPI2-enabled D-Bus session
eval $(dbus-launch --sh-syntax)
export DBUS_SESSION_BUS_ADDRESS

# Run the accessibility integration tests
cargo test atspi -- --test-threads=1
```

### Interpreting Failures

AT-SPI2 test failures indicate that the platform bridge is not correctly
translating the ARIA tree to the native accessibility API. Check:

1. **D-Bus connection** — Is the AT-SPI2 bus running?
2. **Role mapping** — Are `AriaRole` values correctly mapped to `AtkRole`?
3. **Property propagation** — Are labels and descriptions reaching the bus?

## Tier 3: Web/WASM Accessibility (Future)

When the WASM integration (GUP-117) is deployed with a DOM overlay, axe-core
scans can validate the generated HTML:

```bash
# Install axe-core CLI
npm install -g @axe-core/cli

# Run against a served WASM example
axe http://localhost:8080 --tags wcag2a,wcag2aa --exit
```

This tier is not yet integrated into CI because it requires a headless browser
environment. It is planned for a future story.

## Adding New Accessibility Tests

When adding a new accessibility feature:

1. Write unit tests in the relevant module under `src/accessibility/`
2. Include the WCAG criterion number in the test name or doc comment
3. Use the `meets_wcag_aa()` and `meets_wcag_aa_large_text()` helpers for
   contrast validation
4. Run `cargo test accessibility -- --test-threads=1` locally before pushing
5. The CI pipeline will catch any regressions automatically

## Severity Levels

| Severity | Action | Example |
| --- | --- | --- |
| **Critical** | Blocks merge | ARIA role returns empty string |
| **Serious** | Blocks merge | Contrast ratio below AA threshold |
| **Moderate** | Warning | Missing optional description |
| **Minor** | Informational | Suboptimal focus order |

All critical and serious violations block merge. Moderate and minor issues are
logged but do not block.
