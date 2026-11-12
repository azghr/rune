//! Expression parsing (precedence climbing via recursive descent).

use super::Parser;
use crate::diagnostics::diagnostic::Diagnostic;
use crate::language::ast::Expr;
use crate::language::token::TokenKind;

impl Parser {
    pub fn parse_expression(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_equality();
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
                right: Box::new(expr),
                span,
            };
        }

        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_term();

        while self.matches(&[
            TokenKind::Less,
            TokenKind::LessEq,
            TokenKind::Greater,
            TokenKind::GreaterEq,
        ]) {
            let op = self.previous().kind.clone();
            let right = self.parse_term();
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
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.advance();
        match &token.kind {
            TokenKind::Int(v) => Ok(Expr::Int {
                value: *v,
                span: token.span,
            }),
            TokenKind::True => Ok(Expr::Bool {
                value: true,
                span: token.span,
            }),
            TokenKind::False => Ok(Expr::Bool {
                value: false,
                span: token.span,
            }),
            TokenKind::String(s) => Ok(Expr::String {
                value: s.clone(),
                span: token.span,
            }),
            TokenKind::Inent(name) => {
                if self.matches(&[TokenKind::LParen]) {
                    // function call
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expression()?);
                            if !self.matches(&[TokenKind::Comma]) {
                                break;
                            }
                        }
                    }
                    let end = self.consume(TokenKind::RParen, "expected `)` after arguments")?;
                    Ok(Expr::Call {
                        allee: name.clone(),
                        args,
                        span: token.span.merge(end.span),
                    })
                } else {
                    Ok(Expr::Variable {
                        name: name.clone(),
                        span: token.span,
                    })
                }
            }
            TokenKind::LParen => {
                let expr = self.parse_expression()?;
                let end = self.consume(TokenKind::RParen, "expected `)` after expression")?;
                Ok(Expr::Grouping {
                    expr: Box::new(expr),
                    span: token.span.merge(end.span),
                })
            }
            _ => Err(Diagnostic::error("expected expression", token.span)),
        }
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
            | Expr::Grouping { span, .. } => *span,
        }
    }
}
