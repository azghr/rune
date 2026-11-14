// SPDX-License-Identifier: MIT

//! Simple three-address style Intermediate Representation.
//!
//! This IR is intentionally small so it is easy to understand and transform.
//! Later it can be lowered to LLVM IR or machine code.

use crate::language::ast::Type;

#[derive(Debug, Clone)]
pub struct Module {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub return_type: Type,
    pub body: Vec<BasicBlock>,
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: usize,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone)]
pub enum Instruction {
    /// dest = left op right
    Binary {
        dest: String,
        left: Value,
        op: BinOp,
        right: Value,
        ty: Type,
    },
    /// dest = op value
    Unary {
        dest: String,
        op: UnaryOp,
        value: Value,
        ty: Type,
    },
    /// dest = value
    Move {
        dest: String,
        value: Value,
        ty: Type,
    },
    /// dest = call name(args...)
    Call {
        dest: Option<String>,
        name: String,
        args: Vec<Value>,
        ty: Type,
    },
}

#[derive(Debug, Clone)]
pub enum Terminator {
    Return(Option<Value>),
    Jump(usize),
    Branch {
        condition: Value,
        then_block: usize,
        else_block: usize,
    },
}

#[derive(Debug, Clone)]
pub enum Value {
    ConstInt(i64),
    ConstBool(bool),
    ConstString(String),
    Local(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}
