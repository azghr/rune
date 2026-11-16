// SPDX-License-Identifier: MIT

//! Abstract Syntax Tree definitions for Rune.

use crate::language::token::TokenKind;
use crate::shared::span::Span;

#[derive(Clone, Debug)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: String,
        mutable: bool,
        ty: Option<Type>,
        value: Expr,
        span: Span,
    },
    Assign {
        name: String,
        /// `Eq` for plain assignment, or the arithmetic operator for
        /// compound assignment (`+=` etc.).
        op: TokenKind,
        value: Expr,
        span: Span,
    },
    /// `array[index] = value` (or a compound form like `+=`). The target is
    /// an arbitrary index expression, evaluated once.
    IndexAssign {
        array: Box<Expr>,
        index: Box<Expr>,
        /// `Eq` for plain assignment, or the arithmetic operator for
        /// compound assignment (`+=` etc.).
        op: TokenKind,
        value: Expr,
        span: Span,
    },
    Expr(Expr),
    Return {
        value: Option<Expr>,
        span: Span,
    },
    If {
        condition: Expr,
        then_branch: Block,
        else_branch: Option<Block>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Block,
        span: Span,
    },
    For {
        init: Option<Box<Stmt>>,
        condition: Option<Expr>,
        step: Option<Box<Stmt>>,
        body: Block,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Block(Block),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int {
        value: i64,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    String {
        value: String,
        span: Span,
    },
    Variable {
        name: String,
        span: Span,
    },
    Binary {
        left: Box<Expr>,
        op: TokenKind,
        right: Box<Expr>,
        span: Span,
    },
    Unary {
        op: TokenKind,
        expr: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    /// `[e0, e1, ...]` — an array literal. Element type is inferred from the
    /// elements (which must all agree); empty literals are rejected.
    ArrayLiteral {
        elems: Vec<Expr>,
        span: Span,
    },
    /// `array[index]` in expression position (a read).
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    AnonymousFunction {
        params: Vec<Param>,
        return_type: Option<Type>,
        body: Block,
        span: Span,
    },
    Grouping {
        expr: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Bool,
    String,
    Void,
    /// A function type, written `fn(param, ...) -> ret`. Enables passing
    /// functions as values.
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
    },
    /// An array type, written `[elem]`. A value is a pointer to a heap
    /// block of `[length, elem...]` slots (see the runtime).
    Array(Box<Type>),
    /// Recovery type for expressions whose type could not be determined
    /// (an error has already been reported for them). Never produced by
    /// well-typed programs, so it never reaches lowering or code
    /// generation; the checker treats it as compatible with every type to
    /// avoid cascading diagnostics.
    Error,
}
