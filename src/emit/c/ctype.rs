// SPDX-License-Identifier: MIT

//! Helpers for rendering Rune types as C declarations.
//!
//! Function values map to C function pointers, whose declarator syntax puts
//! the name *inside* the type (`int64_t (*f)(int64_t)`), so plain
//! `format!("{} {}", type, name)` does not work. `c_decl` renders a full
//! declaration for any type, using abstract declarators (empty name) when a
//! bare type is needed.

use crate::language::ast::Type;

/// Renders `TYPE NAME` as a C declaration (or an abstract declarator when
/// `name` is empty).
pub fn c_decl(ty: &Type, name: &str) -> String {
    match ty {
        Type::Int => format!("int64_t {}", name),
        Type::Bool => format!("bool {}", name),
        Type::String => format!("const char* {}", name),
        Type::Void => format!("void {}", name),
        Type::Function {
            params,
            return_type,
        } => {
            let params = params
                .iter()
                .map(|p| c_decl(p, "").trim_end().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{} (*{})({})",
                c_decl(return_type, "").trim_end(),
                name,
                params
            )
        }
    }
}

/// Renders a function signature line `RET NAME(PARAMS)`, handling the
/// function-returning-function case via C's declarator syntax.
pub fn c_signature(ret: &Type, name_and_params: &str) -> String {
    if let Type::Function {
        params,
        return_type,
    } = ret
    {
        let params = params
            .iter()
            .map(|p| c_decl(p, "").trim_end().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        // `RET (*name(args))(inner_params)` declares a function returning a
        // function pointer.
        format!(
            "{} (*{})({})",
            c_decl(return_type, "").trim_end(),
            name_and_params,
            params
        )
    } else {
        format!("{} {}", c_decl(ret, "").trim_end(), name_and_params)
    }
}

/// The C type name for a value type, or None for function types (which need
/// the declarator syntax from `c_decl`).
pub fn c_value_type(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::Int => Some("int64_t"),
        Type::Bool => Some("bool"),
        Type::String => Some("const char*"),
        Type::Void => Some("void"),
        Type::Function { .. } => None,
    }
}
