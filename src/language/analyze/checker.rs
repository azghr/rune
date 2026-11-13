//! Type checker and semantic analyzer.

use crate::diagnostics::diagnostic::Diagnostic;
use crate::language::analyze::scope::{ScopeStack, Symbol};
use crate::language::ast::*;
use crate::language::token::TokenKind;
use crate::shared::span::Span;

pub struct Analyzer {
    scopes: ScopeStack,
    current_return_type: Option<Type>,
    errors: Vec<Diagnostic>,
}

impl Analyzer {
    pub fn new() -> Self {
        Self {
            scopes: ScopeStack::new(),
            current_return_type: None,
            errors: Vec::new(),
        }
    }

    pub fn analyze(mut self, program: Program) -> Result<Program, Vec<Diagnostic>> {
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
                ty,
                value,
                span,
            } => {
                let value_ty = self.analyze_expr(value);
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
                    mutable: true,
                }) {
                    self.error(msg, *span);
                }
            }
            Stmt::Assign { name, value, span } => {
                let sym = self.scopes.resolve(name);
                if let Some(sym) = sym {
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
            }
            Stmt::Expr(expr) => {
                self.analyze_expr(expr);
            }
            Stmt::Return { value, span } => {
                let ret_ty = value
                    .as_ref()
                    .map(|e| self.analyze_expr(e))
                    .unwrap_or(Type::Void);
                if let Some(expected) = &self.current_return_type {
                    if *expected != ret_ty {
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
                        } else {
                            self.error("operands must be numbers", *span);
                            Type::Int
                        }
                    }
                    TokenKind::EqEq | TokenKind::BangEq => {
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
                // Very simple: we only support a few built-ins for now
                match callee.as_str() {
                    "print" => {
                        if args.len() != 1 {
                            self.error("print takes exactly one argument", *span);
                        } else {
                            self.analyze_expr(&args[0]);
                        }
                        Type::Void
                    }
                    _ => {
                        // User functions – for now just accept and return Int
                        for arg in args {
                            self.analyze_expr(arg);
                        }
                        Type::Int
                    }
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
