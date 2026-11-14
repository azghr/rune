//! Top-level declarations (currently only functions).

use super::Parser;
use crate::diagnostics::diagnostic::Diagnostic;
use crate::language::ast::{Function, Param};
use crate::language::token::TokenKind;

impl Parser {
    pub fn parse_function(&mut self) -> Result<Function, Diagnostic> {
        let start_span = self.consume(TokenKind::Fn, "expected `fn`")?.span;

        let name_tok = self.consume(TokenKind::Ident(String::new()), "expected function name")?;
        let name = match &name_tok.kind {
            TokenKind::Ident(n) => n.clone(),
            _ => unreachable!(),
        };

        self.consume(TokenKind::LParen, "expected `(` after function name")?;

        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let (param_name, param_span) = {
                    let tok =
                        self.consume(TokenKind::Ident(String::new()), "expected parameter name")?;
                    let name = match &tok.kind {
                        TokenKind::Ident(n) => n.clone(),
                        _ => unreachable!(),
                    };
                    (name, tok.span)
                };

                self.consume(TokenKind::Colon, "expected `:` after parameter name")?;
                let ty = self.parse_type()?;

                params.push(Param {
                    name: param_name,
                    ty,
                    span: param_span,
                });

                if !self.matches(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        self.consume(TokenKind::RParen, "expected `)` after parameters")?;

        let return_type = if self.matches(&[TokenKind::Arrow]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(TokenKind::LBrace, "expected `{` before function body")?;
        let body = self.parse_block_after_lbrace()?;
        let span = start_span.merge(body.span);

        Ok(Function {
            name,
            params,
            return_type,
            body,
            span,
        })
    }
}
