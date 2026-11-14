//! lexer for Rune.
//!
//! Turns source text into a stream of tokens while tracking precise spans.

use crate::diagnostics::diagnostic::Diagnostic;
use crate::language::token::{Token, TokenKind};
use crate::shared::source::SourceFile;
use crate::shared::span::Span;

pub struct Lexer<'a> {
    source: &'a SourceFile,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a SourceFile) -> Self {
        Self { source, pos: 0 }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = matches!(tok.kind, TokenKind::Eof);
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    pub fn next_token(&mut self) -> Result<Token, Diagnostic> {
        self.skip_whitespace_and_comments();

        if self.is_at_end() {
            return Ok(Token::new(TokenKind::Eof, self.make_span(self.pos)));
        }

        let start = self.pos;
        let c = self.current_char();

        let kind = match c {
            '0'..='9' => self.lex_number(start)?,
            'a'..='z' | 'A'..='Z' | '_' => self.lex_ident_or_keyword(start),
            '"' => self.lex_string(start)?,
            '+' => {
                self.advance();
                TokenKind::Plus
            }
            '-' => {
                self.advance();
                if self.current_char() == '>' {
                    self.advance();
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                self.advance();
                TokenKind::Star
            }
            '/' => {
                self.advance();
                TokenKind::Slash
            }
            '%' => {
                self.advance();
                TokenKind::Percent
            }
            '=' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    TokenKind::EqEq
                } else {
                    TokenKind::Eq
                }
            }
            '!' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    TokenKind::BangEq
                } else {
                    TokenKind::Bang
                }
            }
            '<' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    TokenKind::LessEq
                } else {
                    TokenKind::Less
                }
            }
            '>' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    TokenKind::GreaterEq
                } else {
                    TokenKind::Greater
                }
            }
            '&' if self.peak() == '&' => {
                self.advance();
                self.advance();
                TokenKind::AmpAmp
            }
            '|' if self.peak() == '|' => {
                self.advance();
                self.advance();
                TokenKind::PipePipe
            }
            '(' => {
                self.advance();
                TokenKind::LParen
            }
            ')' => {
                self.advance();
                TokenKind::RParen
            }
            '{' => {
                self.advance();
                TokenKind::LBrace
            }
            '}' => {
                self.advance();
                TokenKind::RBrace
            }
            ',' => {
                self.advance();
                TokenKind::Comma
            }
            ':' => {
                self.advance();
                TokenKind::Colon
            }
            ';' => {
                self.advance();
                TokenKind::Semicolon
            }
            _ => {
                return Err(Diagnostic::error(
                    format!("unexpected character `{}`", c),
                    self.make_span(start),
                ));
            }
        };

        Ok(Token::new(kind, self.make_span(start)))
    }

    // ------- Helper functions
    fn lex_number(&mut self, start: usize) -> Result<TokenKind, Diagnostic> {
        while self.current_char().is_ascii_digit() {
            self.advance();
        }

        let text = &self.source.content[start..self.pos];
        let value = text
            .parse::<i64>()
            .map_err(|_| Diagnostic::error("invalid integer literal", self.make_span(start)))?;
        Ok(TokenKind::Int(value))
    }

    fn lex_ident_or_keyword(&mut self, start: usize) -> TokenKind {
        while matches!(self.current_char(), 'a'..='z' | 'A'..='Z' | '0'..='9' | '_') {
            self.advance();
        }
        let text = &self.source.content[start..self.pos];
        match text {
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "print" => TokenKind::Print,
            "bool" => TokenKind::Bool,
            "int" => TokenKind::IntType,
            "string" => TokenKind::StringType,
            _ => TokenKind::Ident(text.to_string()),
        }
    }

    fn lex_string(&mut self, start: usize) -> Result<TokenKind, Diagnostic> {
        self.advance(); // opening "
        let mut value = String::new();

        while !self.is_at_end() && self.current_char() != '"' {
            if self.current_char() == '\\' {
                self.advance();
                match self.current_char() {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    '"' => value.push('"'),
                    '\\' => value.push('\\'),
                    _ => value.push(self.current_char()),
                }
            } else {
                value.push(self.current_char());
            }
            self.advance()
        }
        if self.is_at_end() {
            return Err(Diagnostic::error(
                "unterminated string",
                self.make_span(start),
            ));
        }

        self.advance(); // closing "
        Ok(TokenKind::String(value))
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.current_char() {
                ' ' | '\t' | '\r' | '\n' => self.advance(),
                '/' if self.peak() == '/' => {
                    while !self.is_at_end() && self.current_char() != '\n' {
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    fn current_char(&self) -> char {
        self.source.content[self.pos..]
            .chars()
            .next()
            .unwrap_or('\0')
    }

    fn peak(&self) -> char {
        let mut chars = self.source.content[self.pos..].chars();
        chars.next();
        chars.next().unwrap_or('\0')
    }

    fn advance(&mut self) {
        if let Some(c) = self.source.content[self.pos..].chars().next() {
            self.pos += c.len_utf8();
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.source.content.len()
    }

    fn make_span(&self, start: usize) -> Span {
        Span::new(start, self.pos)
    }
}
