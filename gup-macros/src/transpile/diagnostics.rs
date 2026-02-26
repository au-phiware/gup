// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Enhanced error reporting and diagnostic system for the transpilation
//! pipeline.
//!
//! Provides structured diagnostics with severity levels, source spans,
//! fix suggestions, and multiple output formats (CLI, IDE-compatible).

use std::fmt;

// ---------------------------------------------------------------------------
// Core diagnostic types
// ---------------------------------------------------------------------------

/// Severity level of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticLevel {
    /// A hint for improvement – not an error.
    Hint,
    /// Informational message.
    Info,
    /// Potential issue that may cause problems.
    Warning,
    /// Error that prevents successful transpilation.
    Error,
}

impl fmt::Display for DiagnosticLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticLevel::Hint => write!(f, "hint"),
            DiagnosticLevel::Info => write!(f, "info"),
            DiagnosticLevel::Warning => write!(f, "warning"),
            DiagnosticLevel::Error => write!(f, "error"),
        }
    }
}

/// A position in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub column: u32,
}

impl Position {
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// A span of source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    /// File name or identifier.
    pub file: String,
    /// Start position.
    pub start: Position,
    /// End position.
    pub end: Position,
}

impl SourceSpan {
    pub fn new(file: impl Into<String>, start: Position, end: Position) -> Self {
        Self {
            file: file.into(),
            start,
            end,
        }
    }

    /// Create a span covering a single line.
    pub fn single_line(file: impl Into<String>, line: u32) -> Self {
        Self {
            file: file.into(),
            start: Position::new(line, 1),
            end: Position::new(line, u32::MAX),
        }
    }
}

impl fmt::Display for SourceSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.start.line, self.start.column)
    }
}

/// A suggested fix for a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Human-readable description of the suggestion.
    pub message: String,
    /// Optional replacement text.
    pub replacement: Option<String>,
}

impl Suggestion {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            replacement: None,
        }
    }

    pub fn with_replacement(message: impl Into<String>, replacement: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            replacement: Some(replacement.into()),
        }
    }
}

/// A complete transpilation diagnostic.
#[derive(Debug, Clone)]
pub struct TranspilationDiagnostic {
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Primary error/warning message.
    pub message: String,
    /// Optional machine-readable error code (e.g. "E0001").
    pub code: Option<String>,
    /// Source location where the issue occurs.
    pub span: Option<SourceSpan>,
    /// Fix suggestions.
    pub suggestions: Vec<Suggestion>,
    /// Additional notes providing context.
    pub notes: Vec<String>,
}

impl TranspilationDiagnostic {
    /// Returns true if this diagnostic is an error.
    pub fn is_error(&self) -> bool {
        self.level == DiagnosticLevel::Error
    }
}

impl fmt::Display for TranspilationDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Default CLI-style formatting.
        format_cli(self, f)
    }
}

// ---------------------------------------------------------------------------
// Diagnostic builder (fluent API)
// ---------------------------------------------------------------------------

/// Fluent builder for constructing diagnostics.
pub struct DiagnosticBuilder {
    inner: TranspilationDiagnostic,
}

impl DiagnosticBuilder {
    /// Create a new error diagnostic.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            inner: TranspilationDiagnostic {
                level: DiagnosticLevel::Error,
                message: message.into(),
                code: None,
                span: None,
                suggestions: vec![],
                notes: vec![],
            },
        }
    }

    /// Create a new warning diagnostic.
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            inner: TranspilationDiagnostic {
                level: DiagnosticLevel::Warning,
                message: message.into(),
                code: None,
                span: None,
                suggestions: vec![],
                notes: vec![],
            },
        }
    }

    /// Create a new info diagnostic.
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            inner: TranspilationDiagnostic {
                level: DiagnosticLevel::Info,
                message: message.into(),
                code: None,
                span: None,
                suggestions: vec![],
                notes: vec![],
            },
        }
    }

    /// Create a new hint diagnostic.
    pub fn hint(message: impl Into<String>) -> Self {
        Self {
            inner: TranspilationDiagnostic {
                level: DiagnosticLevel::Hint,
                message: message.into(),
                code: None,
                span: None,
                suggestions: vec![],
                notes: vec![],
            },
        }
    }

    /// Set the source span.
    pub fn span(mut self, span: SourceSpan) -> Self {
        self.inner.span = Some(span);
        self
    }

    /// Set the error code.
    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.inner.code = Some(code.into());
        self
    }

    /// Add a fix suggestion.
    pub fn suggestion(mut self, suggestion: Suggestion) -> Self {
        self.inner.suggestions.push(suggestion);
        self
    }

    /// Add a help message (shorthand for suggestion with no replacement).
    pub fn help(mut self, message: impl Into<String>) -> Self {
        self.inner.suggestions.push(Suggestion::new(message));
        self
    }

    /// Add a contextual note.
    pub fn note(mut self, message: impl Into<String>) -> Self {
        self.inner.notes.push(message.into());
        self
    }

    /// Build the diagnostic.
    pub fn build(self) -> TranspilationDiagnostic {
        self.inner
    }
}

