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
        // Function values are closure pointers (see rune_rt.h) regardless
        // of their signature.
        Type::Function { .. } => format!("void* {}", name),
        // Arrays are pointers to [length, elems...] blocks (see rune_rt.h).
        Type::Array(_) => format!("void* {}", name),
    }
}

/// Renders a function signature line `RET NAME(PARAMS)`. Functions
/// returning closures return `void*` like any function value.
pub fn c_signature(ret: &Type, name_and_params: &str) -> String {
    format!("{} {}", c_decl(ret, "").trim_end(), name_and_params)
}

/// The C type name for a value type, or None for function types (which need
/// the declarator syntax from `c_decl`).
pub fn c_value_type(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::Int => Some("int64_t"),
        Type::Bool => Some("bool"),
        Type::String => Some("const char*"),
        Type::Void => Some("void"),
        Type::Function { .. } => Some("void*"),
        Type::Array(_) => Some("void*"),
    }
}
