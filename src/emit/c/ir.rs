// SPDX-License-Identifier: MIT

//! Emits C99 from the IR.
//!
//! Each basic block becomes a `goto` label; the result is not as readable as
//! the direct AST-to-C generator, but it reflects exactly what the transform
//! passes produced.

use crate::language::ast::Type;
use crate::transform::ir::*;
use std::collections::HashMap;
use std::fmt::Write;

/// Emits a complete C translation unit for the module.
pub fn generate(module: &Module) -> String {
    let mut out = String::new();
    out.push_str("#include <stdio.h>\n");
    out.push_str("#include <stdint.h>\n");
    out.push_str("#include <stdbool.h>\n");
    out.push_str("#include <string.h>\n");
    out.push_str("#include \"runtime/rune_rt.h\"\n\n");

    // Forward declarations so definition order never matters.
    for func in &module.functions {
        let _ = writeln!(out, "{};", signature(func));
    }

    // Closures call their target through env-first wrappers; declare and
    // define one for every function a MakeClosure references.
    let closure_targets = closure_targets(module);
    for name in &closure_targets {
        let target = module
            .functions
            .iter()
            .find(|f| &f.name == name)
            .expect("closure target must be a declared function");
        let _ = writeln!(out, "{};", closure_wrapper_signature(target));
    }
    out.push('\n');

    for func in &module.functions {
        emit_function(&mut out, func);
    }
    for name in closure_targets {
        let target = module
            .functions
            .iter()
            .find(|f| f.name == name)
            .expect("closure target must be a declared function");
        emit_closure_wrapper(&mut out, target);
    }
    out
}

/// The env-first wrapper for a closure target: drops the environment and
/// forwards to the real function.
fn closure_wrapper_name(func: &str) -> String {
    format!("rune_closure_{func}")
}

fn closure_wrapper_signature(target: &Function) -> String {
    let params = std::iter::once("void *env".to_string())
        .chain(
            target
                .params
                .iter()
                .map(|(n, ty)| super::ctype::c_decl(ty, n)),
        )
        .collect::<Vec<_>>()
        .join(", ");
    super::ctype::c_signature(
        &target.return_type,
        &format!("{}({})", closure_wrapper_name(&target.name), params),
    )
}

fn emit_closure_wrapper(out: &mut String, target: &Function) {
    out.push_str(&closure_wrapper_signature(target));
    out.push_str(" {\n    (void)env;\n    ");
    if target.return_type != Type::Void {
        out.push_str("return ");
    }
    let _ = write!(out, "{}(", target.name);
    for (i, (name, _)) in target.params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(name);
    }
    out.push_str(");\n}\n\n");
}

/// Unique functions referenced by MakeClosure instructions, in first-use
/// order.
fn closure_targets(module: &Module) -> Vec<String> {
    let mut names = Vec::new();
    for func in &module.functions {
        for block in &func.body {
            for inst in &block.instructions {
                if let Instruction::MakeClosure { func: target, .. } = inst
                    && !names.contains(target)
                {
                    names.push(target.clone());
                }
            }
        }
    }
    names
}

fn signature(func: &Function) -> String {
    if func.name == "main" {
        // C requires `main` to return `int`
        let params = func
            .params
            .iter()
            .map(|(name, ty)| super::ctype::c_decl(ty, name))
            .collect::<Vec<_>>()
            .join(", ");
        return format!("int main({})", params);
    }
    let params = func
        .params
        .iter()
        .map(|(name, ty)| super::ctype::c_decl(ty, name))
        .collect::<Vec<_>>()
        .join(", ");
    super::ctype::c_signature(&func.return_type, &format!("{}({})", func.name, params))
}

