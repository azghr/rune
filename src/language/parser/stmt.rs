//! Statement parsing for Rune.

use super::Parser;
use crate::diagnostics::diagnostic::Diagnostic;
use crate::language::ast::{Block, Expr, Stmt};
use crate::language::token::TokenKind;

impl Parser {
    /// Accepts an optional statement terminator (`;`). Newlines are skipped
    /// by the lexer, so statements may also be separated by line breaks.
    fn end_of_statement(&mut self) {
        self.matches(&[TokenKind::Semicolon]);
    }

    pub fn parse_statement(&mut self) -> Result<Stmt, Diagnostic> {
        if self.matches(&[TokenKind::Let]) {
            self.parse_let()
        } else if self.matches(&[TokenKind::If]) {
            self.parse_if()
        } else if self.matches(&[TokenKind::While]) {
            self.parse_while()
        } else if self.matches(&[TokenKind::Return]) {
            self.parse_return()
        } else if self.matches(&[TokenKind::LBrace]) {
            let block = self.parse_block_after_lbrace()?;
            Ok(Stmt::Block(block))
        } else {
            // Expression statement or assignment
            let expr = self.parse_expression()?;
            if let Expr::Variable { name, span } = &expr
                && self.matches(&[TokenKind::Eq])
            {
                let value = self.parse_expression()?;
                let value_span = value.span();
                self.end_of_statement();
                return Ok(Stmt::Assign {
                    name: name.clone(),
                    value,
                    span: span.merge(value_span),
                });
            }

            self.end_of_statement();
            Ok(Stmt::Expr(expr))
        }
    }

    fn parse_let(&mut self) -> Result<Stmt, Diagnostic> {
        let start_span = self.previous().span;
        let name_tok = self.consume(TokenKind::Ident(String::new()), "expected variable name")?;
        let name = match &name_tok.kind {
            TokenKind::Ident(n) => n.clone(),
            _ => unreachable!(),
        };
        let ty = if self.matches(&[TokenKind::Colon]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(TokenKind::Eq, "expected `=` after variable name")?;
        let value = self.parse_expression()?;
        self.end_of_statement();

        Ok(Stmt::Let {
            name,
            ty,
            value,
            span: start_span.merge(self.previous().span),
        })
    }

    fn parse_if(&mut self) -> Result<Stmt, Diagnostic> {
        let start_span = self.previous().span;
        let condition = self.parse_expression()?;
        self.consume(TokenKind::LBrace, "expected `{` after if condition")?;
        let then_branch = self.parse_block_after_lbrace()?;

        let else_branch = if self.matches(&[TokenKind::Else]) {
            self.consume(TokenKind::LBrace, "expected `{` after else")?;
            Some(self.parse_block_after_lbrace()?)
        } else {
            None
        };

        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
            span: start_span.merge(self.previous().span),
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, Diagnostic> {
        let start_span = self.previous().span;
        let condition = self.parse_expression()?;
        self.consume(TokenKind::LBrace, "expected `{` after while condition")?;
        let body = self.parse_block_after_lbrace()?;

        Ok(Stmt::While {
            condition,
            body,
            span: start_span.merge(self.previous().span),
        })
    }

    fn parse_return(&mut self) -> Result<Stmt, Diagnostic> {
        let start_span = self.previous().span;
        let value = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.end_of_statement();
        Ok(Stmt::Return {
            value,
            span: start_span.merge(self.previous().span),
        })
    }

    pub fn parse_block_after_lbrace(&mut self) -> Result<Block, Diagnostic> {
        let start_span = self.previous().span;
        let mut statements = Vec::new();

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.parse_statement()?);
        }

        let end = self.consume(TokenKind::RBrace, "expected `}` after block")?;
        Ok(Block {
            statements,
            span: start_span.merge(end.span),
        })
    }

    pub fn parse_type(&mut self) -> Result<crate::language::ast::Type, Diagnostic> {
        let tok = self.advance();
        match &tok.kind {
            TokenKind::IntType => Ok(crate::language::ast::Type::Int),
            TokenKind::Bool => Ok(crate::language::ast::Type::Bool),
            TokenKind::StringType => Ok(crate::language::ast::Type::String),
            _ => Err(Diagnostic::error("expected type name", tok.span)),
        }
    }
}