// ---------------------------------------------------------------------------
// Output formatting
// ---------------------------------------------------------------------------

/// Supported output formats for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticOutputFormat {
    /// Human-readable CLI output (rustc-style).
    Cli,
    /// Short single-line format for IDE integration.
    Short,
    /// JSON format for tool integration.
    Json,
}

/// Format a diagnostic in the CLI (rustc-style) format.
fn format_cli(diag: &TranspilationDiagnostic, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // Level and message
    if let Some(ref code) = diag.code {
        write!(f, "{}[{code}]: {}", diag.level, diag.message)?;
    } else {
        write!(f, "{}: {}", diag.level, diag.message)?;
    }

    // Source location
    if let Some(ref span) = diag.span {
        write!(f, "\n  --> {span}")?;
    }

    // Notes
    for note in &diag.notes {
        write!(f, "\n  = note: {note}")?;
    }

    // Suggestions
    for suggestion in &diag.suggestions {
        write!(f, "\n  = help: {}", suggestion.message)?;
        if let Some(ref replacement) = suggestion.replacement {
            write!(f, "\n           try: `{replacement}`")?;
        }
    }

    Ok(())
}

/// Format a diagnostic as a short single-line string for IDE integration.
pub fn format_short(diag: &TranspilationDiagnostic) -> String {
    let location = diag
        .span
        .as_ref()
        .map(|s| format!("{s}: "))
        .unwrap_or_default();
    let code = diag
        .code
        .as_ref()
        .map(|c| format!("[{c}] "))
        .unwrap_or_default();
    format!("{location}{}: {code}{}", diag.level, diag.message)
}

/// Format a diagnostic as a JSON string for tool integration.
pub fn format_json(diag: &TranspilationDiagnostic) -> String {
    // Manual JSON construction to avoid serde dependency in proc-macro crate.
    let mut json = String::from("{");

    json.push_str(&format!(
        "\"level\":\"{}\",\"message\":\"{}\"",
        diag.level,
        escape_json(&diag.message)
    ));

    if let Some(ref code) = diag.code {
        json.push_str(&format!(",\"code\":\"{}\"", escape_json(code)));
    }

    if let Some(ref span) = diag.span {
        json.push_str(&format!(
            ",\"span\":{{\"file\":\"{}\",\"start_line\":{},\"start_column\":{},\"end_line\":{},\"end_column\":{}}}",
            escape_json(&span.file),
            span.start.line,
            span.start.column,
            span.end.line,
            span.end.column,
        ));
    }

    if !diag.suggestions.is_empty() {
        json.push_str(",\"suggestions\":[");
        for (i, s) in diag.suggestions.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&format!("{{\"message\":\"{}\"", escape_json(&s.message)));
            if let Some(ref r) = s.replacement {
                json.push_str(&format!(",\"replacement\":\"{}\"", escape_json(r)));
            }
            json.push('}');
        }
        json.push(']');
    }

    if !diag.notes.is_empty() {
        json.push_str(",\"notes\":[");
        for (i, n) in diag.notes.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&format!("\"{}\"", escape_json(n)));
        }
        json.push(']');
    }

    json.push('}');
    json
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Format a diagnostic in the requested format.
pub fn format_diagnostic(diag: &TranspilationDiagnostic, format: DiagnosticOutputFormat) -> String {
    match format {
        DiagnosticOutputFormat::Cli => format!("{diag}"),
        DiagnosticOutputFormat::Short => format_short(diag),
        DiagnosticOutputFormat::Json => format_json(diag),
    }
}

// ---------------------------------------------------------------------------
// Common diagnostic constructors
// ---------------------------------------------------------------------------

/// Create an "unsupported type" error with a suggestion.
pub fn unsupported_type_error(
    type_name: &str,
    suggestion: &str,
    span: Option<SourceSpan>,
) -> TranspilationDiagnostic {
    let mut builder =
        DiagnosticBuilder::error(format!("type `{type_name}` is not supported in WGSL"))
            .code("E0001")
            .help(suggestion.to_string());

    if let Some(s) = span {
        builder = builder.span(s);
    }

    builder.build()
}

/// Create a "unsupported expression" error.
pub fn unsupported_expression_error(
    expr_desc: &str,
    span: Option<SourceSpan>,
) -> TranspilationDiagnostic {
    let mut builder = DiagnosticBuilder::error(format!(
        "expression `{expr_desc}` cannot be transpiled to WGSL"
    ))
    .code("E0002")
    .note("Only a subset of Rust expressions are supported in shader functions".to_string());

    if let Some(s) = span {
        builder = builder.span(s);
    }

    builder.build()
}

