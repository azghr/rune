// SPDX-License-Identifier: MIT

//! Type checker and semantic analyzer.

use crate::diagnostics::diagnostic::Diagnostic;
use crate::language::analyze::scope::{ScopeStack, Symbol};
use crate::language::ast::*;
use crate::language::token::TokenKind;
use crate::shared::span::Span;
use std::collections::HashMap;

/// The signature of a declared function, collected before bodies are checked
/// so that recursion and forward references type-check.
#[derive(Debug, Clone)]
struct FunctionSignature {
    params: Vec<Type>,
    return_type: Type,
}

pub struct Analyzer {
    scopes: ScopeStack,
    current_return_type: Option<Type>,
    functions: HashMap<String, FunctionSignature>,
    errors: Vec<Diagnostic>,
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer {
    pub fn new() -> Self {
        Self {
            scopes: ScopeStack::new(),
            current_return_type: None,
            functions: HashMap::new(),
            errors: Vec::new(),
        }
    }

    pub fn analyze(mut self, program: Program) -> Result<Program, Vec<Diagnostic>> {
        // First pass: collect signatures so calls resolve regardless of
        // definition order.
        for func in &program.functions {
            let signature = FunctionSignature {
                params: func.params.iter().map(|p| p.ty.clone()).collect(),
                return_type: func.return_type.clone().unwrap_or(Type::Void),
            };
            if self
                .functions
                .insert(func.name.clone(), signature)
                .is_some()
            {
                self.error(
                    format!("function `{}` is already defined", func.name),
                    func.span,
                );
            }
        }

        for func in &program.functions {
            self.analyze_function(func);
        }

        if self.errors.is_empty() {
            Ok(program)
        } else {
            Err(self.errors)
        }
    }

    fn analyze_function(&mut self, func: &Function) {
        self.scopes.push();
        self.current_return_type = func.return_type.clone();

        for param in &func.params {
            let _ = self.scopes.define(Symbol {
                name: param.name.clone(),
                ty: param.ty.clone(),
                span: param.span,
                mutable: false,
            });
        }

        self.analyze_block(&func.body);
        self.scopes.pop();
        self.current_return_type = None;
    }

    fn analyze_block(&mut self, block: &Block) {
        self.scopes.push();
        for stmt in &block.statements {
            self.analyze_stmt(stmt);
        }
        self.scopes.pop();
    }

    fn analyze_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name,
                mutable,
                ty,
                value,
                span,
            } => {
                let value_ty = self.analyze_expr(value);
                if value_ty == Type::Void {
                    self.error("cannot bind a `void` value to a variable", *span);
                }
                let final_ty = if let Some(declared) = ty {
                    if *declared != value_ty {
                        self.error(
                            format!(
                                "type mismatch: expected `{}`, found `{}`",
                                type_name(declared),
                                type_name(&value_ty)
                            ),
                            *span,
                        );
                    }
                    declared.clone()
                } else {
                    value_ty
                };

                if let Err(msg) = self.scopes.define(Symbol {
                    name: name.clone(),
                    ty: final_ty,
                    span: *span,
                    mutable: *mutable,
                }) {
                    self.error(msg, *span);
                }
            }
            Stmt::Assign {
                name,
                op: TokenKind::Eq,
                value,
                span,
            } => {
                let sym = match self.scopes.resolve(name) {
                    Some(sym) => sym.clone(),
                    None => {
                        self.error(format!("undefined variable `{}`", name), *span);
                        return;
                    }
                };
                if !sym.mutable {
                    self.error(
                        format!("cannot assign to immutable variable `{}`", name),
                        *span,
                    );
                    return;
                }

                let value_ty = self.analyze_expr(value);
                if sym.ty != value_ty {
                    self.error(
                        format!(
                            "cannot assign `{}` to variable of type `{}`",
                            type_name(&value_ty),
                            type_name(&sym.ty)
                        ),
                        *span,
                    );
                }
            }
            Stmt::Assign {
                name,
                op: compound_op,
                value,
                span,
            } => {
                let sym = match self.scopes.resolve(name) {
                    Some(sym) => sym.clone(),
                    None => {
                        self.error(format!("undefined variable `{}`", name), *span);
                        return;
                    }
                };
                if !sym.mutable {
                    self.error(
                        format!("cannot assign to immutable variable `{}`", name),
                        *span,
                    );
                    return;
                }

                let value_ty = self.analyze_expr(value);
                // The result of `x <op> value` must be assignable back to x.
                let _ = match compound_op {
                    TokenKind::PlusEq if sym.ty == Type::String => {
                        if value_ty != Type::String {
                            self.error(
                                format!(
                                    "cannot concatenate `{}` onto a `string`",
                                    type_name(&value_ty)
                                ),
                                *span,
                            );
                        }
                        Type::String
                    }
                    TokenKind::PlusEq
                    | TokenKind::MinusEq
                    | TokenKind::StarEq
                    | TokenKind::SlashEq
                    | TokenKind::PercentEq => {
                        if sym.ty != Type::Int || value_ty != Type::Int {
                            self.error(
                                format!(
                                    "compound assignment requires two `int`s, found `{}` {}= `{}`",
                                    type_name(&sym.ty),
                                    match compound_op {
                                        TokenKind::PlusEq => "+",
                                        TokenKind::MinusEq => "-",
                                        TokenKind::StarEq => "*",
                                        TokenKind::SlashEq => "/",
                                        _ => "%",
                                    },
                                    type_name(&value_ty)
                                ),
                                *span,
                            );
                        }
                        Type::Int
                    }
                    _ => unreachable!("parser only produces assignment operators here"),
                };
            }
            Stmt::Expr(expr) => {
                self.analyze_expr(expr);
            }
            Stmt::Return { value, span } => {
                let ret_ty = value
                    .as_ref()
                    .map(|e| self.analyze_expr(e))
                    .unwrap_or(Type::Void);
                if let Some(expected) = &self.current_return_type
                    && *expected != ret_ty
                {
                    self.error(
                        format!(
                            "return type mismatch: expected `{}`, found `{}`",
                            type_name(expected),
                            type_name(&ret_ty)
                        ),
                        *span,
                    );
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_ty = self.analyze_expr(condition);
                if !cond_ty.is_bool() {
                    self.error("condition must be `bool`", condition.span());
                }
                self.analyze_block(then_branch);
                if let Some(else_b) = else_branch {
                    self.analyze_block(else_b);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                let cond_ty = self.analyze_expr(condition);
                if !cond_ty.is_bool() {
                    self.error("condition must be `bool`", condition.span());
                }
                self.analyze_block(body);
            }
            Stmt::Block(block) => self.analyze_block(block),
        }
    }

