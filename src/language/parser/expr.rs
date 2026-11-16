// SPDX-License-Identifier: MIT

//! Expression parsing (precedence climbing via recursive descent).

use super::Parser;
use crate::diagnostics::diagnostic::Diagnostic;
use crate::language::ast::Expr;
use crate::language::token::TokenKind;

impl Parser {
    pub fn parse_expression(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_logical()
    }

    fn parse_logical(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_equality()?;

        while self.matches(&[TokenKind::AmpAmp, TokenKind::PipePipe]) {
            let op = self.previous().kind.clone();
            let right = self.parse_equality()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }

        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_comparison()?;

        while self.matches(&[TokenKind::EqEq, TokenKind::BangEq]) {
            let op = self.previous().kind.clone();
            let right = self.parse_comparison()?;
            let span = expr.span().merge(right.span());

            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }

        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_term()?;

        while self.matches(&[
            TokenKind::Less,
            TokenKind::LessEq,
            TokenKind::Greater,
            TokenKind::GreaterEq,
        ]) {
            let op = self.previous().kind.clone();
            let right = self.parse_term()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_factor()?;

        while self.matches(&[TokenKind::Plus, TokenKind::Minus]) {
            let op = self.previous().kind.clone();
            let right = self.parse_factor()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_unary()?;

        while self.matches(&[TokenKind::Star, TokenKind::Slash, TokenKind::Percent]) {
            let op = self.previous().kind.clone();
            let right = self.parse_unary()?;
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.matches(&[TokenKind::Bang, TokenKind::Minus]) {
            let op = self.previous().kind.clone();
            let expr = self.parse_unary()?;
            let span = self.previous().span.merge(expr.span());
            return Ok(Expr::Unary {
                op,
                expr: Box::new(expr),
                span,
            });
        }
        let primary = self.parse_primary()?;
        self.parse_postfix(primary)
    }

    /// Postfix operators on any primary expression: calls (`f(x)`) and
    /// indexing (`a[i]`), chaining freely (`f(x)[0]`, `m[i][j]`).
    fn parse_postfix(&mut self, mut expr: Expr) -> Result<Expr, Diagnostic> {
        loop {
            if self.matches(&[TokenKind::LParen]) {
                let args = self.parse_call_args()?;
                let callee_span = expr.span();
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    span: callee_span.merge(self.previous().span),
                };
            } else if self.matches(&[TokenKind::LBracket]) {
                let index = self.parse_expression()?;
                self.consume(TokenKind::RBracket, "expected `]` after index")?;
                let array_span = expr.span();
                expr = Expr::Index {
                    array: Box::new(expr),
                    index: Box::new(index),
                    span: array_span.merge(self.previous().span),
                };
            } else {
                return Ok(expr);
            }
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.advance();
        let span = token.span;
        let kind = token.kind.clone();

        match kind {
            TokenKind::Fn => {
                // Anonymous function expression: fn(params) -> ret { body }.
                self.consume(TokenKind::LParen, "expected `(` after `fn`")?;
                let params = self.parse_fn_params()?;
                let return_type = if self.matches(&[TokenKind::Arrow]) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                self.consume(TokenKind::LBrace, "expected `{` to start the function body")?;
                let body = self.parse_block_after_lbrace()?;
                Ok(Expr::AnonymousFunction {
                    params,
                    return_type,
                    body,
                    span: span.merge(self.previous().span),
                })
            }
            TokenKind::Int(v) => Ok(Expr::Int { value: v, span }),
            TokenKind::True => Ok(Expr::Bool { value: true, span }),
            TokenKind::False => Ok(Expr::Bool { value: false, span }),
            TokenKind::String(s) => Ok(Expr::String { value: s, span }),
            TokenKind::Ident(name) => Ok(Expr::Variable { name, span }),
            TokenKind::LBracket => {
                // Array literal: [elem, elem, ...].
                let mut elems = Vec::new();
                if !self.check(&TokenKind::RBracket) {
                    loop {
                        elems.push(self.parse_expression()?);
                        if !self.matches(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RBracket, "expected `]` after array elements")?;
                Ok(Expr::ArrayLiteral {
                    elems,
                    span: span.merge(self.previous().span),
                })
            }
            TokenKind::Print => {
                // `print` is a keyword but reads as a builtin call expression
                self.consume(TokenKind::LParen, "expected `(` after `print`")?;
                let args = self.parse_call_args()?;
                Ok(Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: "print".to_string(),
                        span,
                    }),
                    args,
                    span: span.merge(self.previous().span),
                })
            }
            TokenKind::LParen => {
                let expr = self.parse_expression()?;
                let end = self.consume(TokenKind::RParen, "expected `)` after expression")?;
                Ok(Expr::Grouping {
                    expr: Box::new(expr),
                    span: span.merge(end.span),
                })
            }
            _ => Err(Diagnostic::error("expected expression", span)),
        }
    }

    /// Parses a parenthesized argument list, after the opening `(`.
    fn parse_call_args(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        let mut args = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                args.push(self.parse_expression()?);
                if !self.matches(&[TokenKind::Comma]) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, "expected `)` after arguments")?;
        Ok(args)
    }
}

// Small helper so we can call .span() on Expr
impl Expr {
    pub fn span(&self) -> crate::shared::span::Span {
        match self {
            Expr::Int { span, .. }
            | Expr::Bool { span, .. }
            | Expr::String { span, .. }
            | Expr::Variable { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Call { span, .. }
            | Expr::ArrayLiteral { span, .. }
            | Expr::Index { span, .. }
            | Expr::AnonymousFunction { span, .. }
            | Expr::Grouping { span, .. } => *span,
        }
    }
}
