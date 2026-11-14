//! C code generator for Rune.
//!
//! Produces readable C99 that can be compiled with gcc/clang.

use crate::language::ast::*;
use crate::language::token::TokenKind;
use std::fmt::Write;

pub struct CGenerator {
    output: String,
    indent: usize,
}

impl Default for CGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CGenerator {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    pub fn generate(mut self, program: &Program) -> String {
        self.emit_headers();
        // Forward declarations so functions can call each other regardless
        // of the order they appear in the source.
        for func in &program.functions {
            let mut prototype = self.signature(func);
            prototype.push(';');
            self.line(&prototype);
        }
        self.line("");
        for func in &program.functions {
            self.emit_function(func);
        }
        self.output
    }

    fn emit_headers(&mut self) {
        self.line("#include <stdio.h>");
        self.line("#include <stdint.h>");
        self.line("#include <stdbool.h>");
        self.line("#include \"runtime/rune_rt.h\"");
        self.line("");
    }

    /// Builds the C signature (return type, name, parameters) for a function.
    fn signature(&self, func: &Function) -> String {
        let ret = if func.name == "main" {
            // C requires `main` to return `int`
            "int"
        } else {
            match &func.return_type {
                Some(Type::Int) => "int64_t",
                Some(Type::Bool) => "bool",
                Some(Type::String) => "const char*",
                Some(Type::Void) | None => "void",
            }
        };

        let mut sig = format!("{} {}(", ret, func.name);

        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                sig.push_str(", ");
            }
            let ty = match param.ty {
                Type::Int => "int64_t",
                Type::Bool => "bool",
                Type::String => "const char*",
                Type::Void => "void",
            };
            sig.push_str(&format!("{} {}", ty, param.name));
        }

        sig.push(')');
        sig
    }

    fn emit_function(&mut self, func: &Function) {
        let sig = self.signature(func);
        self.output.push_str(&sig);
        self.output.push_str(" {\n");
        self.indent += 1;

        self.emit_block(&func.body);

        self.indent -= 1;
        if func.name == "main" {
            self.indent_write();
            self.output.push_str("return 0;\n");
        }
        self.line("}");
        self.line("");
    }

    fn emit_block(&mut self, block: &Block) {
        for stmt in &block.statements {
            self.emit_stmt(stmt);
        }
    }

    fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, value, .. } => {
                self.indent_write();
                write!(self.output, "int64_t {} = ", name).unwrap();
                self.emit_expr(value);
                self.output.push_str(";\n");
            }
            Stmt::Assign { name, value, .. } => {
                self.indent_write();
                write!(self.output, "{} = ", name).unwrap();
                self.emit_expr(value);
                self.output.push_str(";\n");
            }
            Stmt::Expr(expr) => {
                self.indent_write();
                self.emit_expr(expr);
                self.output.push_str(";\n");
            }
            Stmt::Return { value, .. } => {
                self.indent_write();
                self.output.push_str("return");
                if let Some(v) = value {
                    self.output.push(' ');
                    self.emit_expr(v);
                }
                self.output.push_str(";\n");
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.indent_write();
                self.output.push_str("if (");
                self.emit_expr(condition);
                self.output.push_str(") {\n");
                self.indent += 1;
                self.emit_block(then_branch);
                self.indent -= 1;
                self.indent_write();
                self.output.push('}');
                if let Some(else_b) = else_branch {
                    self.output.push_str(" else {\n");
                    self.indent += 1;
                    self.emit_block(else_b);
                    self.indent -= 1;
                    self.indent_write();
                    self.output.push('}');
                }
                self.output.push('\n');
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.indent_write();
                self.output.push_str("while (");
                self.emit_expr(condition);
                self.output.push_str(") {\n");
                self.indent += 1;
                self.emit_block(body);
                self.indent -= 1;
                self.indent_write();
                self.output.push_str("}\n");
            }
            Stmt::Block(block) => {
                self.indent_write();
                self.output.push_str("{\n");
                self.indent += 1;
                self.emit_block(block);
                self.indent -= 1;
                self.indent_write();
                self.output.push_str("}\n");
            }
        }
    }

    fn emit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Int { value, .. } => write!(self.output, "{}", value).unwrap(),
            Expr::Bool { value, .. } => self.output.push_str(if *value { "true" } else { "false" }),
            Expr::String { value, .. } => write!(self.output, "\"{}\"", value).unwrap(),
            Expr::Variable { name, .. } => self.output.push_str(name),
            Expr::Binary {
                left, op, right, ..
            } => {
                self.output.push('(');
                self.emit_expr(left);
                self.output.push(' ');
                self.output.push_str(Self::op_to_c(op));
                self.output.push(' ');
                self.emit_expr(right);
                self.output.push(')');
            }
            Expr::Unary { op, expr, .. } => {
                self.output.push_str(Self::op_to_c(op));
                self.emit_expr(expr);
            }
            Expr::Call { callee, args, .. } => {
                if callee == "print" {
                    // Special handling for print
                    self.output.push_str("rune_print_int(");
                    if let Some(arg) = args.first() {
                        self.emit_expr(arg);
                    }
                    self.output.push(')');
                } else {
                    write!(self.output, "{}(", callee).unwrap();
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.emit_expr(arg);
                    }
                    self.output.push(')');
                }
            }
            Expr::Grouping { expr, .. } => {
                self.output.push('(');
                self.emit_expr(expr);
                self.output.push(')');
            }
        }
    }

    fn line(&mut self, text: &str) {
        self.indent_write();
        self.output.push_str(text);
        self.output.push('\n');
    }

    fn indent_write(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    fn op_to_c(op: &TokenKind) -> &'static str {
        match op {
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::EqEq => "==",
            TokenKind::BangEq => "!=",
            TokenKind::Less => "<",
            TokenKind::LessEq => "<=",
            TokenKind::Greater => ">",
            TokenKind::GreaterEq => ">=",
            TokenKind::AmpAmp => "&&",
            TokenKind::PipePipe => "||",
            TokenKind::Bang => "!",
            _ => "/* unknown op */",
        }
    }
}
