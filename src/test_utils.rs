// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Test utilities for GPU resource management
//!
//! This module provides utilities for safely managing GPU resources in tests,
//! preventing resource conflicts when tests run in parallel.

use crate::error::GupResult;
use crate::render::RenderContext;
use std::sync::Arc;
use tokio::sync::{Semaphore, SemaphorePermit};

/// Global semaphore to limit concurrent GPU context creation
///
/// This prevents resource conflicts by ensuring only ONE GPU context
/// can be active at a time. This is conservative but ensures reliability.
static GPU_CONTEXT_SEMAPHORE: Semaphore = Semaphore::const_new(1);

/// RAII guard for GPU context access
///
/// Automatically releases the semaphore permit when dropped, ensuring
/// proper resource cleanup even if tests panic.
pub struct GpuContextGuard<'a> {
    context: Arc<RenderContext>,
    #[allow(dead_code)]
    permit: SemaphorePermit<'a>,
}

impl<'a> GpuContextGuard<'a> {
    /// Get a reference to the underlying context
    pub fn context(&self) -> &Arc<RenderContext> {
        &self.context
    }

    /// Clone the underlying context Arc
    pub fn clone_context(&self) -> Arc<RenderContext> {
        Arc::clone(&self.context)
    }
}

impl<'a> std::ops::Deref for GpuContextGuard<'a> {
    type Target = Arc<RenderContext>;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

/// Create a test GPU context with resource management
///
/// This function acquires a semaphore permit before creating a GPU context,
/// ensuring that only a limited number of contexts are created concurrently.
/// This prevents segmentation faults from GPU driver resource conflicts.
///
/// # Example
///
/// ```no_run
/// use gup::test_utils::create_test_context;
///
/// #[tokio::test]
/// async fn my_gpu_test() {
///     let context = create_test_context().await.unwrap();
///     // Use context for testing
/// }
/// ```
pub async fn create_test_context() -> GupResult<GpuContextGuard<'static>> {
    // Acquire semaphore permit (blocks if too many contexts are being created)
    let permit = GPU_CONTEXT_SEMAPHORE.acquire().await.unwrap();

    // Create the GPU context
    let context = Arc::new(RenderContext::new().await?);

    Ok(GpuContextGuard { context, permit })
}

/// Create a test GPU context that can be cloned
///
/// Convenience function that returns an Arc<RenderContext> for tests that need
/// to share the context across multiple selections or systems.
///
/// # Example
///
/// ```no_run
/// use gup::test_utils::create_shared_test_context;
///
/// #[tokio::test]
/// async fn my_shared_context_test() {
///     let (context, _guard) = create_shared_test_context().await.unwrap();
///     // Use context, guard ensures cleanup
/// }
/// ```
pub async fn create_shared_test_context(
) -> GupResult<(Arc<RenderContext>, GpuContextGuard<'static>)> {
    let guard = create_test_context().await?;
    let context = guard.clone_context();
    Ok((context, guard))
}

/// Create a mutable test GPU context for tests that need mutation
///
/// This function is for tests that need to mutate the context directly.
/// Returns a tuple of (context, permit) where the permit ensures the semaphore
/// is released when dropped.
///
/// # Example
///
/// ```no_run
/// use gup::test_utils::create_mut_test_context;
///
/// #[tokio::test]
/// async fn my_mut_context_test() {
///     let (mut context, _permit) = create_mut_test_context().await.unwrap();
///     // Use mutable context
/// }
/// ```
pub async fn create_mut_test_context() -> GupResult<(RenderContext, SemaphorePermit<'static>)> {
    // Acquire semaphore permit (blocks if too many contexts are being created)
    let permit = GPU_CONTEXT_SEMAPHORE.acquire().await.unwrap();

    // Create the GPU context
    let context = RenderContext::new().await?;

    Ok((context, permit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_test_context() {
        let context = create_test_context().await;
        assert!(context.is_ok());
    }

    #[tokio::test]
    async fn test_create_shared_test_context() {
        let result = create_shared_test_context().await;
        assert!(result.is_ok());

        let (context, _guard) = result.unwrap();
        // Verify we can use the context
        assert!(context.device().features().contains(wgpu::Features::default()));
    }

    #[tokio::test]
    async fn test_multiple_contexts_sequential() {
        // Create multiple contexts sequentially
        for _ in 0..3 {
            let context = create_test_context().await;
            assert!(context.is_ok());
        }
    }

    #[tokio::test]
    async fn test_context_guard_deref() {
        let guard = create_test_context().await.unwrap();
        // Test that we can use the guard like an Arc<RenderContext>
        let _device = guard.device();
    }
}
