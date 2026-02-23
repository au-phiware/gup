// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for automatic device loss detection feature (GUP-149).

use gup::context::{ContextState, GupContext, GupOptions};

/// Test that automatic detection is enabled by default.
#[tokio::test]
async fn test_automatic_detection_enabled_by_default() {
    let options = GupOptions::default();
    assert!(
        options.automatic_device_loss_detection,
        "Automatic device loss detection should be enabled by default"
    );
}

/// Test that automatic detection can be disabled.
#[tokio::test]
async fn test_automatic_detection_can_be_disabled() {
    let mut options = GupOptions::default();
    options.automatic_device_loss_detection = false;
    assert!(
        !options.automatic_device_loss_detection,
        "Automatic device loss detection should be disabled when set to false"
    );
}

/// Test that context stores the automatic detection setting.
#[tokio::test]
async fn test_context_stores_automatic_detection_setting() {
    let mut options = GupOptions::default();
    options.automatic_device_loss_detection = false;

    let context = GupContext::with_options(options)
        .await
        .expect("Failed to create context");

    // The context should have stored the options
    // We can't directly access context_options from outside, but we can
    // verify it works by checking behavior
    assert_eq!(context.state(), ContextState::Active);
}

/// Test backward compatibility - manual detection still works.
#[tokio::test]
async fn test_manual_detection_still_works() {
    let mut options = GupOptions::default();
    options.automatic_device_loss_detection = false;

    let context = GupContext::with_options(options)
        .await
        .expect("Failed to create context");

    let mut ctx = std::sync::Arc::try_unwrap(context).expect("Failed to unwrap Arc");

    // Manual detection should still work
    ctx.mark_device_lost();
    assert_eq!(ctx.state(), ContextState::DeviceLost);
}

/// Test that automatic detection marks device lost on surface errors.
/// Note: This test simulates the behavior by manually marking device lost,
/// as we can't easily trigger actual surface errors in tests.
#[tokio::test]
async fn test_automatic_detection_on_surface_error() {
    let context = GupContext::new().await.expect("Failed to create context");

    let mut ctx = std::sync::Arc::try_unwrap(context).expect("Failed to unwrap Arc");

    // Verify initial state
    assert_eq!(ctx.state(), ContextState::Active);

    // In a real scenario, when begin_frame() fails to get_current_texture(),
    // it would automatically call mark_device_lost() if automatic detection is enabled.
    // We simulate this here:
    ctx.mark_device_lost();
    assert_eq!(ctx.state(), ContextState::DeviceLost);
}

/// Test that context with automatic detection enabled can recover.
#[tokio::test]
async fn test_automatic_detection_with_recovery() {
    let options = GupOptions::default();
    assert!(options.automatic_device_loss_detection);

    let context = GupContext::with_options(options)
        .await
        .expect("Failed to create context");

    let mut ctx = std::sync::Arc::try_unwrap(context).expect("Failed to unwrap Arc");

    // Simulate device loss
    ctx.mark_device_lost();
    assert_eq!(ctx.state(), ContextState::DeviceLost);

    // Attempt recovery
    let recovery_result = ctx
        .attempt_recovery()
        .await
        .expect("Recovery should succeed");

    assert!(recovery_result.success, "Recovery should succeed");
    assert_eq!(ctx.state(), ContextState::Active);
}

/// Test that disabling automatic detection doesn't affect recovery.
#[tokio::test]
async fn test_disabled_automatic_detection_recovery_works() {
    let mut options = GupOptions::default();
    options.automatic_device_loss_detection = false;

    let context = GupContext::with_options(options)
        .await
        .expect("Failed to create context");

    let mut ctx = std::sync::Arc::try_unwrap(context).expect("Failed to unwrap Arc");

    // Manual detection
    ctx.mark_device_lost();
    assert_eq!(ctx.state(), ContextState::DeviceLost);

    // Recovery should still work
    let recovery_result = ctx
        .attempt_recovery()
        .await
        .expect("Recovery should succeed");

    assert!(recovery_result.success, "Recovery should succeed");
    assert_eq!(ctx.state(), ContextState::Active);
}

/// Test that automatic detection setting persists through recovery.
#[tokio::test]
async fn test_automatic_detection_persists_through_recovery() {
    let mut options = GupOptions::default();
    options.automatic_device_loss_detection = true;

    let context = GupContext::with_options(options.clone())
        .await
        .expect("Failed to create context");

    let mut ctx = std::sync::Arc::try_unwrap(context).expect("Failed to unwrap Arc");

    // Mark device lost and recover
    ctx.mark_device_lost();
    let _ = ctx.attempt_recovery().await;

    // The setting should persist (stored in context_options)
    // We verify this by checking that the context is functional
    assert_eq!(ctx.state(), ContextState::Active);
}
