// SPDX-License-Identifier: MIT

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

    /// Whether the current token can begin an expression. Used to decide
    /// whether a statement like `return` carries a value: `return }` or a
    /// `return` followed by another statement yields no value.
    fn starts_expression(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Int(_)
                | TokenKind::String(_)
                | TokenKind::Ident(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Print
                | TokenKind::Fn
                | TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::Minus
                | TokenKind::Bang
        )
    }

    pub fn parse_statement(&mut self) -> Result<Stmt, Diagnostic> {
        if self.matches(&[TokenKind::Let]) {
            self.parse_let()
        } else if self.matches(&[TokenKind::If]) {
            self.parse_if()
        } else if self.matches(&[TokenKind::While]) {
            self.parse_while()
        } else if self.matches(&[TokenKind::For]) {
            self.parse_for()
        } else if self.matches(&[TokenKind::Return]) {
            self.parse_return()
        } else if self.matches(&[TokenKind::Break]) {
            let span = self.previous().span;
            self.end_of_statement();
            Ok(Stmt::Break { span })
        } else if self.matches(&[TokenKind::Continue]) {
            let span = self.previous().span;
            self.end_of_statement();
            Ok(Stmt::Continue { span })
        } else if self.matches(&[TokenKind::LBrace]) {
            let block = self.parse_block_after_lbrace()?;
            Ok(Stmt::Block(block))
        } else {
            // Expression statement or assignment
            let expr = self.parse_expression()?;
            let assignment_op = if self.matches(&[
                TokenKind::Eq,
                TokenKind::PlusEq,
                TokenKind::MinusEq,
                TokenKind::StarEq,
                TokenKind::SlashEq,
                TokenKind::PercentEq,
            ]) {
                // The operator doubles as the AST's `op`: `Eq` is plain
                // assignment, the rest are compound assignments.
                Some(self.previous().kind.clone())
            } else {
                None
            };
            if let (Some(op), Expr::Variable { name, span }) = (assignment_op.clone(), &expr) {
                let value = self.parse_expression()?;
                let value_span = value.span();
                self.end_of_statement();
                return Ok(Stmt::Assign {
                    name: name.clone(),
                    op,
                    value,
                    span: span.merge(value_span),
                });
            }
            if let (Some(op), Expr::Index { array, index, span }) = (assignment_op, &expr) {
                // `a[i] = v` (or a compound form); the array expression is
                // evaluated once.
                let value = self.parse_expression()?;
                let value_span = value.span();
                self.end_of_statement();
                return Ok(Stmt::IndexAssign {
                    array: array.clone(),
                    index: index.clone(),
                    op,
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
        let mutable = self.matches(&[TokenKind::Mut]);
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
            mutable,
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
            // `else if` chains desugar into a block wrapping the inner if,
            // so the AST only ever holds if/else over blocks.
            if self.check(&TokenKind::If) {
                self.advance(); // consume `if`; parse_if starts at the condition
                let inner = self.parse_if()?;
                Some(Block {
                    statements: vec![inner],
                    span: self.previous().span,
                })
            } else {
                self.consume(TokenKind::LBrace, "expected `{` or `if` after else")?;
                Some(self.parse_block_after_lbrace()?)
            }
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

    /// Parses a C-style `for` loop:
    ///
    /// ```text
    /// for let mut i = 0; i < 10; i += 1 { body }
    /// ```
    ///
    /// All three clauses are optional. The loop stays a distinct AST node
    /// (rather than desugaring into `while`) so that `continue` can be
    /// lowered to the step clause, matching C semantics.
    fn parse_for(&mut self) -> Result<Stmt, Diagnostic> {
        let start_span = self.previous().span;

        let init = if self.matches(&[TokenKind::Semicolon]) {
            None
        } else {
            // let / assignment / expression statement; consumes the `;`
            Some(self.parse_statement()?)
        };

        let condition = if self.check(&TokenKind::Semicolon) {
            self.advance();
            None
        } else {
            let cond = Some(self.parse_expression()?);
            self.consume(TokenKind::Semicolon, "expected `;` after for condition")?;
            cond
        };

        let step = if self.check(&TokenKind::LBrace) {
            None
        } else {
            // assignment / expression statement; consumes the optional `;`
            Some(self.parse_statement()?)
        };

        self.consume(TokenKind::LBrace, "expected `{` after for clauses")?;
        let body = self.parse_block_after_lbrace()?;

        Ok(Stmt::For {
            init: init.map(Box::new),
            condition,
            step: step.map(Box::new),
            body,
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
        let value = if self.starts_expression() {
            Some(self.parse_expression()?)
        } else {
            None
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
            TokenKind::LBracket => {
                // Array type: [elem].
                let elem = self.parse_type()?;
                self.consume(TokenKind::RBracket, "expected `]` after array element type")?;
                Ok(crate::language::ast::Type::Array(Box::new(elem)))
            }
            TokenKind::Fn => {
                // Function type: fn(param, ...) -> ret (the arrow clause is
                // optional and defaults to void).
                self.consume(TokenKind::LParen, "expected `(` after `fn` in type")?;
                let mut params = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        params.push(self.parse_type()?);
                        if !self.matches(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RParen, "expected `)` after fn type parameters")?;
                let return_type = if self.matches(&[TokenKind::Arrow]) {
                    self.parse_type()?
                } else {
                    crate::language::ast::Type::Void
                };
                Ok(crate::language::ast::Type::Function {
                    params,
                    return_type: Box::new(return_type),
                })
            }
            _ => Err(Diagnostic::error("expected type name", tok.span)),
        }
    }
}
