// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Accessor function system for Observable Plot-style field mapping.
//!
//! This module provides type-safe field accessors that bridge between
//! high-level Observable Plot syntax and GPU shader functions.

use std::collections::HashMap;
use std::marker::PhantomData;

/// Observable Plot-style accessor function for field-based data mapping.
///
/// Enables syntax like `x("revenue")` and `y("profit")` while maintaining
/// type safety and GPU shader compatibility.
#[derive(Debug, Clone)]
pub struct FieldAccessor {
    field_name: String,
}

impl FieldAccessor {
    /// Create a new field accessor.
    pub fn new(field_name: &str) -> Self {
        Self {
            field_name: field_name.to_string(),
        }
    }

    /// Get the field name.
    pub fn field_name(&self) -> &str {
        &self.field_name
    }
}

/// Observable Plot-style accessor functions for common chart attributes.
///
/// Create an X-axis accessor for the specified field.
pub fn x(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

/// Create a Y-axis accessor for the specified field.
pub fn y(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

/// Create a color accessor for the specified field.
pub fn color(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

/// Create a size accessor for the specified field.
pub fn size(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

/// Create a fill color accessor for the specified field.
pub fn fill(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

/// Create a stroke color accessor for the specified field.
pub fn stroke(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

/// Create a stroke width accessor for the specified field.
pub fn stroke_width(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

/// Create an opacity accessor for the specified field.
pub fn opacity(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

/// Type-safe accessor function that extracts values from data structures.
///
/// This trait provides compile-time type checking for field access while
/// supporting both reflection-based field access and closure-based extraction.
pub trait Accessor<T> {
    /// The output type of this accessor
    type Output;

    /// Extract the value from a data item
    fn extract(&self, data: &T) -> Self::Output;

    /// Get a string representation for debugging
    fn debug_name(&self) -> String;
}

/// Closure-based accessor implementation.
#[derive(Debug)]
pub struct ClosureAccessor<T, Output, F>
where
    F: Fn(&T) -> Output + Send + Sync,
{
    function: F,
    name: String,
    _phantom: PhantomData<(T, Output)>,
}

impl<T, Output, F> ClosureAccessor<T, Output, F>
where
    F: Fn(&T) -> Output + Send + Sync,
{
    /// Create a new closure accessor.
    pub fn new(name: &str, function: F) -> Self {
        Self {
            function,
            name: name.to_string(),
            _phantom: PhantomData,
        }
    }
}

impl<T, Output, F> Accessor<T> for ClosureAccessor<T, Output, F>
where
    F: Fn(&T) -> Output + Send + Sync,
    Output: Send + Sync,
{
    type Output = Output;

    fn extract(&self, data: &T) -> Self::Output {
        (self.function)(data)
    }

    fn debug_name(&self) -> String {
        self.name.clone()
    }
}

/// Constant value accessor for fixed attribute values.
#[derive(Debug, Clone)]
pub struct ConstantAccessor<Output> {
    value: Output,
    name: String,
}

impl<Output> ConstantAccessor<Output>
where
    Output: Clone,
{
    /// Create a new constant accessor.
    pub fn new(name: &str, value: Output) -> Self {
        Self {
            value,
            name: name.to_string(),
        }
    }
}

impl<T, Output> Accessor<T> for ConstantAccessor<Output>
where
    Output: Clone + Send + Sync,
{
    type Output = Output;

    fn extract(&self, _data: &T) -> Self::Output {
        self.value.clone()
    }

    fn debug_name(&self) -> String {
        format!("constant({})", self.name)
    }
}

/// Accessor registry for managing field-based data extraction.
///
/// This registry maps field names to accessor functions, enabling
/// Observable Plot-style syntax while maintaining type safety.
pub struct AccessorRegistry<T> {
    field_accessors: HashMap<String, Box<dyn Accessor<T, Output = AccessorValue> + Send + Sync>>,
    _phantom: PhantomData<T>,
}

/// Union type for different accessor output values.
#[derive(Debug, Clone, PartialEq)]
pub enum AccessorValue {
    Float(f32),
    Color([f32; 4]),
    String(String),
    Position([f32; 2]),
    Bool(bool),
}

impl AccessorValue {
    /// Convert to f32, with reasonable defaults for other types.
    pub fn as_f32(&self) -> f32 {
        match self {
            AccessorValue::Float(f) => *f,
            AccessorValue::Color(c) => c[0], // Use red component
            AccessorValue::String(s) => s.len() as f32, // String length as numeric value
            AccessorValue::Position(p) => p[0], // Use X coordinate
            AccessorValue::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// Convert to color, with reasonable defaults for other types.
    pub fn as_color(&self) -> [f32; 4] {
        match self {
            AccessorValue::Float(f) => [*f, *f, *f, 1.0], // Grayscale
            AccessorValue::Color(c) => *c,
            AccessorValue::String(_) => [0.5, 0.5, 0.5, 1.0], // Default gray
            AccessorValue::Position(p) => [p[0], p[1], 0.0, 1.0],
            AccessorValue::Bool(b) => {
                if *b {
                    [1.0, 1.0, 1.0, 1.0]
                } else {
                    [0.0, 0.0, 0.0, 1.0]
                }
            }
        }
    }

    /// Convert to position, with reasonable defaults for other types.
    pub fn as_position(&self) -> [f32; 2] {
        match self {
            AccessorValue::Float(f) => [*f, *f],
            AccessorValue::Color(c) => [c[0], c[1]], // Use RG components
            AccessorValue::String(s) => [s.len() as f32, 0.0],
            AccessorValue::Position(p) => *p,
            AccessorValue::Bool(b) => {
                if *b {
                    [1.0, 1.0]
                } else {
                    [0.0, 0.0]
                }
            }
        }
    }

    /// Get the type name for this accessor value.
    pub fn type_name(&self) -> &'static str {
        match self {
            AccessorValue::Float(_) => "f32",
            AccessorValue::Color(_) => "vec4<f32>",
            AccessorValue::String(_) => "string",
            AccessorValue::Position(_) => "vec2<f32>",
            AccessorValue::Bool(_) => "bool",
        }
    }
}

impl From<f32> for AccessorValue {
    fn from(value: f32) -> Self {
        AccessorValue::Float(value)
    }
}

impl From<[f32; 4]> for AccessorValue {
    fn from(value: [f32; 4]) -> Self {
        AccessorValue::Color(value)
    }
}

impl From<String> for AccessorValue {
    fn from(value: String) -> Self {
        AccessorValue::String(value)
    }
}

impl From<&str> for AccessorValue {
    fn from(value: &str) -> Self {
        AccessorValue::String(value.to_string())
    }
}

impl From<[f32; 2]> for AccessorValue {
    fn from(value: [f32; 2]) -> Self {
        AccessorValue::Position(value)
    }
}

impl From<bool> for AccessorValue {
    fn from(value: bool) -> Self {
        AccessorValue::Bool(value)
    }
}

impl<T> AccessorRegistry<T> {
    /// Create a new empty accessor registry.
    pub fn new() -> Self {
        Self {
            field_accessors: HashMap::new(),
            _phantom: PhantomData,
        }
    }

    /// Register a field accessor.
    pub fn register_field<A>(&mut self, field_name: &str, accessor: A)
    where
        A: Accessor<T, Output = AccessorValue> + Send + Sync + 'static,
    {
        self.field_accessors
            .insert(field_name.to_string(), Box::new(accessor));
    }

    /// Extract a value using a field accessor.
    pub fn extract_field(&self, field_name: &str, data: &T) -> Option<AccessorValue> {
        self.field_accessors
            .get(field_name)
            .map(|accessor| accessor.extract(data))
    }

    /// Check if a field accessor is registered.
    pub fn has_field(&self, field_name: &str) -> bool {
        self.field_accessors.contains_key(field_name)
    }

    /// Get all registered field names.
    pub fn field_names(&self) -> Vec<&String> {
        self.field_accessors.keys().collect()
    }
}

impl<T> Default for AccessorRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience trait for converting closures to accessors with specific output types.
pub trait IntoAccessor<T, Output> {
    fn into_accessor(self, name: &str) -> ClosureAccessor<T, Output, Self>
    where
        Self: Sized + Fn(&T) -> Output + Send + Sync;
}

impl<T, Output, F> IntoAccessor<T, Output> for F
where
    F: Fn(&T) -> Output + Send + Sync,
{
    fn into_accessor(self, name: &str) -> ClosureAccessor<T, Output, Self> {
        ClosureAccessor::new(name, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestData {
        x: f32,
        y: f32,
        name: String,
        active: bool,
    }

    #[test]
    fn test_field_accessors() {
        let x_accessor = x("revenue");
        assert_eq!(x_accessor.field_name(), "revenue");

        let y_accessor = y("profit");
        assert_eq!(y_accessor.field_name(), "profit");

        let color_accessor = color("region");
        assert_eq!(color_accessor.field_name(), "region");

        let size_accessor = size("employees");
        assert_eq!(size_accessor.field_name(), "employees");
    }

    #[test]
    fn test_closure_accessor() {
        let data = TestData {
            x: 10.0,
            y: 20.0,
            name: "Test".to_string(),
            active: true,
        };

        let x_accessor = ClosureAccessor::new("x", |d: &TestData| d.x);
        assert_eq!(x_accessor.extract(&data), 10.0);
        assert_eq!(x_accessor.debug_name(), "x");

        let name_accessor = ClosureAccessor::new("name", |d: &TestData| d.name.clone());
        assert_eq!(name_accessor.extract(&data), "Test");
    }

    #[test]
    fn test_constant_accessor() {
        let data = TestData {
            x: 10.0,
            y: 20.0,
            name: "Test".to_string(),
            active: true,
        };

        let red_color = ConstantAccessor::new("red", [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(red_color.extract(&data), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(
            Accessor::<TestData>::debug_name(&red_color),
            "constant(red)"
        );

        let fixed_size = ConstantAccessor::new("size", 5.0);
        assert_eq!(fixed_size.extract(&data), 5.0);
    }

    #[test]
    fn test_accessor_value_conversions() {
        let float_val = AccessorValue::Float(5.0);
        assert_eq!(float_val.as_f32(), 5.0);
        assert_eq!(float_val.as_color(), [5.0, 5.0, 5.0, 1.0]);
        assert_eq!(float_val.as_position(), [5.0, 5.0]);

        let color_val = AccessorValue::Color([1.0, 0.5, 0.0, 0.8]);
        assert_eq!(color_val.as_f32(), 1.0);
        assert_eq!(color_val.as_color(), [1.0, 0.5, 0.0, 0.8]);
        assert_eq!(color_val.as_position(), [1.0, 0.5]);

        let string_val = AccessorValue::String("Hello".to_string());
        assert_eq!(string_val.as_f32(), 5.0); // Length = 5
        assert_eq!(string_val.as_color(), [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(string_val.as_position(), [5.0, 0.0]);

        let bool_val = AccessorValue::Bool(true);
        assert_eq!(bool_val.as_f32(), 1.0);
        assert_eq!(bool_val.as_color(), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(bool_val.as_position(), [1.0, 1.0]);

        let false_val = AccessorValue::Bool(false);
        assert_eq!(false_val.as_f32(), 0.0);
        assert_eq!(false_val.as_color(), [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_accessor_value_type_names() {
        assert_eq!(AccessorValue::Float(1.0).type_name(), "f32");
        assert_eq!(
            AccessorValue::Color([1.0, 0.0, 0.0, 1.0]).type_name(),
            "vec4<f32>"
        );
        assert_eq!(
            AccessorValue::String("test".to_string()).type_name(),
            "string"
        );
        assert_eq!(AccessorValue::Position([1.0, 2.0]).type_name(), "vec2<f32>");
        assert_eq!(AccessorValue::Bool(true).type_name(), "bool");
    }

    #[test]
    fn test_accessor_value_from_conversions() {
        let float_val: AccessorValue = 5.0.into();
        assert_eq!(float_val, AccessorValue::Float(5.0));

        let color_val: AccessorValue = [1.0, 0.0, 0.0, 1.0].into();
        assert_eq!(color_val, AccessorValue::Color([1.0, 0.0, 0.0, 1.0]));

        let string_val: AccessorValue = "hello".into();
        assert_eq!(string_val, AccessorValue::String("hello".to_string()));

        let pos_val: AccessorValue = [1.0, 2.0].into();
        assert_eq!(pos_val, AccessorValue::Position([1.0, 2.0]));

        let bool_val: AccessorValue = true.into();
        assert_eq!(bool_val, AccessorValue::Bool(true));
    }

    #[test]
    fn test_accessor_registry() {
        let mut registry = AccessorRegistry::<TestData>::new();
        assert_eq!(registry.field_names().len(), 0);

        // Register field accessors
        let x_accessor = ClosureAccessor::new("x", |d: &TestData| AccessorValue::Float(d.x));
        let name_accessor =
            ClosureAccessor::new("name", |d: &TestData| AccessorValue::String(d.name.clone()));

        registry.register_field("x", x_accessor);
        registry.register_field("name", name_accessor);

        assert_eq!(registry.field_names().len(), 2);
        assert!(registry.has_field("x"));
        assert!(registry.has_field("name"));
        assert!(!registry.has_field("nonexistent"));

        // Test field extraction
        let data = TestData {
            x: 15.0,
            y: 25.0,
            name: "Sample".to_string(),
            active: false,
        };

        let x_value = registry.extract_field("x", &data).unwrap();
        assert_eq!(x_value, AccessorValue::Float(15.0));

        let name_value = registry.extract_field("name", &data).unwrap();
        assert_eq!(name_value, AccessorValue::String("Sample".to_string()));

        let missing_value = registry.extract_field("missing", &data);
        assert!(missing_value.is_none());
    }

    #[test]
    fn test_into_accessor_trait() {
        let x_accessor = (|d: &TestData| d.x).into_accessor("x_field");
        assert_eq!(x_accessor.debug_name(), "x_field");

        let data = TestData {
            x: 42.0,
            y: 24.0,
            name: "Test".to_string(),
            active: true,
        };

        assert_eq!(x_accessor.extract(&data), 42.0);
    }
}
