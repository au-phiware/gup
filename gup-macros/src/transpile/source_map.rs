// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Source map generation for mapping transpiled WGSL back to Rust source.
//!
//! When WGSL is generated from Rust via the transpilation pipeline, this
//! module records mappings between generated WGSL line/column positions
//! and their original Rust source locations. This aids debugging of
//! transpiled shaders.

use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Source mapping types
// ---------------------------------------------------------------------------

/// A single mapping from a WGSL location to its Rust origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMapping {
    /// 1-based line in the generated WGSL.
    pub wgsl_line: u32,
    /// 1-based column in the generated WGSL.
    pub wgsl_column: u32,
    /// File containing the original Rust source.
    pub rust_file: String,
    /// 1-based line in the Rust source.
    pub rust_line: u32,
    /// 1-based column in the Rust source.
    pub rust_column: u32,
    /// Optional name of the symbol at this location.
    pub name: Option<String>,
}

/// A source map collecting all mappings for a transpilation unit.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    /// Mappings keyed by WGSL line for fast lookup.
    mappings: BTreeMap<u32, Vec<SourceMapping>>,
}

impl SourceMap {
    /// Create a new empty source map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a mapping entry.
    pub fn add_mapping(&mut self, mapping: SourceMapping) {
        self.mappings
            .entry(mapping.wgsl_line)
            .or_default()
            .push(mapping);
    }

    /// Find the Rust source location for a given WGSL position.
    ///
    /// Returns the closest mapping on the given line, or `None` if no
    /// mapping exists for that line.
    pub fn find_rust_location(&self, wgsl_line: u32, wgsl_column: u32) -> Option<&SourceMapping> {
        let line_mappings = self.mappings.get(&wgsl_line)?;

        // Find the mapping with the closest column ≤ wgsl_column.
        line_mappings
            .iter()
            .filter(|m| m.wgsl_column <= wgsl_column)
            .max_by_key(|m| m.wgsl_column)
            .or_else(|| line_mappings.first())
    }

    /// Get all mappings for a given WGSL line.
    pub fn mappings_for_line(&self, wgsl_line: u32) -> &[SourceMapping] {
        self.mappings
            .get(&wgsl_line)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Total number of mapping entries.
    pub fn len(&self) -> usize {
        self.mappings.values().map(|v| v.len()).sum()
    }

    /// Returns true if there are no mappings.
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Generate a human-readable dump of all mappings (for debugging).
    pub fn dump(&self) -> String {
        let mut output = String::new();
        for (line, mappings) in &self.mappings {
            for m in mappings {
                output.push_str(&format!(
                    "WGSL {}:{} -> {}:{}:{}{}\n",
                    line,
                    m.wgsl_column,
                    m.rust_file,
                    m.rust_line,
                    m.rust_column,
                    m.name
                        .as_ref()
                        .map(|n| format!(" ({n})"))
                        .unwrap_or_default()
                ));
            }
        }
        output
    }
}

// ---------------------------------------------------------------------------
// Source map builder (for use during code generation)
// ---------------------------------------------------------------------------

/// A builder that tracks the current WGSL output position and creates
/// source mappings on the fly during code generation.
#[derive(Debug, Clone)]
pub struct SourceMapBuilder {
    source_map: SourceMap,
    current_wgsl_line: u32,
    current_wgsl_column: u32,
    current_rust_file: String,
}

impl SourceMapBuilder {
    /// Create a builder for a given Rust source file.
    pub fn new(rust_file: impl Into<String>) -> Self {
        Self {
            source_map: SourceMap::new(),
            current_wgsl_line: 1,
            current_wgsl_column: 1,
            current_rust_file: rust_file.into(),
        }
    }

    /// Record that the current WGSL position maps to the given Rust
    /// source location.
    pub fn map_position(&mut self, rust_line: u32, rust_column: u32, name: Option<String>) {
        self.source_map.add_mapping(SourceMapping {
            wgsl_line: self.current_wgsl_line,
            wgsl_column: self.current_wgsl_column,
            rust_file: self.current_rust_file.clone(),
            rust_line,
            rust_column,
            name,
        });
    }

    /// Advance the WGSL position by one line (e.g. after writing a
    /// newline).
    pub fn advance_line(&mut self) {
        self.current_wgsl_line += 1;
        self.current_wgsl_column = 1;
    }

