// SPDX-License-Identifier: MIT

//! Lexer tests: tokens, spans, comments, and lexer errors.

mod common;

use common::{source, tokens};
use rune::language::token::TokenKind;

#[test]
fn lexes_integer_literals() {
    assert_eq!(tokens("42")[0].kind, TokenKind::Int(42));
    assert_eq!(tokens("0")[0].kind, TokenKind::Int(0));
}

#[test]
fn lexes_keywords() {
    let src = "fn let if else while return true false print int bool string mut";
    let kinds: Vec<TokenKind> = tokens(src)
        .into_iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .map(|t| t.kind)
        .collect();
    let expected = [
        TokenKind::Fn,
        TokenKind::Let,
        TokenKind::If,
        TokenKind::Else,
        TokenKind::While,
        TokenKind::Return,
        TokenKind::True,
        TokenKind::False,
        TokenKind::Print,
        TokenKind::IntType,
        TokenKind::Bool,
        TokenKind::StringType,
        TokenKind::Mut,
    ];
    assert_eq!(kinds, expected);
}

#[test]
fn lexes_identifiers() {
    assert_eq!(
        tokens("hello_world")[0].kind,
        TokenKind::Ident("hello_world".into())
    );
    // identifiers are case sensitive and not confused with keywords
    assert_eq!(tokens("Let")[0].kind, TokenKind::Ident("Let".into()));
}

#[test]
fn lexes_string_literals() {
    assert_eq!(tokens("\"hi\"")[0].kind, TokenKind::String("hi".into()));
    assert_eq!(tokens("\"\"")[0].kind, TokenKind::String("".into()));
}

#[test]
fn lexes_single_char_operators() {
    let kinds: Vec<TokenKind> = tokens("+ - * / % ! < > ( ) { } , : ;")
        .into_iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .map(|t| t.kind)
        .collect();
    let expected = [
        TokenKind::Plus,
        TokenKind::Minus,
        TokenKind::Star,
        TokenKind::Slash,
        TokenKind::Percent,
        TokenKind::Bang,
        TokenKind::Less,
        TokenKind::Greater,
        TokenKind::LParen,
        TokenKind::RParen,
        TokenKind::LBrace,
        TokenKind::RBrace,
        TokenKind::Comma,
        TokenKind::Colon,
        TokenKind::Semicolon,
    ];
    assert_eq!(kinds, expected);
}

#[test]
fn lexes_two_char_operators_as_single_tokens() {
    let kinds: Vec<TokenKind> = tokens("== != <= >= && ||")
        .into_iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .map(|t| t.kind)
        .collect();
    let expected = [
        TokenKind::EqEq,
        TokenKind::BangEq,
        TokenKind::LessEq,
        TokenKind::GreaterEq,
        TokenKind::AmpAmp,
        TokenKind::PipePipe,
    ];
    assert_eq!(kinds, expected);
}

#[test]
fn lexes_compound_assignment_operators() {
    let kinds: Vec<TokenKind> = tokens("+= -= *= /= %=")
        .into_iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .map(|t| t.kind)
        .collect();
    let expected = [
        TokenKind::PlusEq,
        TokenKind::MinusEq,
        TokenKind::StarEq,
        TokenKind::SlashEq,
        TokenKind::PercentEq,
    ];
    assert_eq!(kinds, expected);
}

#[test]
fn lexes_the_for_keyword() {
    assert_eq!(tokens("for")[0].kind, TokenKind::For);
}

#[test]
fn skips_comments() {
    let kinds: Vec<TokenKind> = tokens("// full line comment\n42 // trailing\n")
        .into_iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .map(|t| t.kind)
        .collect();
    assert_eq!(kinds, vec![TokenKind::Int(42)]);
}

#[test]
fn token_spans_point_at_source() {
    let src = "let x = 10";
    let src_file = source(src);
    for tok in tokens(src) {
        assert_eq!(
            &src_file.content[tok.span.to_range()],
            match &tok.kind {
                TokenKind::Let => "let",
                TokenKind::Ident(_) => "x",
                TokenKind::Eq => "=",
                TokenKind::Int(_) => "10",
                TokenKind::Eof => "",
                _ => panic!("unexpected token {:?}", tok.kind),
            }
        );
    }
}

#[test]
fn stream_ends_with_eof() {
    assert_eq!(tokens("").last().unwrap().kind, TokenKind::Eof);
    assert_eq!(tokens("42").last().unwrap().kind, TokenKind::Eof);
}

#[test]
fn errors_on_unexpected_character() {
    let err = rune::language::lexer::Lexer::new(&source("@"))
        .tokenize()
        .expect_err("`@` should not lex");
    assert!(err.message.contains("unexpected character"));
}

#[test]
fn errors_on_integer_overflow() {
    let err = rune::language::lexer::Lexer::new(&source("99999999999999999999999"))
        .tokenize()
        .expect_err("out-of-range integer should not lex");
    assert!(err.message.contains("invalid integer literal"));
}
