// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WGSL code generator — converts the transpilation AST into WGSL text.

use std::fmt::Write;

use super::ast::*;

/// WGSL code generator configuration.
#[derive(Debug, Clone)]
pub struct WgslCodeGenConfig {
    /// Number of spaces per indentation level.
    pub indent_spaces: usize,
}

impl Default for WgslCodeGenConfig {
    fn default() -> Self {
        Self { indent_spaces: 4 }
    }
}

/// Generates WGSL source text from AST nodes.
pub struct WgslCodeGen {
    config: WgslCodeGenConfig,
    output: String,
    indent_level: usize,
}

impl WgslCodeGen {
    /// Create a new code generator with default configuration.
    pub fn new() -> Self {
        Self {
            config: WgslCodeGenConfig::default(),
            output: String::new(),
            indent_level: 0,
        }
    }

    /// Create a new code generator with custom configuration.
    pub fn with_config(config: WgslCodeGenConfig) -> Self {
        Self {
            config,
            output: String::new(),
            indent_level: 0,
        }
    }

    /// Generate WGSL text for a complete module.
    pub fn generate_module(&mut self, module: &WgslModule) -> String {
        self.output.clear();

        for (i, s) in module.structs.iter().enumerate() {
            self.generate_struct(s);
            if i + 1 < module.structs.len() || !module.functions.is_empty() {
                self.write_line("");
            }
        }

        for (i, f) in module.functions.iter().enumerate() {
            self.generate_function(f);
            if i + 1 < module.functions.len() {
                self.write_line("");
            }
        }

        self.output.clone()
    }