    /// Advance the WGSL column position.
    pub fn advance_columns(&mut self, count: u32) {
        self.current_wgsl_column += count;
    }

    /// Set the WGSL position explicitly.
    pub fn set_wgsl_position(&mut self, line: u32, column: u32) {
        self.current_wgsl_line = line;
        self.current_wgsl_column = column;
    }

    /// Consume the builder and return the completed source map.
    pub fn build(self) -> SourceMap {
        self.source_map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_source_map() {
        let sm = SourceMap::new();
        assert!(sm.is_empty());
        assert_eq!(sm.len(), 0);
    }

    #[test]
    fn add_and_find_mapping() {
        let mut sm = SourceMap::new();
        sm.add_mapping(SourceMapping {
            wgsl_line: 1,
            wgsl_column: 1,
            rust_file: "test.rs".to_string(),
            rust_line: 5,
            rust_column: 10,
            name: Some("my_func".to_string()),
        });

        assert_eq!(sm.len(), 1);
        assert!(!sm.is_empty());

        let found = sm.find_rust_location(1, 1);
        assert!(found.is_some());
        let m = found.unwrap();
        assert_eq!(m.rust_line, 5);
        assert_eq!(m.rust_column, 10);
        assert_eq!(m.name.as_deref(), Some("my_func"));
    }

    #[test]
    fn find_closest_column() {
        let mut sm = SourceMap::new();
        sm.add_mapping(SourceMapping {
            wgsl_line: 3,
            wgsl_column: 1,
            rust_file: "a.rs".to_string(),
            rust_line: 10,
            rust_column: 1,
            name: None,
        });
        sm.add_mapping(SourceMapping {
            wgsl_line: 3,
            wgsl_column: 10,
            rust_file: "a.rs".to_string(),
            rust_line: 10,
            rust_column: 20,
            name: None,
        });

        // Column 5 should map to the first mapping (column 1).
        let found = sm.find_rust_location(3, 5).unwrap();
        assert_eq!(found.wgsl_column, 1);

        // Column 15 should map to the second mapping (column 10).
        let found = sm.find_rust_location(3, 15).unwrap();
        assert_eq!(found.wgsl_column, 10);
    }

    #[test]
    fn find_on_missing_line_returns_none() {
        let sm = SourceMap::new();
        assert!(sm.find_rust_location(99, 1).is_none());
    }

    #[test]
    fn mappings_for_line() {
        let mut sm = SourceMap::new();
        sm.add_mapping(SourceMapping {
            wgsl_line: 2,
            wgsl_column: 1,
            rust_file: "x.rs".to_string(),
            rust_line: 1,
            rust_column: 1,
            name: None,
        });
        sm.add_mapping(SourceMapping {
            wgsl_line: 2,
            wgsl_column: 10,
            rust_file: "x.rs".to_string(),
            rust_line: 1,
            rust_column: 15,
            name: None,
        });

        assert_eq!(sm.mappings_for_line(2).len(), 2);
        assert_eq!(sm.mappings_for_line(1).len(), 0);
    }

    #[test]
    fn source_map_builder_basic() {
        let mut builder = SourceMapBuilder::new("shader.rs");
        builder.map_position(1, 1, Some("my_fn".to_string()));
        builder.advance_line();
        builder.map_position(2, 5, None);

        let sm = builder.build();
        assert_eq!(sm.len(), 2);

        let m1 = sm.find_rust_location(1, 1).unwrap();
        assert_eq!(m1.rust_line, 1);
        assert_eq!(m1.name.as_deref(), Some("my_fn"));

        let m2 = sm.find_rust_location(2, 1).unwrap();
        assert_eq!(m2.rust_line, 2);
    }

    #[test]
    fn dump_output() {
        let mut sm = SourceMap::new();
        sm.add_mapping(SourceMapping {
            wgsl_line: 1,
            wgsl_column: 1,
            rust_file: "test.rs".to_string(),
            rust_line: 3,
            rust_column: 5,
            name: Some("foo".to_string()),
        });

        let dump = sm.dump();
        assert!(dump.contains("WGSL 1:1 -> test.rs:3:5 (foo)"));
    }
}
