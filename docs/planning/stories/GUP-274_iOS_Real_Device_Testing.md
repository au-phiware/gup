# GUP-274: iOS Real-Device Testing Guide

## Story Overview

**Initiative**: Mobile **Status**: 📋 Planned **Created**: 2025-07-24

## Context

GUP-270 delivered iOS platform support with CI verification on the iOS
Simulator. However, Simulator GPU behaviour differs from real Metal hardware,
and performance numbers measured on Simulator do not transfer to device.

This story documents the workflow for testing on a physical iPhone/iPad and
establishes a real-device performance baseline.

## User Story

> "As a Gup contributor working on iOS features, I want a documented workflow
> for running Gup on a physical iPhone so that I can validate Metal rendering
> correctness and measure real-world performance."

## Acceptance Criteria

- [ ] A `docs/guides/ios-device-testing.md` documents: Xcode setup, code
      signing, provisioning profiles, deployment to device, and Metal GPU
      capture
- [ ] Performance baseline recorded: FPS, frame time, GPU memory for 10k-point
      scatter plot on iPhone 12 (or equivalent A14 device)
- [ ] Known Simulator-vs-device differences documented (texture formats, feature
      sets, performance characteristics)
- [ ] CI can optionally target a self-hosted macOS runner with a connected
      device (gated workflow)

## Dependencies

### Prerequisite Stories

- GUP-270: iOS Platform Support ✅
- GUP-272: iOS Chart Rendering Integration 📋

## Testing Strategy

- Manual testing on physical device
- Screenshot comparison with Simulator output
- Frame timing measurements via Metal System Trace

## Definition of Done

- [ ] Testing guide written and reviewed
- [ ] Performance baseline documented
- [ ] Simulator-vs-device caveats documented