    /// Generate WGSL text for a single function.
    pub fn generate_function(&mut self, func: &WgslFunction) -> String {
        let start = self.output.len();

        // Function signature
        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ty))
            .collect();

        let ret = if func.return_type == WgslType::Void {
            String::new()
        } else {
            format!(" -> {}", func.return_type)
        };

        self.write_line(&format!(
            "fn {}({}){} {{",
            func.name,
            params.join(", "),
            ret
        ));
        self.indent_level += 1;

        for stmt in &func.body {
            self.generate_stmt(stmt);
        }

        self.indent_level -= 1;
        self.write_line("}");

        self.output[start..].to_string()
    }

    /// Generate WGSL text for a struct definition.
    fn generate_struct(&mut self, s: &WgslStructDef) {
        self.write_line(&format!("struct {} {{", s.name));
        self.indent_level += 1;

        for (i, field) in s.fields.iter().enumerate() {
            let comma = if i + 1 < s.fields.len() { "," } else { "," };
            self.write_line(&format!("{}: {}{}", field.name, field.ty, comma));
        }

        self.indent_level -= 1;
        self.write_line("}");
    }

    /// Generate WGSL text for a statement.
    fn generate_stmt(&mut self, stmt: &WgslStatement) {
        match stmt {
            WgslStatement::Let {
                name,
                ty,
                value,
                mutable,
            } => {
                let keyword = if *mutable { "var" } else { "let" };
                let type_ann = ty.as_ref().map(|t| format!(": {t}")).unwrap_or_default();
                let expr = self.generate_expr(value);
                self.write_line(&format!("{keyword} {name}{type_ann} = {expr};"));
            }
            WgslStatement::Return(expr) => {
                if let Some(e) = expr {
                    let expr = self.generate_expr(e);
                    self.write_line(&format!("return {expr};"));
                } else {
                    self.write_line("return;");
                }
            }
            WgslStatement::If {
                condition,
                body,
                else_body,
            } => {
                let cond = self.generate_expr(condition);
                self.write_line(&format!("if ({cond}) {{"));
                self.indent_level += 1;
                for s in body {
                    self.generate_stmt(s);
                }
                self.indent_level -= 1;

                if let Some(else_stmts) = else_body {
                    self.write_line("} else {");
                    self.indent_level += 1;
                    for s in else_stmts {
                        self.generate_stmt(s);
                    }
                    self.indent_level -= 1;
                }
                self.write_line("}");
            }
            WgslStatement::Expression(e) => {
                let expr = self.generate_expr(e);
                self.write_line(&format!("{expr};"));
            }
            WgslStatement::Assign(target, value) => {
                let t = self.generate_expr(target);
                let v = self.generate_expr(value);
                self.write_line(&format!("{t} = {v};"));
            }
        }
    }

    /// Generate WGSL text for an expression.
    pub fn generate_expr(&self, expr: &WgslExpr) -> String {
        match expr {
            WgslExpr::Literal(lit) => self.generate_literal(lit),
            WgslExpr::Ident(name) => name.clone(),
            WgslExpr::Binary(left, op, right) => {
                let l = self.generate_expr(left);
                let r = self.generate_expr(right);
                format!("{l} {op} {r}")
            }
            WgslExpr::Unary(op, operand) => {
                let inner = self.generate_expr(operand);
                format!("{op}{inner}")
            }
            WgslExpr::Call(name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| self.generate_expr(a)).collect();
                format!("{name}({})", args_str.join(", "))
            }
            WgslExpr::TypeConstructor(ty, args) => {
                let args_str: Vec<String> = args.iter().map(|a| self.generate_expr(a)).collect();
                format!("{ty}({})", args_str.join(", "))
            }
            WgslExpr::MemberAccess(base, field) => {
                let base_str = self.generate_expr(base);
                format!("{base_str}.{field}")
            }
            WgslExpr::IndexAccess(base, index) => {
                let base_str = self.generate_expr(base);
                let idx_str = self.generate_expr(index);
                format!("{base_str}[{idx_str}]")
            }
            WgslExpr::Paren(inner) => {
                let inner_str = self.generate_expr(inner);
                format!("({inner_str})")
            }
            WgslExpr::Cast(ty, inner) => {
                let inner_str = self.generate_expr(inner);
                format!("{ty}({inner_str})")
            }
        }
    }

    /// Generate WGSL text for a literal value.
    fn generate_literal(&self, lit: &Literal) -> String {
        match lit {
            Literal::Float(v) => {
                let s = format!("{v}");
                // Ensure there's a decimal point for WGSL float literals
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    s
                } else {
                    format!("{s}.0")
                }
            }
            Literal::Int(v) => format!("{v}"),
            Literal::UInt(v) => format!("{v}u"),
            Literal::Bool(v) => format!("{v}"),
        }
    }

    /// Write a line at the current indentation level.
    fn write_line(&mut self, line: &str) {
        let indent = " ".repeat(self.indent_level * self.config.indent_spaces);
        let _ = writeln!(self.output, "{indent}{line}");
    }
}

