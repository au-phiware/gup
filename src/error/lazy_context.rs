// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lazy error context creation for performance-critical paths.
//!
//! This module provides a lazy wrapper around ErrorContext that defers expensive
//! system information collection until the context is actually needed.

use std::sync::OnceLock;
use std::time::Instant;

use super::{ErrorContext, GupError};

/// Lazy error context that defers expensive context creation until needed.
///
/// This wrapper holds the error and creates the full ErrorContext only when
/// accessed. This is useful for hot paths where errors are created frequently
/// but context is rarely needed.
#[derive(Debug)]
pub struct LazyErrorContext {
    error: GupError,
    context: OnceLock<ErrorContext>,
    creation_time: Instant,
}

impl LazyErrorContext {
    /// Create a new lazy error context.
    ///
    /// This is a cheap operation that only stores the error and creation time.
    /// The expensive ErrorContext creation is deferred until `context()` is called.
    pub fn new(error: GupError) -> Self {
        Self {
            error,
            context: OnceLock::new(),
            creation_time: Instant::now(),
        }
    }

    /// Get the error without creating full context.
    pub fn error(&self) -> &GupError {
        &self.error
    }

    /// Get the error creation time.
    pub fn creation_time(&self) -> Instant {
        self.creation_time
    }

    /// Get or create the full error context.
    ///
    /// This method will create the full ErrorContext on first call and cache it
    /// for subsequent calls. Context creation involves expensive system information
    /// collection (GPU info, memory stats, performance metrics).
    pub fn context(&self) -> &ErrorContext {
        self.context
            .get_or_init(|| ErrorContext::new(self.error.clone()))
    }

    /// Check if the full context has been created.
    pub fn has_context(&self) -> bool {
        self.context.get().is_some()
    }

    /// Get the time since creation.
    pub fn age(&self) -> std::time::Duration {
        self.creation_time.elapsed()
    }

    /// Consume this lazy context and return the full ErrorContext.
    ///
    /// This will create the context if it hasn't been created yet.
    pub fn into_context(self) -> ErrorContext {
        match self.context.into_inner() {
            Some(ctx) => ctx,
            None => ErrorContext::new(self.error),
        }
    }
}

impl Clone for LazyErrorContext {
    fn clone(&self) -> Self {
        Self {
            error: self.error.clone(),
            context: match self.context.get() {
                Some(ctx) => OnceLock::from(ctx.clone()),
                None => OnceLock::new(),
            },
            creation_time: self.creation_time,
        }
    }
}

impl From<GupError> for LazyErrorContext {
    fn from(error: GupError) -> Self {
        Self::new(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lazy_creation() {
        let error = GupError::performance_target_missed(16.67, 20.0);
        let lazy = LazyErrorContext::new(error);

        // Context should not be created yet
        assert!(!lazy.has_context());

        // Access should create context
        let _context = lazy.context();
        assert!(lazy.has_context());

        // Second access should return cached context
        let _context2 = lazy.context();
        assert!(lazy.has_context());
    }

    #[test]
    fn test_error_access_without_context() {
        let error = GupError::gpu_memory_exhausted(2048, 1024);
        let lazy = LazyErrorContext::new(error.clone());

        // Should be able to access error without creating context
        assert_eq!(lazy.error().category(), error.category());
        assert!(!lazy.has_context());
    }

    #[test]
    fn test_into_context() {
        let error = GupError::shader_compilation_failed("vertex", "syntax error");
        let lazy = LazyErrorContext::new(error);

        let context = lazy.into_context();
        assert!(!context.recovery_suggestions.is_empty());
    }

    #[test]
    fn test_clone() {
        let error = GupError::gpu_memory_exhausted(2048, 1024);
        let lazy = LazyErrorContext::new(error);

        // Create context in original
        let _ctx = lazy.context();
        assert!(lazy.has_context());

        // Clone should preserve the context
        let cloned = lazy.clone();
        assert!(cloned.has_context());
    }

    #[test]
    fn test_age() {
        let error = GupError::performance_target_missed(16.67, 20.0);
        let lazy = LazyErrorContext::new(error);

        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(lazy.age().as_millis() >= 10);
    }
}
