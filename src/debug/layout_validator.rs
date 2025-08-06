// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Memory layout validation utilities for Rust ↔ WGSL compatibility checking.
//!
//! This module provides tools to validate that Rust struct layouts match WGSL struct layouts,
//! preventing common GPU programming errors related to memory alignment and field ordering.

use crate::error::GupResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Memory layout validator for Rust ↔ WGSL compatibility
#[derive(Debug)]
pub struct MemoryLayoutValidator {
    /// Cache of validated layouts to avoid redundant checks
    validation_cache: HashMap<String, LayoutValidationResult>,
    /// History of all validation results for reporting
    validation_history: Vec<LayoutValidationResult>,
}

impl MemoryLayoutValidator {
    /// Create a new memory layout validator
    pub fn new() -> Self {
        Self {
            validation_cache: HashMap::new(),
            validation_history: Vec::new(),
        }
    }

    /// Validate that a Rust struct matches WGSL memory layout requirements
    pub fn validate_layout<T>(&mut self, struct_name: &str) -> GupResult<LayoutValidationResult>
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        // Check cache first
        if let Some(cached_result) = self.validation_cache.get(struct_name) {
            return Ok(cached_result.clone());
        }

        let mut result = LayoutValidationResult {
            struct_name: struct_name.to_string(),
            is_valid: true,
            warnings: Vec::new(),
            errors: Vec::new(),
            rust_size: std::mem::size_of::<T>(),
            rust_alignment: std::mem::align_of::<T>(),
            expected_wgsl_size: None,
            expected_wgsl_alignment: None,
            field_offsets: Vec::new(),
            recommendations: Vec::new(),
        };

        // Basic validation checks
        self.validate_size_alignment::<T>(&mut result);
        self.validate_bytemuck_traits::<T>(&mut result);
        self.check_common_issues::<T>(&mut result);

        // Cache and store result
        self.validation_cache
            .insert(struct_name.to_string(), result.clone());
        self.validation_history.push(result.clone());

        Ok(result)
    }

    /// Validate multiple structs and return a summary report
    pub fn validate_multiple(
        &mut self,
        structs: Vec<(&str, fn(&mut LayoutValidationResult))>,
    ) -> ValidationSummary {
        let mut results = Vec::new();
        let mut total_errors = 0;
        let mut total_warnings = 0;

        for (name, validator_fn) in structs {
            let mut result = LayoutValidationResult {
                struct_name: name.to_string(),
                is_valid: true,
                warnings: Vec::new(),
                errors: Vec::new(),
                rust_size: 0,
                rust_alignment: 0,
                expected_wgsl_size: None,
                expected_wgsl_alignment: None,
                field_offsets: Vec::new(),
                recommendations: Vec::new(),
            };

            validator_fn(&mut result);

            total_errors += result.errors.len();
            total_warnings += result.warnings.len();
            result.is_valid = result.errors.is_empty();

            results.push(result);
        }

        ValidationSummary {
            total_structs: results.len(),
            valid_structs: results.iter().filter(|r| r.is_valid).count(),
            total_errors,
            total_warnings,
            results,
        }
    }

    /// Get validation history for reporting
    pub fn get_validation_history(&self) -> Vec<LayoutValidationResult> {
        self.validation_history.clone()
    }

    /// Clear validation cache and history
    pub fn clear_cache(&mut self) {
        self.validation_cache.clear();
        self.validation_history.clear();
    }

    /// Validate size and alignment requirements
    fn validate_size_alignment<T>(&self, result: &mut LayoutValidationResult)
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        let size = std::mem::size_of::<T>();
        let alignment = std::mem::align_of::<T>();

        // WGSL alignment rules
        if size % 16 != 0 && size > 16 {
            result.warnings.push(format!(
                "Struct size {size} is not 16-byte aligned. WGSL prefers 16-byte alignment for larger structs."
            ));
            result
                .recommendations
                .push("Consider adding padding fields to align to 16-byte boundary".to_string());
        }

        if alignment < 4 {
            result.warnings.push(format!(
                "Struct alignment {alignment} is less than 4 bytes. WGSL minimum alignment is typically 4 bytes."
            ));
        }

        // Check for power-of-2 alignment
        if !alignment.is_power_of_two() {
            result.errors.push(format!(
                "Struct alignment {alignment} is not a power of 2, which is required for GPU buffers."
            ));
        }
    }

    /// Validate that bytemuck traits are properly implemented
    fn validate_bytemuck_traits<T>(&self, result: &mut LayoutValidationResult)
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        // If we can call this function, the traits are implemented
        // This is a compile-time check, so we just add a success note
        result.recommendations.push(
            "✓ bytemuck::Pod and bytemuck::Zeroable traits are properly implemented".to_string(),
        );

        // Check for zero-initialization safety
        let zero_instance: T = bytemuck::Zeroable::zeroed();
        let _ = zero_instance; // Consume to avoid unused variable warning

        result
            .recommendations
            .push("✓ Struct can be safely zero-initialized".to_string());
    }

    /// Check for common GPU programming issues
    fn check_common_issues<T>(&self, result: &mut LayoutValidationResult) {
        let size = std::mem::size_of::<T>();

        // Check for very large structs that might cause GPU memory issues
        if size > 1024 {
            result.warnings.push(format!(
                "Struct size {size} bytes is quite large. Consider breaking into smaller structs or using references."
            ));
        }

        // Check for empty structs
        if size == 0 {
            result
                .errors
                .push("Zero-sized structs are not valid for GPU buffers".to_string());
        }

        // Check for common problematic sizes
        if size == 1 || size == 2 || size == 3 {
            result.warnings.push(format!(
                "Struct size {size} bytes may cause alignment issues. Consider padding to 4 bytes minimum."
            ));
        }
    }
}

