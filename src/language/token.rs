//! Token definitions for the Rune language.

use crate::shared::span::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // Literals
    Int(i64),
    Float(f64),
    String(String),
    Ident(String),

    // Keywords
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    Return,
    True,
    False,
    Print,
    Bool,
    IntType,
    StringType,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    Bang,
    BangEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    AmpAmp,
    PipePipe,

    // Punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Arrow,
    Semicolon,

    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}