/// Create a "unsupported method" error with known alternatives.
pub fn unsupported_method_error(
    method_name: &str,
    alternatives: &[&str],
    span: Option<SourceSpan>,
) -> TranspilationDiagnostic {
    let mut builder = DiagnosticBuilder::error(format!(
        "method `{method_name}` is not supported in WGSL transpilation"
    ))
    .code("E0003");

    if !alternatives.is_empty() {
        let alts = alternatives.join("`, `");
        builder = builder.help(format!("consider using: `{alts}`"));
    }

    if let Some(s) = span {
        builder = builder.span(s);
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_builder_error() {
        let diag = DiagnosticBuilder::error("something went wrong")
            .code("E0001")
            .span(SourceSpan::new(
                "shader.rs",
                Position::new(10, 5),
                Position::new(10, 20),
            ))
            .help("try doing it differently")
            .note("this is a known limitation")
            .build();

        assert_eq!(diag.level, DiagnosticLevel::Error);
        assert_eq!(diag.message, "something went wrong");
        assert_eq!(diag.code, Some("E0001".to_string()));
        assert!(diag.span.is_some());
        assert_eq!(diag.suggestions.len(), 1);
        assert_eq!(diag.notes.len(), 1);
        assert!(diag.is_error());
    }

    #[test]
    fn diagnostic_builder_warning() {
        let diag = DiagnosticBuilder::warning("might be slow").build();
        assert_eq!(diag.level, DiagnosticLevel::Warning);
        assert!(!diag.is_error());
    }

    #[test]
    fn diagnostic_builder_with_suggestion_replacement() {
        let diag = DiagnosticBuilder::error("type not supported")
            .suggestion(Suggestion::with_replacement(
                "Use f32 instead of f64",
                "f32",
            ))
            .build();

        assert_eq!(diag.suggestions.len(), 1);
        assert_eq!(diag.suggestions[0].replacement, Some("f32".to_string()));
    }

    #[test]
    fn format_cli_output() {
        let diag = DiagnosticBuilder::error("type `f64` not supported")
            .code("E0001")
            .span(SourceSpan::new(
                "shader.rs",
                Position::new(5, 12),
                Position::new(5, 15),
            ))
            .help("Use f32 instead")
            .note("WGSL only supports 32-bit floats")
            .build();

        let output = format!("{diag}");
        assert!(output.contains("error[E0001]: type `f64` not supported"));
        assert!(output.contains("--> shader.rs:5:12"));
        assert!(output.contains("= note: WGSL only supports 32-bit floats"));
        assert!(output.contains("= help: Use f32 instead"));
    }

    #[test]
    fn format_short_output() {
        let diag = DiagnosticBuilder::error("type not supported")
            .span(SourceSpan::new(
                "test.rs",
                Position::new(3, 1),
                Position::new(3, 10),
            ))
            .code("E0001")
            .build();

        let output = format_short(&diag);
        assert_eq!(output, "test.rs:3:1: error: [E0001] type not supported");
    }

    #[test]
    fn format_json_output() {
        let diag = DiagnosticBuilder::warning("potential issue")
            .code("W0001")
            .help("do something")
            .build();

        let json = format_json(&diag);
        assert!(json.contains("\"level\":\"warning\""));
        assert!(json.contains("\"message\":\"potential issue\""));
        assert!(json.contains("\"code\":\"W0001\""));
        assert!(json.contains("\"suggestions\":["));
    }

    #[test]
    fn format_diagnostic_dispatches() {
        let diag = DiagnosticBuilder::info("note").build();
        let cli = format_diagnostic(&diag, DiagnosticOutputFormat::Cli);
        let short = format_diagnostic(&diag, DiagnosticOutputFormat::Short);
        let json = format_diagnostic(&diag, DiagnosticOutputFormat::Json);

        assert!(cli.starts_with("info:"));
        assert!(short.starts_with("info:"));
        assert!(json.starts_with('{'));
    }

    #[test]
    fn unsupported_type_error_has_suggestion() {
        let diag = unsupported_type_error("f64", "Use f32 instead", None);
        assert!(diag.is_error());
        assert!(diag.message.contains("f64"));
        assert_eq!(diag.suggestions.len(), 1);
        assert!(diag.suggestions[0].message.contains("f32"));
    }

    #[test]
    fn unsupported_method_error_lists_alternatives() {
        let diag = unsupported_method_error("to_string", &["abs", "sqrt"], None);
        assert!(diag.is_error());
        assert!(diag.suggestions[0].message.contains("abs"));
        assert!(diag.suggestions[0].message.contains("sqrt"));
    }

    #[test]
    fn diagnostic_level_ordering() {
        assert!(DiagnosticLevel::Hint < DiagnosticLevel::Info);
        assert!(DiagnosticLevel::Info < DiagnosticLevel::Warning);
        assert!(DiagnosticLevel::Warning < DiagnosticLevel::Error);
    }

    #[test]
    fn source_span_display() {
        let span = SourceSpan::new("file.rs", Position::new(10, 5), Position::new(10, 20));
        assert_eq!(format!("{span}"), "file.rs:10:5");
    }
}