impl Default for MemoryLayoutValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of memory layout validation for a single struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutValidationResult {
    pub struct_name: String,
    pub is_valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub rust_size: usize,
    pub rust_alignment: usize,
    pub expected_wgsl_size: Option<usize>,
    pub expected_wgsl_alignment: Option<usize>,
    pub field_offsets: Vec<FieldOffset>,
    pub recommendations: Vec<String>,
}

/// Information about a struct field's memory offset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldOffset {
    pub field_name: String,
    pub offset: usize,
    pub size: usize,
    pub alignment: usize,
}

/// Summary of validation results for multiple structs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_structs: usize,
    pub valid_structs: usize,
    pub total_errors: usize,
    pub total_warnings: usize,
    pub results: Vec<LayoutValidationResult>,
}

impl ValidationSummary {
    /// Check if all validations passed without errors
    pub fn is_all_valid(&self) -> bool {
        self.total_errors == 0
    }

    /// Get validation success rate as percentage
    pub fn success_rate(&self) -> f32 {
        if self.total_structs == 0 {
            100.0
        } else {
            (self.valid_structs as f32 / self.total_structs as f32) * 100.0
        }
    }
}

/// Macro to help with common struct validation patterns
#[macro_export]
macro_rules! validate_gpu_struct {
    ($validator:expr, $struct_type:ty, $name:literal) => {
        $validator.validate_layout::<$struct_type>($name)
    };
}

/// Helper function to validate common GPU struct types used in the project
pub fn validate_common_gpu_structs(
    validator: &mut MemoryLayoutValidator,
) -> GupResult<ValidationSummary> {
    let structs = vec![
        (
            "ElementData",
            validate_element_data as fn(&mut LayoutValidationResult),
        ),
        (
            "GpuInteractionQuery",
            validate_gpu_interaction_query as fn(&mut LayoutValidationResult),
        ),
        (
            "InteractionResult",
            validate_interaction_result as fn(&mut LayoutValidationResult),
        ),
    ];

    Ok(validator.validate_multiple(structs))
}

