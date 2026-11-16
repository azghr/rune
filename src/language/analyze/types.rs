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

pub fn type_name(ty: &Type) -> String {
    match ty {
        Type::Int => "int".to_string(),
        Type::Bool => "bool".to_string(),
        Type::String => "string".to_string(),
        Type::Void => "void".to_string(),
        Type::Function {
            params,
            return_type,
        } => {
            let params = params.iter().map(type_name).collect::<Vec<_>>().join(", ");
            format!("fn({}) -> {}", params, type_name(return_type))
        }
        Type::Array(elem) => format!("[{}]", type_name(elem)),
    }
}