    fn analyze_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Int { .. } => Type::Int,
            Expr::Bool { .. } => Type::Bool,
            Expr::String { .. } => Type::String,
            Expr::Variable { name, span } => {
                if let Some(sym) = self.scopes.resolve(name) {
                    sym.ty.clone()
                } else {
                    self.error(format!("undefined variable `{}`", name), *span);
                    Type::Int // recovery
                }
            }
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                let left_ty = self.analyze_expr(left);
                let right_ty = self.analyze_expr(right);

                match op {
                    TokenKind::Plus
                    | TokenKind::Minus
                    | TokenKind::Star
                    | TokenKind::Slash
                    | TokenKind::Percent => {
                        if left_ty.is_numeric() && right_ty.is_numeric() {
                            Type::Int
                        } else if *op == TokenKind::Plus
                            && left_ty == Type::String
                            && right_ty == Type::String
                        {
                            // Concatenation, compiled to rune_strcat.
                            Type::String
                        } else if *op == TokenKind::Plus
                            && (left_ty == Type::String || right_ty == Type::String)
                        {
                            self.error("cannot mix strings and numbers", *span);
                            Type::String
                        } else {
                            self.error("operands must be numbers", *span);
                            Type::Int
                        }
                    }
                    TokenKind::EqEq | TokenKind::BangEq => {
                        // String comparison is compiled to strcmp by the
                        // emitters, so same-type operands are fine here.
                        if left_ty == right_ty {
                            Type::Bool
                        } else {
                            self.error("operands must have the same type", *span);
                            Type::Bool
                        }
                    }
                    TokenKind::Less
                    | TokenKind::LessEq
                    | TokenKind::Greater
                    | TokenKind::GreaterEq => {
                        if left_ty.is_numeric() && right_ty.is_numeric() {
                            Type::Bool
                        } else {
                            self.error("operands must be numbers", *span);
                            Type::Bool
                        }
                    }
                    TokenKind::AmpAmp | TokenKind::PipePipe => {
                        if left_ty.is_bool() && right_ty.is_bool() {
                            Type::Bool
                        } else {
                            self.error("operands must be bool", *span);
                            Type::Bool
                        }
                    }
                    _ => {
                        self.error("invalid binary operator", *span);
                        Type::Int
                    }
                }
            }
            Expr::Unary { op, expr, span } => {
                let ty = self.analyze_expr(expr);
                match op {
                    TokenKind::Minus if ty.is_numeric() => Type::Int,
                    TokenKind::Bang if ty.is_bool() => Type::Bool,
                    _ => {
                        self.error("invalid unary operator for type", *span);
                        ty
                    }
                }
            }
            Expr::Call { callee, args, span } => {
                // The only builtin: everything else must be a declared function.
                match callee.as_str() {
                    "print" => {
                        if args.len() != 1 {
                            self.error("print takes exactly one argument", *span);
                        } else {
                            let arg_ty = self.analyze_expr(&args[0]);
                            if arg_ty == Type::Void {
                                self.error("cannot print a `void` value", *span);
                            }
                        }
                        Type::Void
                    }
                    name => match self.functions.get(name).cloned() {
                        Some(signature) => {
                            if args.len() != signature.params.len() {
                                self.error(
                                    format!(
                                        "function `{}` expects {} argument(s), found {}",
                                        name,
                                        signature.params.len(),
                                        args.len()
                                    ),
                                    *span,
                                );
                            }
                            for (arg, param_ty) in args.iter().zip(&signature.params) {
                                let arg_ty = self.analyze_expr(arg);
                                if arg_ty != *param_ty {
                                    self.error(
                                        format!(
                                            "argument type mismatch: expected `{}`, found `{}`",
                                            type_name(param_ty),
                                            type_name(&arg_ty)
                                        ),
                                        arg.span(),
                                    );
                                }
                            }
                            // Analyze any excess arguments so their own errors
                            // still get reported.
                            for arg in args.iter().skip(signature.params.len()) {
                                self.analyze_expr(arg);
                            }
                            signature.return_type
                        }
                        None => {
                            self.error(format!("call to undefined function `{}`", name), *span);
                            // Recovery type so the rest of the function still checks.
                            Type::Int
                        }
                    },
                }
            }
            Expr::Grouping { expr, .. } => self.analyze_expr(expr),
        }
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(Diagnostic::error(message, span));
    }
}

fn type_name(ty: &Type) -> &'static str {
    match ty {
        Type::Int => "int",
        Type::Bool => "bool",
        Type::String => "string",
        Type::Void => "void",
    }
}