/// Collects the type of every local (variable or temp) declared by the
/// function's instructions, for declarations and `print` dispatch.
fn local_types(func: &Function) -> HashMap<&str, Type> {
    let mut types: HashMap<&str, Type> = func
        .params
        .iter()
        .map(|(name, ty)| (name.as_str(), ty.clone()))
        .collect();
    for block in &func.body {
        for inst in &block.instructions {
            let (dest, ty) = match inst {
                Instruction::Binary { dest, ty, .. }
                | Instruction::Unary { dest, ty, .. }
                | Instruction::Move { dest, ty, .. }
                | Instruction::Call {
                    dest: Some(dest),
                    ty,
                    ..
                }
                | Instruction::CallIndirect {
                    dest: Some(dest),
                    ty,
                    ..
                }
                | Instruction::MakeClosure { dest, ty, .. } => (dest.as_str(), ty.clone()),
                Instruction::Call { dest: None, .. }
                | Instruction::CallIndirect { dest: None, .. } => continue,
            };
            types.entry(dest).or_insert(ty);
        }
    }
    types
}

fn value_type(value: &Value, types: &HashMap<&str, Type>) -> Type {
    match value {
        Value::ConstInt(_) => Type::Int,
        Value::ConstBool(_) => Type::Bool,
        Value::ConstString(_) => Type::String,
        Value::Local(name) => types.get(name.as_str()).cloned().unwrap_or(Type::Int),
    }
}

fn emit_function(out: &mut String, func: &Function) {
    out.push_str(&signature(func));
    out.push_str(" {\n");

    // Declare all locals up front; the lowering always writes a local before
    // reading it on any path, so this is safe.
    let types = local_types(func);
    let param_names: Vec<&str> = func.params.iter().map(|(n, _)| n.as_str()).collect();
    for (name, ty) in &types {
        if !param_names.contains(name) {
            let _ = writeln!(out, "    {};", super::ctype::c_decl(ty, name));
        }
    }

    for block in &func.body {
        let _ = writeln!(out, "L{}:;", block.id);
        for inst in &block.instructions {
            out.push_str("    ");
            emit_instruction(out, inst, &types);
        }
        match &block.terminator {
            Terminator::Return(Some(value)) => {
                out.push_str("    return ");
                emit_value(out, value);
                out.push_str(";\n");
            }
            Terminator::Return(None) => {
                // Falling off the end of a non-void function (including
                // `main`) returns 0; only void functions get a bare return.
                if func.name == "main" || func.return_type != Type::Void {
                    out.push_str("    return 0;\n");
                } else {
                    out.push_str("    return;\n");
                }
            }
            Terminator::Jump(target) => {
                let _ = writeln!(out, "    goto L{};", target);
            }
            Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                out.push_str("    if (");
                emit_value(out, condition);
                let _ = writeln!(out, ") goto L{};", then_block);
                let _ = writeln!(out, "    goto L{};", else_block);
            }
        }
        out.push('\n');
    }

    out.push_str("}\n\n");
}

