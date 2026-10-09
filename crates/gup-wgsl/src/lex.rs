// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A WGSL token scanner: just enough to find identifiers, `::` paths,
//! braces and module-scope declarations. It never interprets expressions.

use std::ops::Range;

/// One token and its byte range in the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tok<'a> {
    /// An identifier or keyword.
    Ident(&'a str),
    /// A numeric literal.
    Number(&'a str),
    /// `::`.
    PathSep,
    /// Any other single character (`{`, `;`, `.`, `<`, …).
    Punct(char),
    /// Whitespace or a comment.
    Trivia,
    /// A preprocessor line (`#import …`, `#define_import_path …`), without
    /// its line break.
    Directive(&'a str),
}

/// Split `src` into tokens. Line and (nested) block comments are trivia; a
/// `#` that starts a line, after optional indentation, runs to the end of
/// that line.
pub(crate) fn lex(src: &str) -> Vec<(Range<usize>, Tok<'_>)> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line_start = true;
    while i < bytes.len() {
        let start = i;
        let c = bytes[i];
        let tok = if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            Tok::Trivia
        } else if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let mut depth = 0;
            while i < bytes.len() {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            Tok::Trivia
        } else if c.is_ascii_whitespace() {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            Tok::Trivia
        } else if c == b'#' && line_start {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            Tok::Directive(src[start..i].trim_end())
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            Tok::Ident(&src[start..i])
        } else if c.is_ascii_digit() {
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
            {
                i += 1;
            }
            Tok::Number(&src[start..i])
        } else if c == b':' && bytes.get(i + 1) == Some(&b':') {
            i += 2;
            Tok::PathSep
        } else {
            let ch = src[i..].chars().next().expect("in bounds");
            i += ch.len_utf8();
            Tok::Punct(ch)
        };
        // A directive must start its line: only whitespace since the last
        // line break (comments end at one).
        line_start = match tok {
            Tok::Trivia => line_start || src[start..i].contains('\n'),
            _ => false,
        };
        out.push((start..i, tok));
    }
    out
}

/// A module-scope declaration in WGSL text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Decl<'a> {
    /// The declared name, or `None` for a directive-like statement
    /// (`enable`, `requires`, `diagnostic`, `const_assert`).
    pub name: Option<&'a str>,
    /// The declaration's text, from its first attribute or keyword to its
    /// closing `}` or `;`.
    pub text: &'a str,
}

/// The module-scope declarations of `src`, in order. Preprocessor lines
/// are skipped. Declarations end at a `;` outside braces, or at the `}`
/// that closes their body.
pub(crate) fn declarations(src: &str) -> Vec<Decl<'_>> {
    let toks: Vec<_> = lex(src)
        .into_iter()
        .filter(|(_, t)| !matches!(t, Tok::Trivia | Tok::Directive(_)))
        .collect();
    let mut decls = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let start = toks[i].0.start;
        let mut depth = 0usize;
        let mut name = None;
        let mut j = i;
        let mut expect_name = false;
        let mut in_var_template = 0usize;
        loop {
            let Some((range, tok)) = toks.get(j) else {
                // Unterminated: take the rest.
                decls.push(Decl {
                    name,
                    text: &src[start..],
                });
                return decls;
            };
            match *tok {
                Tok::Punct('{') => depth += 1,
                Tok::Punct('}') => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        decls.push(Decl {
                            name,
                            text: &src[start..range.end],
                        });
                        break;
                    }
                }
                Tok::Punct(';') if depth == 0 => {
                    decls.push(Decl {
                        name,
                        text: &src[start..range.end],
                    });
                    break;
                }
                Tok::Punct('<') if expect_name && in_var_template > 0 => in_var_template += 1,
                Tok::Punct('>') if in_var_template > 1 => {
                    in_var_template -= 1;
                }
                Tok::Ident(word) if depth == 0 && name.is_none() => {
                    if expect_name && in_var_template <= 1 {
                        name = Some(word);
                        expect_name = false;
                    } else if !expect_name {
                        match word {
                            "struct" | "fn" | "const" | "override" | "alias" => expect_name = true,
                            "var" => {
                                expect_name = true;
                                in_var_template = 1;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            j += 1;
        }
        i = j + 1;
    }
    decls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_find_names_bodies_and_attributes() {
        let src = "#define_import_path a::b\n\
                   // comment with fn fake() {}\n\
                   struct P { x: f32, }\n\
                   @group(0) @binding(0)\nvar<uniform> u: P;\n\
                   var<storage, read_write> s: array<vec4<f32>>;\n\
                   const k: f32 = 1.0;\n\
                   fn f(p: P) -> f32 { if (true) { return p.x; } return k; }\n\
                   alias T = vec2<f32>;\n";
        let decls = declarations(src);
        let names: Vec<_> = decls.iter().map(|d| d.name.unwrap()).collect();
        assert_eq!(names, ["P", "u", "s", "k", "f", "T"]);
        assert_eq!(decls[1].text, "@group(0) @binding(0)\nvar<uniform> u: P;");
        assert!(decls[4].text.ends_with("return k; }"));
    }

    #[test]
    fn directives_start_lines_only() {
        let toks = lex("  #import a::b as c\nx # y");
        assert_eq!(toks[1].1, Tok::Directive("#import a::b as c"));
        assert!(toks.iter().any(|(_, t)| *t == Tok::Punct('#')));
    }
}