impl Default for WgslCodeGen {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Convenience functions
// ---------------------------------------------------------------------------

/// Generate WGSL text for a single function.
pub fn generate_function_wgsl(func: &WgslFunction) -> String {
    let mut codegen = WgslCodeGen::new();
    codegen.generate_function(func)
}

/// Generate WGSL text for a complete module.
pub fn generate_module_wgsl(module: &WgslModule) -> String {
    let mut codegen = WgslCodeGen::new();
    codegen.generate_module(module)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_simple_function() {
        let func = WgslFunction {
            name: "add_one".to_string(),
            params: vec![WgslParam {
                name: "x".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
            }],
            return_type: WgslType::Scalar(ScalarType::F32),
            body: vec![WgslStatement::Return(Some(WgslExpr::Binary(
                Box::new(WgslExpr::Ident("x".to_string())),
                BinaryOp::Add,
                Box::new(WgslExpr::Literal(Literal::Float(1.0))),
            )))],
        };

        let wgsl = generate_function_wgsl(&func);
        assert!(wgsl.contains("fn add_one(x: f32) -> f32"));
        assert!(wgsl.contains("return x + 1.0;"));
    }

    #[test]
    fn generate_function_with_uniforms() {
        let func = WgslFunction {
            name: "scale".to_string(),
            params: vec![
                WgslParam {
                    name: "value".to_string(),
                    ty: WgslType::Scalar(ScalarType::F32),
                },
                WgslParam {
                    name: "uniforms".to_string(),
                    ty: WgslType::Struct("ScaleUniforms".to_string()),
                },
            ],
            return_type: WgslType::Scalar(ScalarType::F32),
            body: vec![WgslStatement::Return(Some(WgslExpr::Binary(
                Box::new(WgslExpr::Ident("value".to_string())),
                BinaryOp::Mul,
                Box::new(WgslExpr::MemberAccess(
                    Box::new(WgslExpr::Ident("uniforms".to_string())),
                    "factor".to_string(),
                )),
            )))],
        };

        let wgsl = generate_function_wgsl(&func);
        assert!(wgsl.contains("fn scale(value: f32, uniforms: ScaleUniforms) -> f32"));
        assert!(wgsl.contains("return value * uniforms.factor;"));
    }

    #[test]
    fn generate_let_binding() {
        let func = WgslFunction {
            name: "test".to_string(),
            params: vec![WgslParam {
                name: "x".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
            }],
            return_type: WgslType::Scalar(ScalarType::F32),
            body: vec![
                WgslStatement::Let {
                    name: "doubled".to_string(),
                    ty: None,
                    value: WgslExpr::Binary(
                        Box::new(WgslExpr::Ident("x".to_string())),
                        BinaryOp::Mul,
                        Box::new(WgslExpr::Literal(Literal::Float(2.0))),
                    ),
                    mutable: false,
                },
                WgslStatement::Return(Some(WgslExpr::Ident("doubled".to_string()))),
            ],
        };

        let wgsl = generate_function_wgsl(&func);
        assert!(wgsl.contains("let doubled = x * 2.0;"));
        assert!(wgsl.contains("return doubled;"));
    }

    #[test]
    fn generate_var_binding() {
        let stmt = WgslStatement::Let {
            name: "counter".to_string(),
            ty: Some(WgslType::Scalar(ScalarType::I32)),
            value: WgslExpr::Literal(Literal::Int(0)),
            mutable: true,
        };

        let func = WgslFunction {
            name: "test".to_string(),
            params: vec![],
            return_type: WgslType::Void,
            body: vec![stmt],
        };

        let wgsl = generate_function_wgsl(&func);
        assert!(wgsl.contains("var counter: i32 = 0;"));
    }

    #[test]
    fn generate_if_else() {
        let func = WgslFunction {
            name: "clamp_manual".to_string(),
            params: vec![WgslParam {
                name: "x".to_string(),
                ty: WgslType::Scalar(ScalarType::F32),
            }],
            return_type: WgslType::Scalar(ScalarType::F32),
            body: vec![WgslStatement::If {
                condition: WgslExpr::Binary(
                    Box::new(WgslExpr::Ident("x".to_string())),
                    BinaryOp::Greater,
                    Box::new(WgslExpr::Literal(Literal::Float(1.0))),
                ),
                body: vec![WgslStatement::Return(Some(WgslExpr::Literal(
                    Literal::Float(1.0),
                )))],
                else_body: Some(vec![WgslStatement::Return(Some(WgslExpr::Ident(
                    "x".to_string(),
                )))]),
            }],
        };

        let wgsl = generate_function_wgsl(&func);
        assert!(wgsl.contains("if (x > 1.0)"));
        assert!(wgsl.contains("} else {"));
    }

    #[test]
    fn generate_type_constructor() {
        let codegen = WgslCodeGen::new();
        let expr = WgslExpr::TypeConstructor(
            WgslType::Vector(ScalarType::F32, 3),
            vec![
                WgslExpr::Literal(Literal::Float(1.0)),
                WgslExpr::Literal(Literal::Float(2.0)),
                WgslExpr::Literal(Literal::Float(3.0)),
            ],
        );
        assert_eq!(codegen.generate_expr(&expr), "vec3<f32>(1.0, 2.0, 3.0)");
    }

    #[test]
    fn generate_cast() {
        let codegen = WgslCodeGen::new();
        let expr = WgslExpr::Cast(
            WgslType::Scalar(ScalarType::F32),
            Box::new(WgslExpr::Ident("x".to_string())),
        );
        assert_eq!(codegen.generate_expr(&expr), "f32(x)");
    }

    #[test]
    fn generate_index_access() {
        let codegen = WgslCodeGen::new();
        let expr = WgslExpr::IndexAccess(
            Box::new(WgslExpr::Ident("arr".to_string())),
            Box::new(WgslExpr::Literal(Literal::Int(0))),
        );
        assert_eq!(codegen.generate_expr(&expr), "arr[0]");
    }

    #[test]
    fn generate_struct_def() {
        let module = WgslModule {
            structs: vec![WgslStructDef {
                name: "Uniforms".to_string(),
                fields: vec![
                    WgslField {
                        name: "scale".to_string(),
                        ty: WgslType::Scalar(ScalarType::F32),
                    },
                    WgslField {
                        name: "offset".to_string(),
                        ty: WgslType::Vector(ScalarType::F32, 2),
                    },
                ],
            }],
            functions: vec![],
        };

        let wgsl = generate_module_wgsl(&module);
        assert!(wgsl.contains("struct Uniforms {"));
        assert!(wgsl.contains("scale: f32,"));
        assert!(wgsl.contains("offset: vec2<f32>,"));
    }

    #[test]
    fn generate_full_module() {
        let module = WgslModule {
            structs: vec![WgslStructDef {
                name: "ScaleUniforms".to_string(),
                fields: vec![WgslField {
                    name: "factor".to_string(),
                    ty: WgslType::Scalar(ScalarType::F32),
                }],
            }],
            functions: vec![WgslFunction {
                name: "apply_scale".to_string(),
                params: vec![
                    WgslParam {
                        name: "value".to_string(),
                        ty: WgslType::Scalar(ScalarType::F32),
                    },
                    WgslParam {
                        name: "uniforms".to_string(),
                        ty: WgslType::Struct("ScaleUniforms".to_string()),
                    },
                ],
                return_type: WgslType::Scalar(ScalarType::F32),
                body: vec![WgslStatement::Return(Some(WgslExpr::Binary(
                    Box::new(WgslExpr::Ident("value".to_string())),
                    BinaryOp::Mul,
                    Box::new(WgslExpr::MemberAccess(
                        Box::new(WgslExpr::Ident("uniforms".to_string())),
                        "factor".to_string(),
                    )),
                )))],
            }],
        };

        let wgsl = generate_module_wgsl(&module);
        assert!(wgsl.contains("struct ScaleUniforms"));
        assert!(wgsl.contains("fn apply_scale"));
        assert!(wgsl.contains("return value * uniforms.factor;"));
    }

    #[test]
    fn generate_unary_negate() {
        let codegen = WgslCodeGen::new();
        let expr = WgslExpr::Unary(UnaryOp::Negate, Box::new(WgslExpr::Ident("x".to_string())));
        assert_eq!(codegen.generate_expr(&expr), "-x");
    }

    #[test]
    fn generate_nested_member_access() {
        let codegen = WgslCodeGen::new();
        let expr = WgslExpr::MemberAccess(
            Box::new(WgslExpr::MemberAccess(
                Box::new(WgslExpr::Ident("uniforms".to_string())),
                "config".to_string(),
            )),
            "min_val".to_string(),
        );
        assert_eq!(codegen.generate_expr(&expr), "uniforms.config.min_val");
    }

    #[test]
    fn generate_float_without_decimal() {
        let codegen = WgslCodeGen::new();
        // Integer-valued float should still have .0
        let expr = WgslExpr::Literal(Literal::Float(42.0));
        let result = codegen.generate_expr(&expr);
        assert!(
            result.contains('.'),
            "Float '42' should render as '42.0' but got '{result}'"
        );
    }
}