fn emit_instruction(out: &mut String, inst: &Instruction, types: &HashMap<&str, Type>) {
    match inst {
        Instruction::Binary {
            dest,
            left,
            op,
            right,
            ty,
            ..
        } => {
            let _ = write!(out, "{} = ", dest);
            if *op == BinOp::Add && *ty == Type::String {
                // Concatenation goes through the runtime helper; the result
                // is heap-allocated and never freed (see rune_rt.c).
                out.push_str("rune_strcat(");
                emit_value(out, left);
                out.push_str(", ");
                emit_value(out, right);
                out.push(')');
            } else if matches!(op, BinOp::Eq | BinOp::NotEq)
                && (value_type(left, types) == Type::String
                    || value_type(right, types) == Type::String)
            {
                // String equality compares contents via strcmp; == on the C
                // side would compare pointers.
                let cmp = match op {
                    BinOp::Eq => "== 0",
                    _ => "!= 0",
                };
                out.push_str("strcmp(");
                emit_value(out, left);
                out.push_str(", ");
                emit_value(out, right);
                let _ = write!(out, ") {}", cmp);
            } else {
                emit_value(out, left);
                let _ = write!(out, " {} ", op_to_c(op));
                emit_value(out, right);
            }
            out.push_str(";\n");
        }
        Instruction::Unary {
            dest, op, value, ..
        } => {
            let _ = write!(out, "{} = {}", dest, unary_op_to_c(op));
            emit_value(out, value);
            out.push_str(";\n");
        }
        Instruction::Move { dest, value, .. } => {
            let _ = write!(out, "{} = ", dest);
            emit_value(out, value);
            out.push_str(";\n");
        }
        Instruction::Call {
            dest, name, args, ..
        } => {
            if let Some(dest) = dest {
                let _ = write!(out, "{} = ", dest);
            }
            emit_call(out, name, args, types);
            out.push_str(";\n");
        }
        Instruction::CallIndirect {
            dest, callee, args, ..
        } => {
            if let Some(dest) = dest {
                let _ = write!(out, "{} = ", dest);
            }
            // Function values are closures: the code word is the first
            // field of the block. Cast it to an env-first signature and
            // pass the closure itself as the environment.
            let callee_text = match callee {
                Value::Local(name) => name.clone(),
                other => unreachable!("non-local callee {other:?} in CallIndirect"),
            };
            let callee_ty = value_type(callee, types);
            let (sig_params, ret_decl) = match &callee_ty {
                Type::Function {
                    params,
                    return_type,
                } => {
                    // env pointer first, then the signature's parameters
                    let ps = std::iter::once("void *".to_string())
                        .chain(
                            params
                                .iter()
                                .map(|p| super::ctype::c_decl(p, "").trim_end().to_string()),
                        )
                        .collect::<Vec<_>>()
                        .join(", ");
                    (
                        ps,
                        super::ctype::c_decl(return_type, "").trim_end().to_string(),
                    )
                }
                _ => ("void *".to_string(), "int64_t".to_string()),
            };
            let _ = write!(
                out,
                "(({} (*)({}))(*((void **){})))( {}",
                ret_decl, sig_params, callee_text, callee_text
            );
            for arg in args {
                out.push_str(", ");
                emit_value(out, arg);
            }
            out.push(')');
            out.push_str(";\n");
        }
        Instruction::MakeClosure {
            dest,
            func,
            captures,
            ..
        } => {
            // Allocate a closure block: [wrapper code, capture values...].
            let _ = writeln!(
                out,
                "{} = rune_closure_new((void *){}, {});",
                dest,
                closure_wrapper_name(func),
                captures.len()
            );
            for (idx, cap) in captures.iter().enumerate() {
                let _ = write!(out, "rune_closure_set({}, {}, (int64_t)(", dest, idx);
                emit_value(out, cap);
                out.push_str("));\n");
            }
        }
    }
}

fn emit_call(out: &mut String, name: &str, args: &[Value], types: &HashMap<&str, Type>) {
    if name == "print" {
        // Dispatch on the argument's static type.
        let runtime_fn = match args.first().map(|a| value_type(a, types)) {
            Some(Type::Bool) => "rune_print_bool",
            Some(Type::String) => "rune_print_string",
            _ => "rune_print_int",
        };
        out.push_str(runtime_fn);
    } else {
        out.push_str(name);
    }
    out.push('(');
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        emit_value(out, arg);
    }
    out.push(')');
}

fn emit_value(out: &mut String, value: &Value) {
    match value {
        Value::ConstInt(v) => {
            let _ = write!(out, "{}", v);
        }
        Value::ConstBool(v) => {
            let _ = write!(out, "{}", v);
        }
        Value::ConstString(s) => {
            let _ = write!(out, "\"{}\"", escape_c_string(s));
        }
        Value::Local(name) => {
            let _ = write!(out, "{}", name);
        }
    }
}

/// Re-escapes a Rune string value as a C string literal. The lexer decoded
/// escape sequences (\n, \t, \\", \\\\) into raw characters, so they must be
/// restored before emitting C.
fn escape_c_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

fn op_to_c(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::NotEq => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

fn unary_op_to_c(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
    }
}