/// Validation function for ElementData
fn validate_element_data(result: &mut LayoutValidationResult) {
    use crate::interaction::ElementData;
    use std::mem::{align_of, offset_of, size_of};

    result.rust_size = size_of::<ElementData>();
    result.rust_alignment = align_of::<ElementData>();

    // Add field offset information
    result.field_offsets = vec![
        FieldOffset {
            field_name: "position".to_string(),
            offset: offset_of!(ElementData, position),
            size: 8, // [f32; 2]
            alignment: 4,
        },
        FieldOffset {
            field_name: "size".to_string(),
            offset: offset_of!(ElementData, size),
            size: 8, // [f32; 2]
            alignment: 4,
        },
        FieldOffset {
            field_name: "mark_type".to_string(),
            offset: offset_of!(ElementData, mark_type),
            size: 4, // u32
            alignment: 4,
        },
        FieldOffset {
            field_name: "element_id".to_string(),
            offset: offset_of!(ElementData, element_id),
            size: 4, // u32
            alignment: 4,
        },
        FieldOffset {
            field_name: "selection_id".to_string(),
            offset: offset_of!(ElementData, selection_id),
            size: 4, // u32
            alignment: 4,
        },
        FieldOffset {
            field_name: "_padding".to_string(),
            offset: offset_of!(ElementData, _padding),
            size: 4, // u32
            alignment: 4,
        },
    ];

    // Validate alignment requirements for WGSL compatibility
    if result.rust_size % 16 == 0 {
        result
            .recommendations
            .push("✓ Struct is 16-byte aligned for optimal GPU performance".to_string());
    } else {
        result.warnings.push(format!(
            "Struct size {} is not 16-byte aligned. Consider adding padding.",
            result.rust_size
        ));
    }
}

/// Validation function for GpuInteractionQuery
fn validate_gpu_interaction_query(result: &mut LayoutValidationResult) {
    use crate::interaction::GpuInteractionQuery;
    use std::mem::{align_of, offset_of, size_of};

    result.rust_size = size_of::<GpuInteractionQuery>();
    result.rust_alignment = align_of::<GpuInteractionQuery>();

    result.field_offsets = vec![
        FieldOffset {
            field_name: "query_type".to_string(),
            offset: offset_of!(GpuInteractionQuery, query_type),
            size: 4, // u32
            alignment: 4,
        },
        FieldOffset {
            field_name: "max_results".to_string(),
            offset: offset_of!(GpuInteractionQuery, max_results),
            size: 4, // u32
            alignment: 4,
        },
        FieldOffset {
            field_name: "position".to_string(),
            offset: offset_of!(GpuInteractionQuery, position),
            size: 8, // [f32; 2]
            alignment: 4,
        },
        FieldOffset {
            field_name: "region_size".to_string(),
            offset: offset_of!(GpuInteractionQuery, region_size),
            size: 8, // [f32; 2]
            alignment: 4,
        },
        FieldOffset {
            field_name: "_padding".to_string(),
            offset: offset_of!(GpuInteractionQuery, _padding),
            size: 8, // [u32; 2]
            alignment: 4,
        },
    ];

    // Check that position field is at 8-byte boundary (critical for GUP-013 fix)
    let position_offset = offset_of!(GpuInteractionQuery, position);
    if position_offset == 8 {
        result
            .recommendations
            .push("✓ Position field correctly aligned at 8-byte boundary".to_string());
    } else {
        result.errors.push(format!(
            "Position field at offset {position_offset} should be at offset 8 for WGSL compatibility"
        ));
    }
}

