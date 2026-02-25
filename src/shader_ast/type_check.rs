// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Type checking system for WGSL AST-based shader composition.
//!
//! Validates that composed shader functions have compatible input/output types
//! and provides clear error messages with suggestions for fixes.

use super::types::*;
use std::fmt;

/// Error produced during type checking.
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub expected: WgslType,
    pub actual: WgslType,
    pub suggestion: Option<String>,
    pub context: String,
}

impl TypeError {
    pub fn new(
        message: impl Into<String>,
        expected: WgslType,
        actual: WgslType,
        context: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            expected,
            actual,
            suggestion: None,
            context: context.into(),
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Type error in {}: {} (expected {}, found {})",
            self.context, self.message, self.expected, self.actual
        )?;
        if let Some(ref sug) = self.suggestion {
            write!(f, "\n  Suggestion: {sug}")?;
        }
        Ok(())
    }
}

impl std::error::Error for TypeError {}

/// Error produced during composition validation.
#[derive(Debug, Clone)]
pub struct CompositionError {
    pub message: String,
    pub errors: Vec<TypeError>,
    pub suggestion: Option<String>,
}

impl CompositionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            errors: Vec::new(),
            suggestion: None,
        }
    }

    pub fn with_type_error(mut self, error: TypeError) -> Self {
        self.errors.push(error);
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl fmt::Display for CompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Composition error: {}", self.message)?;
        for err in &self.errors {
            write!(f, "\n  - {err}")?;
        }
        if let Some(ref sug) = self.suggestion {
            write!(f, "\n  Suggestion: {sug}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CompositionError {}

/// Metadata about a shader function for type checking.
#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub name: String,
    /// The primary input type (first parameter).
    pub input_type: WgslType,
    /// The output / return type.
    pub output_type: WgslType,
    /// Additional parameters (e.g., uniform struct).
    pub extra_params: Vec<(String, WgslType)>,
}

impl FunctionSignature {
    /// Create from an AST function definition.
    pub fn from_function(func: &Function) -> Option<Self> {
        if func.parameters.is_empty() {
            return None;
        }
        let input_type = func.parameters[0].ty.clone();
        let output_type = func.return_type.clone().unwrap_or(WgslType::Void);
        let extra_params = func.parameters[1..]
            .iter()
            .map(|p| (p.name.clone(), p.ty.clone()))
            .collect();

        Some(Self {
            name: func.name.clone(),
            input_type,
            output_type,
            extra_params,
        })
    }
}

/// Type checker for shader function composition.
pub struct TypeChecker;

impl TypeChecker {
    /// Check if two types are directly compatible (same type).
    pub fn is_directly_compatible(output: &WgslType, input: &WgslType) -> bool {
        output == input
    }

    /// Check if `output` can be automatically promoted to `input`.
    pub fn is_promotable(output: &WgslType, input: &WgslType) -> bool {
        output.can_promote_to(input)
    }

    /// Check compatibility between an output type and an input type.
    ///
    /// Returns `Ok(())` if types are compatible (directly or via promotion),
    /// or `Err(TypeError)` with a helpful message if not.
    pub fn check_compatibility(
        output: &WgslType,
        input: &WgslType,
        context: &str,
    ) -> Result<(), TypeError> {
        if Self::is_directly_compatible(output, input) {
            return Ok(());
        }

        if Self::is_promotable(output, input) {
            return Ok(());
        }

        let mut err = TypeError::new(
            format!("output type '{output}' is not compatible with input type '{input}'"),
            input.clone(),
            output.clone(),
            context,
        );

        // Suggest a promotion if possible in the other direction
        if input.can_promote_to(output) {
            err = err.with_suggestion(format!(
                "consider swapping the function order, since '{input}' can promote to '{output}'"
            ));
        } else {
            err = err.with_suggestion(Self::suggest_conversion(output, input));
        }

        Err(err)
    }

    /// Suggest a conversion between two incompatible types.
    pub fn suggest_conversion(from: &WgslType, to: &WgslType) -> String {
        match (from, to) {
            // Scalar → Scalar
            (WgslType::Scalar(a), WgslType::Scalar(b)) => {
                format!("use {b}(value) to convert from {a} to {b}")
            }
            // Vector → Scalar (e.g., extract component)
            (WgslType::Vector(_, _), WgslType::Scalar(_)) => {
                format!("extract a component: value.x, value.y, etc. to get {to}")
            }
            // Scalar → Vector
            (WgslType::Scalar(_), WgslType::Vector(_, dim)) => {
                format!(
                    "use {to}(value{}) to construct a {to}",
                    ", 0.0".repeat((*dim as usize) - 1)
                )
            }
            // Larger vector → Smaller vector
            (WgslType::Vector(_, from_dim), WgslType::Vector(_, to_dim)) if from_dim > to_dim => {
                let swizzle: String = "xyzw".chars().take(*to_dim as usize).collect();
                format!("use value.{swizzle} to truncate from {from} to {to}")
            }
            _ => format!(
                "no automatic conversion from {from} to {to}; consider adding an adapter function"
            ),
        }
    }

    /// Validate a chain of function signatures for type compatibility.
    ///
    /// Checks that each function's output is compatible with the next function's input.
    pub fn validate_function_chain(
        functions: &[FunctionSignature],
    ) -> Result<(), CompositionError> {
        if functions.len() < 2 {
            return Ok(());
        }

        let mut errors = Vec::new();

        for pair in functions.windows(2) {
            let output = &pair[0].output_type;
            let input = &pair[1].input_type;
            let context = format!("{} → {}", pair[0].name, pair[1].name);

            if let Err(err) = Self::check_compatibility(output, input, &context) {
                errors.push(err);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            let mut comp_err = CompositionError::new(format!(
                "function chain has {} type incompatibilit{}",
                errors.len(),
                if errors.len() == 1 { "y" } else { "ies" }
            ));
            for err in errors {
                comp_err = comp_err.with_type_error(err);
            }
            comp_err = comp_err.with_suggestion(
                "ensure each function's output type matches the next function's input type"
                    .to_string(),
            );
            Err(comp_err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_compatibility() {
        let f32_ty = WgslType::Scalar(ScalarType::F32);
        assert!(TypeChecker::is_directly_compatible(&f32_ty, &f32_ty));

        let vec3_ty = WgslType::Vector(ScalarType::F32, 3);
        assert!(TypeChecker::is_directly_compatible(&vec3_ty, &vec3_ty));

        assert!(!TypeChecker::is_directly_compatible(&f32_ty, &vec3_ty));
    }

    #[test]
    fn test_promotion_compatibility() {
        let f32_ty = WgslType::Scalar(ScalarType::F32);
        let vec2_ty = WgslType::Vector(ScalarType::F32, 2);
        let vec4_ty = WgslType::Vector(ScalarType::F32, 4);

        assert!(TypeChecker::is_promotable(&f32_ty, &vec2_ty));
        assert!(TypeChecker::is_promotable(&vec2_ty, &vec4_ty));
        assert!(!TypeChecker::is_promotable(&vec4_ty, &vec2_ty));
    }

    #[test]
    fn test_check_compatibility_ok() {
        let f32_ty = WgslType::Scalar(ScalarType::F32);
        let vec3_ty = WgslType::Vector(ScalarType::F32, 3);

        // Same type
        assert!(TypeChecker::check_compatibility(&f32_ty, &f32_ty, "test").is_ok());

        // Promotable
        assert!(TypeChecker::check_compatibility(&f32_ty, &vec3_ty, "test").is_ok());
    }

    #[test]
    fn test_check_compatibility_error() {
        let vec4_ty = WgslType::Vector(ScalarType::F32, 4);
        let f32_ty = WgslType::Scalar(ScalarType::F32);

        let result = TypeChecker::check_compatibility(&vec4_ty, &f32_ty, "scale → color");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.expected, f32_ty);
        assert_eq!(err.actual, vec4_ty);
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn test_type_error_display() {
        let err = TypeError::new(
            "mismatch",
            WgslType::Scalar(ScalarType::F32),
            WgslType::Vector(ScalarType::F32, 3),
            "linear_scale → color_map",
        )
        .with_suggestion("extract a component with .x");

        let display = format!("{err}");
        assert!(display.contains("linear_scale → color_map"));
        assert!(display.contains("mismatch"));
        assert!(display.contains("Suggestion"));
    }

    #[test]
    fn test_validate_chain_compatible() {
        let chain = vec![
            FunctionSignature {
                name: "normalize".to_string(),
                input_type: WgslType::Scalar(ScalarType::F32),
                output_type: WgslType::Scalar(ScalarType::F32),
                extra_params: vec![],
            },
            FunctionSignature {
                name: "color_map".to_string(),
                input_type: WgslType::Scalar(ScalarType::F32),
                output_type: WgslType::Vector(ScalarType::F32, 4),
                extra_params: vec![],
            },
        ];

        assert!(TypeChecker::validate_function_chain(&chain).is_ok());
    }

    #[test]
    fn test_validate_chain_with_promotion() {
        let chain = vec![
            FunctionSignature {
                name: "scale".to_string(),
                input_type: WgslType::Scalar(ScalarType::F32),
                output_type: WgslType::Scalar(ScalarType::F32),
                extra_params: vec![],
            },
            FunctionSignature {
                name: "transform".to_string(),
                input_type: WgslType::Vector(ScalarType::F32, 3),
                output_type: WgslType::Vector(ScalarType::F32, 4),
                extra_params: vec![],
            },
        ];

        // f32 can promote to vec3<f32>
        assert!(TypeChecker::validate_function_chain(&chain).is_ok());
    }

    #[test]
    fn test_validate_chain_incompatible() {
        let chain = vec![
            FunctionSignature {
                name: "color_map".to_string(),
                input_type: WgslType::Scalar(ScalarType::F32),
                output_type: WgslType::Vector(ScalarType::F32, 4),
                extra_params: vec![],
            },
            FunctionSignature {
                name: "normalize".to_string(),
                input_type: WgslType::Scalar(ScalarType::F32),
                output_type: WgslType::Scalar(ScalarType::F32),
                extra_params: vec![],
            },
        ];

        let result = TypeChecker::validate_function_chain(&chain);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.errors.len(), 1);
        assert!(err.errors[0].context.contains("color_map → normalize"));
    }

    #[test]
    fn test_validate_single_function() {
        let chain = vec![FunctionSignature {
            name: "scale".to_string(),
            input_type: WgslType::Scalar(ScalarType::F32),
            output_type: WgslType::Scalar(ScalarType::F32),
            extra_params: vec![],
        }];

        assert!(TypeChecker::validate_function_chain(&chain).is_ok());
    }

    #[test]
    fn test_suggest_conversion_scalar_to_vector() {
        let suggestion = TypeChecker::suggest_conversion(
            &WgslType::Scalar(ScalarType::F32),
            &WgslType::Vector(ScalarType::F32, 3),
        );
        assert!(suggestion.contains("vec3<f32>"));
    }

    #[test]
    fn test_suggest_conversion_vector_to_scalar() {
        let suggestion = TypeChecker::suggest_conversion(
            &WgslType::Vector(ScalarType::F32, 3),
            &WgslType::Scalar(ScalarType::F32),
        );
        assert!(suggestion.contains("value.x"));
    }

    #[test]
    fn test_suggest_conversion_larger_to_smaller_vector() {
        let suggestion = TypeChecker::suggest_conversion(
            &WgslType::Vector(ScalarType::F32, 4),
            &WgslType::Vector(ScalarType::F32, 2),
        );
        assert!(suggestion.contains("value.xy"));
    }

    #[test]
    fn test_function_signature_from_ast() {
        let func = Function {
            name: "linear_scale".to_string(),
            parameters: vec![
                Parameter {
                    name: "value".to_string(),
                    ty: WgslType::Scalar(ScalarType::F32),
                },
                Parameter {
                    name: "uniforms".to_string(),
                    ty: WgslType::Struct("LinearScaleUniforms".to_string()),
                },
            ],
            return_type: Some(WgslType::Scalar(ScalarType::F32)),
            body: Block::empty(),
            attributes: vec![],
        };

        let sig = FunctionSignature::from_function(&func).unwrap();
        assert_eq!(sig.name, "linear_scale");
        assert_eq!(sig.input_type, WgslType::Scalar(ScalarType::F32));
        assert_eq!(sig.output_type, WgslType::Scalar(ScalarType::F32));
        assert_eq!(sig.extra_params.len(), 1);
        assert_eq!(sig.extra_params[0].0, "uniforms");
    }
}
