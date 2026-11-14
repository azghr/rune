// SPDX-License-Identifier: MIT

//! Type system for Rune.

use crate::language::ast::Type;

impl Type {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Type::Bool)
    }
}

pub fn type_name(ty: &Type) -> &'static str {
    match ty {
        Type::Int => "int",
        Type::Bool => "bool",
        Type::String => "string",
        Type::Void => "void",
    }
}