/// Validation function for InteractionResult
fn validate_interaction_result(result: &mut LayoutValidationResult) {
    use crate::interaction::InteractionResult;
    use std::mem::{align_of, offset_of, size_of};

    result.rust_size = size_of::<InteractionResult>();
    result.rust_alignment = align_of::<InteractionResult>();

    result.field_offsets = vec![
        FieldOffset {
            field_name: "element_id".to_string(),
            offset: offset_of!(InteractionResult, element_id),
            size: 4, // u32
            alignment: 4,
        },
        FieldOffset {
            field_name: "selection_id".to_string(),
            offset: offset_of!(InteractionResult, selection_id),
            size: 4, // u32
            alignment: 4,
        },
        FieldOffset {
            field_name: "distance".to_string(),
            offset: offset_of!(InteractionResult, distance),
            size: 4, // f32
            alignment: 4,
        },
        FieldOffset {
            field_name: "is_hit".to_string(),
            offset: offset_of!(InteractionResult, is_hit),
            size: 4, // u32
            alignment: 4,
        },
        FieldOffset {
            field_name: "intersection_point".to_string(),
            offset: offset_of!(InteractionResult, intersection_point),
            size: 8, // [f32; 2]
            alignment: 4,
        },
        FieldOffset {
            field_name: "_padding".to_string(),
            offset: offset_of!(InteractionResult, _padding),
            size: 8, // [u32; 2]
            alignment: 4,
        },
    ];

    // Check that intersection_point field is at 16-byte boundary (critical for GUP-013 fix)
    let intersection_offset = offset_of!(InteractionResult, intersection_point);
    if intersection_offset == 16 {
        result
            .recommendations
            .push("✓ Intersection point field correctly aligned at 16-byte boundary".to_string());
    } else {
        result.errors.push(format!(
            "Intersection point field at offset {intersection_offset} should be at offset 16 for WGSL compatibility"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_validator_creation() {
        let validator = MemoryLayoutValidator::new();
        assert_eq!(validator.validation_cache.len(), 0);
        assert_eq!(validator.validation_history.len(), 0);
    }

    #[test]
    fn test_field_offset() {
        let offset = FieldOffset {
            field_name: "test_field".to_string(),
            offset: 8,
            size: 4,
            alignment: 4,
        };

        assert_eq!(offset.field_name, "test_field");
        assert_eq!(offset.offset, 8);
        assert_eq!(offset.size, 4);
        assert_eq!(offset.alignment, 4);
    }

    #[test]
    fn test_validation_summary() {
        let summary = ValidationSummary {
            total_structs: 10,
            valid_structs: 8,
            total_errors: 2,
            total_warnings: 5,
            results: Vec::new(),
        };

        assert!(!summary.is_all_valid()); // Has errors
        assert_eq!(summary.success_rate(), 80.0); // 8/10 = 80%
    }

    #[test]
    fn test_validation_summary_perfect() {
        let summary = ValidationSummary {
            total_structs: 5,
            valid_structs: 5,
            total_errors: 0,
            total_warnings: 0,
            results: Vec::new(),
        };

        assert!(summary.is_all_valid());
        assert_eq!(summary.success_rate(), 100.0);
    }

    #[test]
    fn test_validation_summary_empty() {
        let summary = ValidationSummary {
            total_structs: 0,
            valid_structs: 0,
            total_errors: 0,
            total_warnings: 0,
            results: Vec::new(),
        };

        assert!(summary.is_all_valid());
        assert_eq!(summary.success_rate(), 100.0); // Empty case returns 100%
    }

    #[test]
    fn test_layout_validation_result() {
        let result = LayoutValidationResult {
            struct_name: "TestStruct".to_string(),
            is_valid: true,
            warnings: vec!["Warning 1".to_string()],
            errors: Vec::new(),
            rust_size: 32,
            rust_alignment: 8,
            expected_wgsl_size: Some(32),
            expected_wgsl_alignment: Some(8),
            field_offsets: Vec::new(),
            recommendations: vec!["Recommendation 1".to_string()],
        };

        assert_eq!(result.struct_name, "TestStruct");
        assert!(result.is_valid);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.errors.len(), 0);
        assert_eq!(result.rust_size, 32);
        assert_eq!(result.rust_alignment, 8);
    }
}
