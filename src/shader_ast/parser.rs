// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WGSL parser — converts WGSL text into the AST defined in [`super::types`].
//!
//! This is a purpose-built, minimal parser targeting the subset of WGSL used
//! in Gup's shader function composition system.  It is not a full WGSL parser;
//! it handles functions, struct definitions, global variable declarations,
//! and the expression / statement forms that appear in composed shader
//! functions.

use super::types::*;
use crate::error::GupError;
use std::iter::Peekable;
use std::str::Chars;

// ---------------------------------------------------------------------------
// Token types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    // Punctuation
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    Arrow, // ->

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Equal,        // =
    EqualEqual,   // ==
    BangEqual,    // !=
    Less,         // <
    LessEqual,    // <=
    Greater,      // >
    GreaterEqual, // >=
    AmpAmp,       // &&
    PipePipe,     // ||
    Bang,         // !
    Ampersand,    // & (single)
    Pipe,         // | (single)
    Caret,        // ^
    ShiftLeft,    // <<
    ShiftRight,   // >>
    PlusEqual,    // +=

    // Keywords
    Fn,
    Let,
    Var,
    Return,
    If,
    Else,
    For,
    Struct,
    True,
    False,
    Loop,
    Break,
    Continue,
    Switch,
    Case,
    Default,
    Const,

    // Literals
    FloatLiteral(f64),
    IntLiteral(i64),
    UIntLiteral(u64),

    // Identifiers & attributes
    Ident(String),
    At, // @

    // End
    Eof,
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
            line: 1,
            col: 1,
        }
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.next();
        if let Some(c) = ch {
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        ch
    }

    fn peek(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(&c) if c.is_whitespace() => {
                    self.advance();
                }
                Some(&'/') => {
                    // Peek further to distinguish / from //
                    let mut clone = self.chars.clone();
                    clone.next(); // consume '/'
                    if clone.peek() == Some(&'/') {
                        // Line comment — skip to newline
                        while let Some(&c) = self.peek() {
                            if c == '\n' {
                                break;
                            }
                            self.advance();
                        }
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
    }

    fn read_number(&mut self, first: char) -> Token {
        // Check for hex: 0x...
        if first == '0' && self.peek() == Some(&'x') {
            self.advance(); // consume 'x'
            let mut hex = String::new();
            while let Some(&c) = self.peek() {
                if c.is_ascii_hexdigit() {
                    hex.push(c);
                    self.advance();
                } else {
                    break;
                }
            }
            // Check for u suffix
            if self.peek() == Some(&'u') {
                self.advance();
                let val = u64::from_str_radix(&hex, 16).unwrap_or(0);
                return Token::UIntLiteral(val);
            }
            let val = i64::from_str_radix(&hex, 16).unwrap_or(0);
            return Token::IntLiteral(val);
        }

        let mut s = String::new();
        s.push(first);
        let mut has_dot = false;
        let mut is_uint = false;

        while let Some(&c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else if c == '.' && !has_dot {
                has_dot = true;
                s.push(c);
                self.advance();
            } else if c == 'u' && !has_dot {
                is_uint = true;
                self.advance();
                break;
            } else {
                break;
            }
        }

        if is_uint {
            Token::UIntLiteral(s.parse().unwrap_or(0))
        } else if has_dot {
            Token::FloatLiteral(s.parse().unwrap_or(0.0))
        } else {
            Token::IntLiteral(s.parse().unwrap_or(0))
        }
    }

    fn read_ident(&mut self, first: char) -> Token {
        let mut s = String::new();
        s.push(first);

        while let Some(&c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        match s.as_str() {
            "fn" => Token::Fn,
            "let" => Token::Let,
            "var" => Token::Var,
            "return" => Token::Return,
            "if" => Token::If,
            "else" => Token::Else,
            "for" => Token::For,
            "struct" => Token::Struct,
            "true" => Token::True,
            "false" => Token::False,
            "loop" => Token::Loop,
            "break" => Token::Break,
            "continue" => Token::Continue,
            "switch" => Token::Switch,
            "case" => Token::Case,
            "default" => Token::Default,
            "const" => Token::Const,
            _ => Token::Ident(s),
        }
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace_and_comments();

        let ch = match self.advance() {
            Some(c) => c,
            None => return Ok(Token::Eof),
        };

        match ch {
            '(' => Ok(Token::LeftParen),
            ')' => Ok(Token::RightParen),
            '{' => Ok(Token::LeftBrace),
            '}' => Ok(Token::RightBrace),
            '[' => Ok(Token::LeftBracket),
            ']' => Ok(Token::RightBracket),
            ',' => Ok(Token::Comma),
            ':' => Ok(Token::Colon),
            ';' => Ok(Token::Semicolon),
            '.' => Ok(Token::Dot),
            '@' => Ok(Token::At),
            '+' => {
                if self.peek() == Some(&'=') {
                    self.advance();
                    Ok(Token::PlusEqual)
                } else {
                    Ok(Token::Plus)
                }
            }
            '*' => Ok(Token::Star),
            '%' => Ok(Token::Percent),
            '-' => {
                if self.peek() == Some(&'>') {
                    self.advance();
                    Ok(Token::Arrow)
                } else {
                    Ok(Token::Minus)
                }
            }
            '/' => Ok(Token::Slash),
            '=' => {
                if self.peek() == Some(&'=') {
                    self.advance();
                    Ok(Token::EqualEqual)
                } else {
                    Ok(Token::Equal)
                }
            }
            '!' => {
                if self.peek() == Some(&'=') {
                    self.advance();
                    Ok(Token::BangEqual)
                } else {
                    Ok(Token::Bang)
                }
            }
            '<' => {
                if self.peek() == Some(&'=') {
                    self.advance();
                    Ok(Token::LessEqual)
                } else if self.peek() == Some(&'<') {
                    self.advance();
                    Ok(Token::ShiftLeft)
                } else {
                    Ok(Token::Less)
                }
            }
            '>' => {
                if self.peek() == Some(&'=') {
                    self.advance();
                    Ok(Token::GreaterEqual)
                } else if self.peek() == Some(&'>') {
                    self.advance();
                    Ok(Token::ShiftRight)
                } else {
                    Ok(Token::Greater)
                }
            }
            '&' => {
                if self.peek() == Some(&'&') {
                    self.advance();
                    Ok(Token::AmpAmp)
                } else {
                    Ok(Token::Ampersand)
                }
            }
            '|' => {
                if self.peek() == Some(&'|') {
                    self.advance();
                    Ok(Token::PipePipe)
                } else {
                    Ok(Token::Pipe)
                }
            }
            '^' => Ok(Token::Caret),
            c if c.is_ascii_digit() => Ok(self.read_number(c)),
            c if c.is_alphabetic() || c == '_' => Ok(self.read_ident(c)),
            c => Err(ParseError::unexpected_char(c, self.line, self.col)),
        }
    }

    fn position(&self) -> (usize, usize) {
        (self.line, self.col)
    }
}

// ---------------------------------------------------------------------------
// Parse errors
// ---------------------------------------------------------------------------

/// Error produced during WGSL parsing.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
    pub suggestion: Option<String>,
}

impl ParseError {
    pub fn new(message: impl Into<String>, line: usize, col: usize) -> Self {
        Self {
            message: message.into(),
            line,
            col,
            suggestion: None,
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    fn unexpected_char(ch: char, line: usize, col: usize) -> Self {
        Self::new(format!("unexpected character '{ch}'"), line, col)
    }

    fn unexpected_token(token: &Token, expected: &str, line: usize, col: usize) -> Self {
        Self::new(format!("expected {expected}, found {token:?}"), line, col)
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WGSL parse error at line {}, col {}: {}",
            self.line, self.col, self.message
        )?;
        if let Some(ref sug) = self.suggestion {
            write!(f, " (suggestion: {sug})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError {}

impl From<ParseError> for GupError {
    fn from(e: ParseError) -> Self {
        GupError::ShaderCompilationError {
            shader_type: "WGSL AST parse".to_string(),
            error: e.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parses a WGSL text string into a [`WgslModule`].
pub struct WgslParser {
    tokens: Vec<Token>,
    pos: usize,
    /// Position tracking (line, col) for each token.
    positions: Vec<(usize, usize)>,
}

impl WgslParser {
    /// Create a parser from WGSL source text.
    pub fn new(source: &str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        let mut tokens = Vec::new();
        let mut positions = Vec::new();

        loop {
            let pos = lexer.position();
            let token = lexer.next_token()?;
            let is_eof = token == Token::Eof;
            positions.push(pos);
            tokens.push(token);
            if is_eof {
                break;
            }
        }

        Ok(Self {
            tokens,
            pos: 0,
            positions,
        })
    }

    /// Parse the entire module.
    pub fn parse_module(&mut self) -> Result<WgslModule, ParseError> {
        let mut module = WgslModule::new();

        while !self.is_at_end() {
            // Skip any attributes we encounter at top level
            let attrs = self.parse_attributes()?;

            match self.peek() {
                Token::Fn => {
                    let func = self.parse_function(attrs)?;
                    module.functions.push(func);
                }
                Token::Struct => {
                    let s = self.parse_struct()?;
                    module.structs.push(s);
                }
                Token::Const => {
                    let c = self.parse_global_const()?;
                    module.constants.push(c);
                }
                Token::Eof => break,
                tok => {
                    // Try to parse as a global variable declaration
                    if let Token::Ident(id) = tok
                        && (id == "var" || id.starts_with("var<"))
                    {
                        let global = self.parse_global_var(attrs)?;
                        module.globals.push(global);
                        continue;
                    }
                    // Also handle Var token
                    if tok == &Token::Var {
                        let global = self.parse_global_var(attrs)?;
                        module.globals.push(global);
                        continue;
                    }
                    let (line, col) = self.current_position();
                    return Err(ParseError::unexpected_token(
                        &tok.clone(),
                        "fn, struct, var, or const declaration",
                        line,
                        col,
                    ));
                }
            }
        }

        Ok(module)
    }

    // --- Helpers ---

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }

    fn current_position(&self) -> (usize, usize) {
        self.positions.get(self.pos).copied().unwrap_or((0, 0))
    }

    fn expect(&mut self, expected: &Token) -> Result<(), ParseError> {
        let (line, col) = self.current_position();
        let tok = self.advance();
        if std::mem::discriminant(&tok) == std::mem::discriminant(expected) {
            Ok(())
        } else {
            Err(ParseError::unexpected_token(
                &tok,
                &format!("{expected:?}"),
                line,
                col,
            ))
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let (line, col) = self.current_position();
        let tok = self.advance();
        match tok {
            Token::Ident(s) => Ok(s),
            // Accept keywords that are used as identifiers in some contexts
            // (e.g., @builtin(position) where "position" could be shadowed).
            other => Err(ParseError::unexpected_token(
                &other,
                "identifier",
                line,
                col,
            )),
        }
    }

    /// Expect a `>` token, handling the case where `>>` was tokenized as a
    /// single `ShiftRight` token.
    fn expect_greater(&mut self) -> Result<(), ParseError> {
        let (line, col) = self.current_position();
        match self.peek().clone() {
            Token::Greater => {
                self.advance();
                Ok(())
            }
            Token::ShiftRight => {
                // >> was tokenized as one token; consume it and insert a
                // replacement `Greater` so the remaining `>` is available.
                let pos = self.positions[self.pos];
                self.advance(); // consume ShiftRight
                self.tokens.insert(self.pos, Token::Greater);
                self.positions.insert(self.pos, pos);
                Ok(())
            }
            ref tok => Err(ParseError::unexpected_token(tok, ">", line, col)),
        }
    }

    // --- Attributes ---

    fn parse_attributes(&mut self) -> Result<Vec<Attribute>, ParseError> {
        let mut attrs = Vec::new();
        while self.peek() == &Token::At {
            self.advance(); // consume @
            let name = self.expect_ident()?;
            let attr = match name.as_str() {
                "vertex" => Attribute::Vertex,
                "fragment" => Attribute::Fragment,
                "compute" => Attribute::Compute,
                "workgroup_size" => {
                    self.expect(&Token::LeftParen)?;
                    let x = self.parse_u32_literal()?;
                    let y = if self.peek() == &Token::Comma {
                        self.advance();
                        // Check if next is RightParen (trailing comma)
                        if self.peek() == &Token::RightParen {
                            None
                        } else {
                            Some(self.parse_u32_literal()?)
                        }
                    } else {
                        None
                    };
                    let z = if y.is_some() && self.peek() == &Token::Comma {
                        self.advance();
                        if self.peek() == &Token::RightParen {
                            None
                        } else {
                            Some(self.parse_u32_literal()?)
                        }
                    } else {
                        None
                    };
                    self.expect(&Token::RightParen)?;
                    Attribute::WorkgroupSize(x, y, z)
                }
                "group" => {
                    self.expect(&Token::LeftParen)?;
                    let n = self.parse_u32_literal()?;
                    self.expect(&Token::RightParen)?;
                    Attribute::Group(n)
                }
                "binding" => {
                    self.expect(&Token::LeftParen)?;
                    let n = self.parse_u32_literal()?;
                    self.expect(&Token::RightParen)?;
                    Attribute::Binding(n)
                }
                "location" => {
                    self.expect(&Token::LeftParen)?;
                    let n = self.parse_u32_literal()?;
                    self.expect(&Token::RightParen)?;
                    Attribute::Location(n)
                }
                "builtin" => {
                    self.expect(&Token::LeftParen)?;
                    let builtin_name = self.expect_ident()?;
                    self.expect(&Token::RightParen)?;
                    Attribute::Builtin(builtin_name)
                }
                other => Attribute::Custom(other.to_string()),
            };
            attrs.push(attr);
        }
        Ok(attrs)
    }

    fn parse_u32_literal(&mut self) -> Result<u32, ParseError> {
        let (line, col) = self.current_position();
        let tok = self.advance();
        match tok {
            Token::IntLiteral(n) => Ok(n as u32),
            Token::UIntLiteral(n) => Ok(n as u32),
            other => Err(ParseError::unexpected_token(
                &other,
                "integer literal",
                line,
                col,
            )),
        }
    }

    // --- Type parsing ---

    fn parse_type(&mut self) -> Result<WgslType, ParseError> {
        let (line, col) = self.current_position();
        let tok = self.advance();
        match tok {
            Token::Ident(name) => self.resolve_type_name(&name, line, col),
            other => Err(ParseError::unexpected_token(&other, "type name", line, col)),
        }
    }

    fn resolve_type_name(
        &mut self,
        name: &str,
        line: usize,
        col: usize,
    ) -> Result<WgslType, ParseError> {
        match name {
            "f32" => Ok(WgslType::Scalar(ScalarType::F32)),
            "i32" => Ok(WgslType::Scalar(ScalarType::I32)),
            "u32" => Ok(WgslType::Scalar(ScalarType::U32)),
            "bool" => Ok(WgslType::Scalar(ScalarType::Bool)),
            n if n.starts_with("vec") && n.len() == 4 => {
                let dim = n.chars().nth(3).and_then(|c| c.to_digit(10));
                match dim {
                    Some(d @ 2..=4) => {
                        // Check for <scalar> suffix
                        if self.peek() == &Token::Less {
                            self.advance(); // <
                            let scalar = self.parse_scalar_type()?;
                            self.expect_greater()?;
                            Ok(WgslType::Vector(scalar, d as u8))
                        } else {
                            // Default to f32
                            Ok(WgslType::Vector(ScalarType::F32, d as u8))
                        }
                    }
                    _ => Err(ParseError::new(
                        format!("invalid vector dimension in '{name}'"),
                        line,
                        col,
                    )),
                }
            }
            n if n.starts_with("mat") => {
                // mat{C}x{R} or mat{C}x{R}<scalar>
                let rest = &n[3..];
                let parts: Vec<&str> = rest.split('x').collect();
                if parts.len() == 2 {
                    let cols: u8 = parts[0].parse().map_err(|_| {
                        ParseError::new(format!("invalid matrix type '{name}'"), line, col)
                    })?;
                    let rows: u8 = parts[1].parse().map_err(|_| {
                        ParseError::new(format!("invalid matrix type '{name}'"), line, col)
                    })?;
                    if self.peek() == &Token::Less {
                        self.advance();
                        let scalar = self.parse_scalar_type()?;
                        self.expect_greater()?;
                        Ok(WgslType::Matrix(scalar, cols, rows))
                    } else {
                        Ok(WgslType::Matrix(ScalarType::F32, cols, rows))
                    }
                } else {
                    Err(ParseError::new(
                        format!("invalid matrix type '{name}'"),
                        line,
                        col,
                    ))
                }
            }
            "array" => {
                self.expect(&Token::Less)?;
                let elem = self.parse_type()?;
                let size = if self.peek() == &Token::Comma {
                    self.advance();
                    Some(self.parse_u32_literal()?)
                } else {
                    None
                };
                self.expect_greater()?;
                Ok(WgslType::Array(Box::new(elem), size))
            }
            "atomic" => {
                self.expect(&Token::Less)?;
                let inner = self.parse_type()?;
                self.expect_greater()?;
                Ok(WgslType::Atomic(Box::new(inner)))
            }
            "ptr" => {
                self.expect(&Token::Less)?;
                let space_name = self.expect_ident()?;
                let addr_space = match space_name.as_str() {
                    "function" => AddressSpace::Function,
                    "private" => AddressSpace::Private,
                    "workgroup" => AddressSpace::Workgroup,
                    "uniform" => AddressSpace::Uniform,
                    "storage" => AddressSpace::Storage(AccessMode::Read),
                    _ => AddressSpace::Function,
                };
                self.expect(&Token::Comma)?;
                let inner = self.parse_type()?;
                self.expect_greater()?;
                Ok(WgslType::Pointer(addr_space, Box::new(inner)))
            }
            other => Ok(WgslType::Struct(other.to_string())),
        }
    }

    fn parse_scalar_type(&mut self) -> Result<ScalarType, ParseError> {
        let (line, col) = self.current_position();
        let name = self.expect_ident()?;
        match name.as_str() {
            "f32" => Ok(ScalarType::F32),
            "i32" => Ok(ScalarType::I32),
            "u32" => Ok(ScalarType::U32),
            "bool" => Ok(ScalarType::Bool),
            _ => Err(ParseError::new(
                format!("expected scalar type, found '{name}'"),
                line,
                col,
            )),
        }
    }

    // --- Struct ---

    fn parse_struct(&mut self) -> Result<StructDef, ParseError> {
        self.expect(&Token::Struct)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LeftBrace)?;

        let mut fields = Vec::new();
        while self.peek() != &Token::RightBrace {
            let attrs = self.parse_attributes()?;
            let field_name = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let ty = self.parse_type()?;
            // Optional comma or trailing comma
            if self.peek() == &Token::Comma {
                self.advance();
            }
            fields.push(StructField {
                name: field_name,
                ty,
                attributes: attrs,
            });
        }
        self.expect(&Token::RightBrace)?;
        Ok(StructDef { name, fields })
    }

    // --- Global var ---

    fn parse_global_var(&mut self, attributes: Vec<Attribute>) -> Result<GlobalVar, ParseError> {
        self.advance(); // consume 'var'

        // Parse address space: var<uniform>, var<storage, read_write>, etc.
        let address_space = if self.peek() == &Token::Less {
            self.advance(); // <
            let space_name = self.expect_ident()?;
            
            match space_name.as_str() {
                "uniform" => {
                    self.expect_greater()?;
                    AddressSpace::Uniform
                }
                "storage" => {
                    // Check for access mode: var<storage, read> or var<storage, read_write>
                    if self.peek() == &Token::Comma {
                        self.advance(); // consume ','
                        let mode_name = self.expect_ident()?;
                        let mode = match mode_name.as_str() {
                            "read" => AccessMode::Read,
                            "read_write" => AccessMode::ReadWrite,
                            _ => AccessMode::Read,
                        };
                        self.expect_greater()?;
                        AddressSpace::Storage(mode)
                    } else {
                        self.expect_greater()?;
                        AddressSpace::Storage(AccessMode::Read)
                    }
                }
                "private" => {
                    self.expect_greater()?;
                    AddressSpace::Private
                }
                "workgroup" => {
                    self.expect_greater()?;
                    AddressSpace::Workgroup
                }
                _ => {
                    self.expect_greater()?;
                    AddressSpace::Private
                }
            }
        } else {
            AddressSpace::Private
        };

        let name = self.expect_ident()?;
        self.expect(&Token::Colon)?;
        let ty = self.parse_type()?;
        self.expect(&Token::Semicolon)?;

        Ok(GlobalVar {
            name,
            ty,
            address_space,
            attributes,
        })
    }

    // --- Const ---

    fn parse_global_const(&mut self) -> Result<GlobalConst, ParseError> {
        self.advance(); // consume 'const'
        let name = self.expect_ident()?;
        let ty = if self.peek() == &Token::Colon {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(&Token::Equal)?;
        let value = self.parse_expression()?;
        self.expect(&Token::Semicolon)?;
        Ok(GlobalConst { name, ty, value })
    }

    // --- Function ---

    fn parse_function(&mut self, attributes: Vec<Attribute>) -> Result<Function, ParseError> {
        self.expect(&Token::Fn)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LeftParen)?;

        let mut parameters = Vec::new();
        while self.peek() != &Token::RightParen {
            // Skip any parameter attributes (e.g., @builtin(vertex_index))
            let param_attrs = self.parse_attributes()?;
            let param_name = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let ty = self.parse_type()?;
            parameters.push(Parameter {
                name: param_name,
                ty,
                attributes: param_attrs,
            });
            if self.peek() == &Token::Comma {
                self.advance();
            }
        }
        self.expect(&Token::RightParen)?;

        let (return_type, return_attributes) = if self.peek() == &Token::Arrow {
            self.advance();
            // Check for @location(N) attribute before return type
            let return_attrs = self.parse_attributes()?;
            (Some(self.parse_type()?), return_attrs)
        } else {
            (None, vec![])
        };

        let body = self.parse_block()?;

        Ok(Function {
            name,
            parameters,
            return_type,
            return_attributes,
            body,
            attributes,
        })
    }

    // --- Block ---

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        self.expect(&Token::LeftBrace)?;
        let mut stmts = Vec::new();

        while self.peek() != &Token::RightBrace && !self.is_at_end() {
            stmts.push(self.parse_statement()?);
        }

        self.expect(&Token::RightBrace)?;
        Ok(Block::new(stmts))
    }

    // --- Statements ---

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek().clone() {
            Token::Let | Token::Const => self.parse_let(false),
            Token::Var => self.parse_let(true),
            Token::Return => self.parse_return(),
            Token::If => self.parse_if(),
            Token::For => self.parse_for(),
            Token::Loop => self.parse_loop(),
            Token::Break => {
                self.advance();
                self.expect(&Token::Semicolon)?;
                Ok(Statement::Break)
            }
            Token::Continue => {
                self.advance();
                self.expect(&Token::Semicolon)?;
                Ok(Statement::Continue)
            }
            Token::Switch => self.parse_switch(),
            Token::LeftBrace => {
                let block = self.parse_block()?;
                Ok(Statement::Block(block))
            }
            _ => self.parse_expr_or_assign_statement(),
        }
    }

    fn parse_let(&mut self, mutable: bool) -> Result<Statement, ParseError> {
        self.advance(); // consume let/var
        let name = self.expect_ident()?;
        let ty = if self.peek() == &Token::Colon {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        // Handle both `var x: T;` (no initializer) and `var x: T = expr;`
        let value = if self.peek() == &Token::Equal {
            self.advance();
            self.parse_expression()?
        } else {
            // No initializer — use a default-constructed expression
            // For type-annotated vars this is valid in WGSL
            if let Some(ref t) = ty {
                Expr::TypeConstructor(t.clone(), vec![])
            } else {
                // Uninitialized without type annotation is an error in WGSL
                let (line, col) = self.current_position();
                return Err(ParseError::new(
                    "variable declaration requires either a type annotation or an initializer",
                    line,
                    col,
                ));
            }
        };
        self.expect(&Token::Semicolon)?;
        Ok(Statement::Let {
            name,
            ty,
            value,
            mutable,
        })
    }

    fn parse_return(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume return
        if self.peek() == &Token::Semicolon {
            self.advance();
            return Ok(Statement::Return(None));
        }
        let expr = self.parse_expression()?;
        self.expect(&Token::Semicolon)?;
        Ok(Statement::Return(Some(expr)))
    }

    fn parse_if(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume if
        // WGSL uses `if (cond)` or `if cond`
        let has_paren = if self.peek() == &Token::LeftParen {
            self.advance();
            true
        } else {
            false
        };
        let condition = self.parse_expression()?;
        if has_paren {
            self.expect(&Token::RightParen)?;
        }
        let body = self.parse_block()?;
        let else_body = if self.peek() == &Token::Else {
            self.advance();
            // Handle `else if` by wrapping in a block with an if statement
            if self.peek() == &Token::If {
                let nested_if = self.parse_if()?;
                Some(Block::new(vec![nested_if]))
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        Ok(Statement::If {
            condition,
            body,
            else_body,
        })
    }

    fn parse_for(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume for
        self.expect(&Token::LeftParen)?;

        let init = if self.peek() == &Token::Semicolon {
            self.advance();
            None
        } else {
            let stmt = self.parse_statement()?;
            // Note: the statement parse already consumed the semicolon
            Some(Box::new(stmt))
        };

        let condition = if self.peek() == &Token::Semicolon {
            self.advance();
            None
        } else {
            let expr = self.parse_expression()?;
            self.expect(&Token::Semicolon)?;
            Some(expr)
        };

        let update = if self.peek() == &Token::RightParen {
            None
        } else {
            // Parse update expression as a statement
            let expr = self.parse_expression()?;
            Some(Box::new(Statement::Expression(expr)))
        };

        self.expect(&Token::RightParen)?;
        let body = self.parse_block()?;

        Ok(Statement::For {
            init,
            condition,
            update,
            body,
        })
    }

    fn parse_loop(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'loop'
        let body = self.parse_block()?;
        Ok(Statement::Loop { body })
    }

    fn parse_switch(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 'switch'

        // Parse subject expression, with optional parens
        let has_paren = if self.peek() == &Token::LeftParen {
            self.advance();
            true
        } else {
            false
        };
        let subject = self.parse_expression()?;
        if has_paren {
            self.expect(&Token::RightParen)?;
        }

        self.expect(&Token::LeftBrace)?;
        let mut cases = Vec::new();

        while self.peek() != &Token::RightBrace && !self.is_at_end() {
            match self.peek().clone() {
                Token::Case => {
                    self.advance(); // consume 'case'
                    let selector = self.parse_expression()?;
                    self.expect(&Token::Colon)?;
                    let body = self.parse_block()?;
                    cases.push(SwitchCase {
                        selector: Some(selector),
                        body,
                    });
                }
                Token::Default => {
                    self.advance(); // consume 'default'
                    self.expect(&Token::Colon)?;
                    let body = self.parse_block()?;
                    cases.push(SwitchCase {
                        selector: None,
                        body,
                    });
                }
                _ => {
                    let (line, col) = self.current_position();
                    return Err(ParseError::new(
                        "expected 'case' or 'default' in switch",
                        line,
                        col,
                    ));
                }
            }
        }

        self.expect(&Token::RightBrace)?;
        Ok(Statement::Switch { subject, cases })
    }

    fn parse_expr_or_assign_statement(&mut self) -> Result<Statement, ParseError> {
        let expr = self.parse_expression()?;
        match self.peek() {
            Token::Equal => {
                self.advance();
                let value = self.parse_expression()?;
                self.expect(&Token::Semicolon)?;
                Ok(Statement::Assign(expr, value))
            }
            Token::PlusEqual => {
                self.advance();
                let value = self.parse_expression()?;
                self.expect(&Token::Semicolon)?;
                Ok(Statement::CompoundAssign(expr, BinaryOp::Add, value))
            }
            _ => {
                self.expect(&Token::Semicolon)?;
                Ok(Statement::Expression(expr))
            }
        }
    }

    // --- Expressions (precedence climbing) ---
    // WGSL precedence (low to high):
    //   ||  →  &&  →  |  →  ^  →  &  →  ==,!=  →  <,>,<=,>=  →  <<,>>  →  +,-  →  *,/,%  →  unary  →  postfix

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and_expr()?;
        while self.peek() == &Token::PipePipe {
            self.advance();
            let right = self.parse_and_expr()?;
            left = Expr::Binary(Box::new(left), BinaryOp::Or, Box::new(right));
        }
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_bitwise_or_expr()?;
        while self.peek() == &Token::AmpAmp {
            self.advance();
            let right = self.parse_bitwise_or_expr()?;
            left = Expr::Binary(Box::new(left), BinaryOp::And, Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitwise_or_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_bitwise_xor_expr()?;
        while self.peek() == &Token::Pipe {
            self.advance();
            let right = self.parse_bitwise_xor_expr()?;
            left = Expr::Binary(Box::new(left), BinaryOp::BitwiseOr, Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitwise_xor_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_bitwise_and_expr()?;
        while self.peek() == &Token::Caret {
            self.advance();
            let right = self.parse_bitwise_and_expr()?;
            left = Expr::Binary(Box::new(left), BinaryOp::BitwiseXor, Box::new(right));
        }
        Ok(left)
    }

    fn parse_bitwise_and_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_equality_expr()?;
        while self.peek() == &Token::Ampersand {
            self.advance();
            let right = self.parse_equality_expr()?;
            left = Expr::Binary(Box::new(left), BinaryOp::BitwiseAnd, Box::new(right));
        }
        Ok(left)
    }

    fn parse_equality_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_comparison_expr()?;
        loop {
            let op = match self.peek() {
                Token::EqualEqual => BinaryOp::Equal,
                Token::BangEqual => BinaryOp::NotEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison_expr()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_comparison_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_shift_expr()?;
        loop {
            let op = match self.peek() {
                Token::Less => BinaryOp::Less,
                Token::LessEqual => BinaryOp::LessEqual,
                Token::Greater => BinaryOp::Greater,
                Token::GreaterEqual => BinaryOp::GreaterEqual,
                _ => break,
            };
            self.advance();
            let right = self.parse_shift_expr()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_shift_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_additive_expr()?;
        loop {
            let op = match self.peek() {
                Token::ShiftLeft => BinaryOp::ShiftLeft,
                Token::ShiftRight => BinaryOp::ShiftRight,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive_expr()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_additive_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative_expr()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative_expr()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_multiplicative_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary_expr()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinaryOp::Mul,
                Token::Slash => BinaryOp::Div,
                Token::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary_expr()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Token::Minus => {
                self.advance();
                let expr = self.parse_unary_expr()?;
                Ok(Expr::Unary(UnaryOp::Negate, Box::new(expr)))
            }
            Token::Bang => {
                self.advance();
                let expr = self.parse_unary_expr()?;
                Ok(Expr::Unary(UnaryOp::Not, Box::new(expr)))
            }
            Token::Ampersand => {
                self.advance();
                let expr = self.parse_unary_expr()?;
                Ok(Expr::Unary(UnaryOp::AddressOf, Box::new(expr)))
            }
            Token::Star => {
                self.advance();
                let expr = self.parse_unary_expr()?;
                Ok(Expr::Unary(UnaryOp::Deref, Box::new(expr)))
            }
            _ => self.parse_postfix_expr(),
        }
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            match self.peek() {
                Token::Dot => {
                    self.advance();
                    let field = self.expect_ident()?;
                    expr = Expr::MemberAccess(Box::new(expr), field);
                }
                Token::LeftBracket => {
                    self.advance();
                    let index = self.parse_expression()?;
                    self.expect(&Token::RightBracket)?;
                    expr = Expr::IndexAccess(Box::new(expr), Box::new(index));
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_primary_expr(&mut self) -> Result<Expr, ParseError> {
        let (line, col) = self.current_position();
        match self.peek().clone() {
            Token::FloatLiteral(v) => {
                self.advance();
                Ok(Expr::Literal(Literal::Float(v)))
            }
            Token::IntLiteral(v) => {
                self.advance();
                Ok(Expr::Literal(Literal::Int(v)))
            }
            Token::UIntLiteral(v) => {
                self.advance();
                Ok(Expr::Literal(Literal::UInt(v)))
            }
            Token::True => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(true)))
            }
            Token::False => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(false)))
            }
            Token::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(&Token::RightParen)?;
                Ok(expr)
            }
            Token::Ident(name) => {
                self.advance();

                // Check if this is a type constructor: vec3<f32>(...)
                if is_type_constructor_name(&name) && self.peek() == &Token::Less {
                    let ty = self.resolve_type_name(&name, line, col)?;
                    if self.peek() == &Token::LeftParen {
                        self.advance();
                        let args = self.parse_arg_list()?;
                        return Ok(Expr::TypeConstructor(ty, args));
                    }
                }

                // Check if this is a function call or type constructor without generic
                if self.peek() == &Token::LeftParen {
                    self.advance();
                    let args = self.parse_arg_list()?;
                    // Check if it could be a type constructor
                    if is_type_constructor_name(&name) {
                        let ty = self.resolve_type_name(&name, line, col)?;
                        Ok(Expr::TypeConstructor(ty, args))
                    } else {
                        Ok(Expr::Call(name, args))
                    }
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            other => Err(ParseError::unexpected_token(
                &other,
                "expression",
                line,
                col,
            )),
        }
    }

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        while self.peek() != &Token::RightParen {
            args.push(self.parse_expression()?);
            if self.peek() == &Token::Comma {
                self.advance();
            }
        }
        self.expect(&Token::RightParen)?;
        Ok(args)
    }
}

/// Checks if an identifier looks like a WGSL type constructor name.
fn is_type_constructor_name(name: &str) -> bool {
    name.starts_with("vec")
        || name.starts_with("mat")
        || name == "array"
        || name == "atomic"
        || name == "bitcast"
}

/// Convenience function to parse WGSL source into a module.
pub fn parse_wgsl(source: &str) -> Result<WgslModule, ParseError> {
    let mut parser = WgslParser::new(source)?;
    parser.parse_module()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_function() {
        let source = r#"
            fn add_one(value: f32) -> f32 {
                return value + 1.0;
            }
        "#;
        let module = parse_wgsl(source).unwrap();
        assert_eq!(module.functions.len(), 1);
        let func = &module.functions[0];
        assert_eq!(func.name, "add_one");
        assert_eq!(func.parameters.len(), 1);
        assert_eq!(func.parameters[0].name, "value");
        assert_eq!(func.parameters[0].ty, WgslType::Scalar(ScalarType::F32));
        assert_eq!(func.return_type, Some(WgslType::Scalar(ScalarType::F32)));
    }

    #[test]
    fn test_parse_function_with_vector_types() {
        let source = r#"
            fn transform(pos: vec2<f32>) -> vec4<f32> {
                return vec4<f32>(pos, 0.0, 1.0);
            }
        "#;
        let module = parse_wgsl(source).unwrap();
        let func = &module.functions[0];
        assert_eq!(func.parameters[0].ty, WgslType::Vector(ScalarType::F32, 2));
        assert_eq!(func.return_type, Some(WgslType::Vector(ScalarType::F32, 4)));
    }

    #[test]
    fn test_parse_struct() {
        let source = r#"
            struct Uniforms {
                scale: f32,
                offset: vec2<f32>,
            }
        "#;
        let module = parse_wgsl(source).unwrap();
        assert_eq!(module.structs.len(), 1);
        assert_eq!(module.structs[0].name, "Uniforms");
        assert_eq!(module.structs[0].fields.len(), 2);
        assert_eq!(module.structs[0].fields[0].name, "scale");
    }

    #[test]
    fn test_parse_global_var() {
        let source = r#"
            @group(0) @binding(0) var<uniform> my_uniform: Uniforms;
        "#;
        let module = parse_wgsl(source).unwrap();
        assert_eq!(module.globals.len(), 1);
        assert_eq!(module.globals[0].name, "my_uniform");
        assert_eq!(module.globals[0].address_space, AddressSpace::Uniform);
        assert!(module.globals[0].attributes.contains(&Attribute::Group(0)));
        assert!(
            module.globals[0]
                .attributes
                .contains(&Attribute::Binding(0))
        );
    }

    #[test]
    fn test_parse_if_else() {
        let source = r#"
            fn clamp_val(x: f32) -> f32 {
                if (x > 1.0) {
                    return 1.0;
                } else {
                    return x;
                }
            }
        "#;
        let module = parse_wgsl(source).unwrap();
        assert_eq!(module.functions.len(), 1);
        let body = &module.functions[0].body;
        assert_eq!(body.statements.len(), 1);
        assert!(matches!(body.statements[0], Statement::If { .. }));
    }

    #[test]
    fn test_parse_let_var() {
        let source = r#"
            fn test() -> f32 {
                let x: f32 = 1.0;
                var y = x + 2.0;
                return y;
            }
        "#;
        let module = parse_wgsl(source).unwrap();
        let body = &module.functions[0].body;
        assert_eq!(body.statements.len(), 3);
        match &body.statements[0] {
            Statement::Let {
                name, mutable, ty, ..
            } => {
                assert_eq!(name, "x");
                assert!(!mutable);
                assert_eq!(*ty, Some(WgslType::Scalar(ScalarType::F32)));
            }
            _ => panic!("expected Let"),
        }
        match &body.statements[1] {
            Statement::Let { name, mutable, .. } => {
                assert_eq!(name, "y");
                assert!(mutable);
            }
            _ => panic!("expected Var"),
        }
    }

    #[test]
    fn test_parse_binary_expressions() {
        let source = r#"
            fn calc(a: f32, b: f32) -> f32 {
                return a * b + 1.0;
            }
        "#;
        let module = parse_wgsl(source).unwrap();
        let body = &module.functions[0].body;
        // return (a * b) + 1.0
        match &body.statements[0] {
            Statement::Return(Some(Expr::Binary(_, BinaryOp::Add, _))) => {}
            other => panic!("expected binary add, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_function_call() {
        let source = r#"
            fn test(x: f32) -> f32 {
                return clamp(x, 0.0, 1.0);
            }
        "#;
        let module = parse_wgsl(source).unwrap();
        let body = &module.functions[0].body;
        match &body.statements[0] {
            Statement::Return(Some(Expr::Call(name, args))) => {
                assert_eq!(name, "clamp");
                assert_eq!(args.len(), 3);
            }
            other => panic!("expected function call, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_member_access() {
        let source = r#"
            fn test(u: Uniforms) -> f32 {
                return u.scale;
            }
        "#;
        let module = parse_wgsl(source).unwrap();
        let body = &module.functions[0].body;
        match &body.statements[0] {
            Statement::Return(Some(Expr::MemberAccess(_, field))) => {
                assert_eq!(field, "scale");
            }
            other => panic!("expected member access, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_struct_with_attributes() {
        let source = r#"
            struct VertexOutput {
                @builtin(position) clip_position: vec4<f32>,
                @location(0) color: vec4<f32>,
            }
        "#;
        let module = parse_wgsl(source).unwrap();
        let s = &module.structs[0];
        assert_eq!(
            s.fields[0].attributes,
            vec![Attribute::Builtin("position".to_string())]
        );
        assert_eq!(s.fields[1].attributes, vec![Attribute::Location(0)]);
    }

    #[test]
    fn test_parse_vertex_function() {
        let source = r#"
            @vertex
            fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
                var output: VertexOutput;
                return output;
            }
        "#;
        let module = parse_wgsl(source).unwrap();
        let func = &module.functions[0];
        assert!(func.attributes.contains(&Attribute::Vertex));
        assert_eq!(func.name, "vs_main");
    }

    #[test]
    fn test_parse_error_reports_location() {
        let source = "fn test( { }";
        let result = parse_wgsl(source);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.line > 0);
        assert!(err.col > 0);
    }
}
