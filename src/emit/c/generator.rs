// SPDX-License-Identifier: MIT

//! C code generator for Rune.
//!
//! Produces readable C99 that can be compiled with gcc/clang.

use crate::language::ast::*;
use crate::language::token::TokenKind;
use std::collections::HashMap;
use std::fmt::Write;

pub struct CGenerator {
    output: String,
    indent: usize,
    /// Return types of declared functions, used to type call expressions.
    function_returns: HashMap<String, Type>,
    /// Lexically scoped variable types, maintained while emitting so that
    /// declarations and `print` calls pick the right C types.
    scopes: Vec<HashMap<String, Type>>,
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
            function_returns: HashMap::new(),
            scopes: Vec::new(),
        }
    }

    pub fn generate(mut self, program: &Program) -> String {
        for func in &program.functions {
            self.function_returns.insert(
                func.name.clone(),
                func.return_type.clone().unwrap_or(Type::Void),
            );
        }
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
            sig.push_str(&format!("{} {}", Self::c_type_name(&param.ty), param.name));
        }

        sig.push(')');
        sig
    }

    fn emit_function(&mut self, func: &Function) {
        let sig = self.signature(func);
        self.output.push_str(&sig);
        self.output.push_str(" {\n");
        self.indent += 1;

        self.enter_scope();
        for param in &func.params {
            self.declare_var(&param.name, param.ty.clone());
        }
        self.emit_block(&func.body);
        self.exit_scope();

        self.indent -= 1;
        if func.name == "main" {
            self.indent_write();
            self.output.push_str("return 0;\n");
        }
        self.line("}");
        self.line("");
    }

    fn emit_block(&mut self, block: &Block) {
        self.enter_scope();
        for stmt in &block.statements {
            self.emit_stmt(stmt);
        }
        self.exit_scope();
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_var(&mut self, name: &str, ty: Type) {
        if let Some(top) = self.scopes.last_mut() {
            top.insert(name.to_string(), ty);
        }
    }

    fn var_type(&self, name: &str) -> Option<Type> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    /// Maps a Rune type to its C equivalent.
    fn c_type_name(ty: &Type) -> &'static str {
        match ty {
            Type::Int => "int64_t",
            Type::Bool => "bool",
            Type::String => "const char*",
            Type::Void => "void",
        }
    }

    /// Infers the static type of an expression during emission. The checker
    /// guarantees the program is well-typed, so the fallbacks here only guard
    /// against internal mistakes.
    fn infer_type(&self, expr: &Expr) -> Type {
        match expr {
            Expr::Int { .. } => Type::Int,
            Expr::Bool { .. } => Type::Bool,
            Expr::String { .. } => Type::String,
            Expr::Variable { name, .. } => self.var_type(name).unwrap_or(Type::Int),
            Expr::Grouping { expr, .. } => self.infer_type(expr),
            Expr::Unary { op, .. } => match op {
                TokenKind::Minus => Type::Int,
                _ => Type::Bool,
            },
            Expr::Binary { op, .. } => match op {
                TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Percent => Type::Int,
                _ => Type::Bool,
            },
            Expr::Call { callee, .. } => {
                if callee == "print" {
                    Type::Void
                } else {
                    self.function_returns
                        .get(callee)
                        .cloned()
                        .unwrap_or(Type::Int)
                }
            }
        }
    }

    fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let ty = self.infer_type(value);
                self.declare_var(name, ty.clone());
                self.indent_write();
                write!(self.output, "{} {} = ", Self::c_type_name(&ty), name).unwrap();
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
                    // Dispatch on the argument's static type.
                    let runtime_fn = match args.first().map(|a| self.infer_type(a)) {
                        Some(Type::Bool) => "rune_print_bool",
                        Some(Type::String) => "rune_print_string",
                        _ => "rune_print_int",
                    };
                    self.output.push_str(runtime_fn);
                    self.output.push('(');
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
