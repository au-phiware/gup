// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for context error recovery functionality.

use gup::context::{ContextState, GupContext, GupOptions};
use std::sync::{Arc, Mutex};
use wgpu::{Features, Limits};

#[tokio::test]
async fn test_context_initial_state() {
    let context = GupContext::new().await.expect("Failed to create context");
    
    assert_eq!(context.state(), ContextState::Active);
    assert!(context.check_device_status());
}

#[tokio::test]
async fn test_graceful_degradation_options() {
    let options = GupOptions::default();
    
    // Default options should allow fallback
    assert!(options.allow_software_fallback);
    assert!(options.reduced_features.is_some());
    assert!(options.reduced_limits.is_some());
}

#[tokio::test]
async fn test_context_with_reduced_features() {
    let mut options = GupOptions::default();
    options.reduced_features = Some(Features::empty());
    options.reduced_limits = Some(Limits::downlevel_defaults());
    
    let context = GupContext::with_options(options).await.expect("Failed to create context");
    assert_eq!(context.state(), ContextState::Active);
}

#[tokio::test]
async fn test_recovery_with_fallback_options() {
    let mut options = GupOptions::default();
    options.allow_software_fallback = true;
    options.reduced_features = Some(Features::empty());
    options.reduced_limits = Some(Limits::downlevel_defaults());
    
    let mut context = GupContext::with_options(options).await.expect("Failed to create context");
    let context_mut = Arc::get_mut(&mut context).expect("Failed to get mutable context");
    
    context_mut.mark_device_lost();
    let result = context_mut.attempt_recovery().await.expect("Failed to get recovery result");
    
    // Recovery should succeed even with reduced options
    assert!(result.success);
    assert_eq!(context_mut.state(), ContextState::Active);
}
#[tokio::test]
async fn test_recovery_callback() {
    let mut context = GupContext::new().await.expect("Failed to create context");
    
    let callback_states = Arc::new(Mutex::new(Vec::new()));
    let callback_states_clone = Arc::clone(&callback_states);
    
    let context_mut = Arc::get_mut(&mut context).expect("Failed to get mutable context");
    context_mut.set_recovery_callback(Box::new(move |state| {
        callback_states_clone.lock().unwrap().push(state);
    }));
    
    // Manually mark device as lost
    context_mut.mark_device_lost();
    
    let states = callback_states.lock().unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0], ContextState::DeviceLost);
}

#[tokio::test]
async fn test_device_loss_detection() {
    let mut context = GupContext::new().await.expect("Failed to create context");
    let context_mut = Arc::get_mut(&mut context).expect("Failed to get mutable context");
    
    // Initially active
    assert_eq!(context_mut.state(), ContextState::Active);
    
    // Mark as lost
    context_mut.mark_device_lost();
    assert_eq!(context_mut.state(), ContextState::DeviceLost);
    assert!(!context_mut.check_device_status());
}

#[tokio::test]
async fn test_recovery_attempt() {
    let mut context = GupContext::new().await.expect("Failed to create context");
    let context_mut = Arc::get_mut(&mut context).expect("Failed to get mutable context");
    
    // Mark device as lost
    context_mut.mark_device_lost();
    assert_eq!(context_mut.state(), ContextState::DeviceLost);
    
    // Attempt recovery
    let result = context_mut.attempt_recovery().await.expect("Failed to get recovery result");
    
    // Recovery should succeed in our test environment
    assert!(result.success);
    assert!(result.error.is_none());
    
    // State should be back to Active
    assert_eq!(context_mut.state(), ContextState::Active);
    
    // Should have a recovery attempt recorded
    let last_attempt = context_mut.last_recovery_attempt().expect("No recovery attempt recorded");
    assert!(last_attempt.success);
}

#[tokio::test]
async fn test_recovery_timing() {
    let mut context = GupContext::new().await.expect("Failed to create context");
    let context_mut = Arc::get_mut(&mut context).expect("Failed to get mutable context");
    
    context_mut.mark_device_lost();
    
    let result = context_mut.attempt_recovery().await.expect("Failed to get recovery result");
    
    // Recovery should complete in reasonable time (< 2 seconds as per AC)
    assert!(result.duration.as_secs() < 2, "Recovery took too long: {:?}", result.duration);
}

#[tokio::test]
async fn test_multiple_recovery_attempts() {
    let mut context = GupContext::new().await.expect("Failed to create context");
    let context_mut = Arc::get_mut(&mut context).expect("Failed to get mutable context");
    
    // First recovery
    context_mut.mark_device_lost();
    let result1 = context_mut.attempt_recovery().await.expect("Failed first recovery");
    assert!(result1.success);
    
    // Second recovery
    context_mut.mark_device_lost();
    let result2 = context_mut.attempt_recovery().await.expect("Failed second recovery");
    assert!(result2.success);
    
    // Last attempt should be the second one
    let last_attempt = context_mut.last_recovery_attempt().expect("No recovery attempt recorded");
    assert_eq!(last_attempt.duration, result2.duration);
}

#[tokio::test]
async fn test_state_transitions() {
    let mut context = GupContext::new().await.expect("Failed to create context");
    let context_mut = Arc::get_mut(&mut context).expect("Failed to get mutable context");
    
    let states = Arc::new(Mutex::new(Vec::new()));
    let states_clone = Arc::clone(&states);
    
    context_mut.set_recovery_callback(Box::new(move |state| {
        states_clone.lock().unwrap().push(state);
    }));
    
    // Trigger recovery process
    context_mut.mark_device_lost();
    let _result = context_mut.attempt_recovery().await;
    
    // Should have seen: DeviceLost -> Recovering -> Active
    let recorded_states = states.lock().unwrap();
    assert!(recorded_states.contains(&ContextState::DeviceLost));
    assert!(recorded_states.contains(&ContextState::Recovering));
    assert!(recorded_states.contains(&ContextState::Active));
}
