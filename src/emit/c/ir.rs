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
    out.push('\n');

    for func in &module.functions {
        emit_function(&mut out, func);
    }
    out
}

fn c_type_name(ty: &Type) -> &'static str {
    match ty {
        Type::Int => "int64_t",
        Type::Bool => "bool",
        Type::String => "const char*",
        Type::Void => "void",
    }
}

fn signature(func: &Function) -> String {
    let ret = if func.name == "main" {
        // C requires `main` to return `int`
        "int"
    } else {
        c_type_name(&func.return_type)
    };
    let params = func
        .params
        .iter()
        .map(|(name, ty)| format!("{} {}", c_type_name(ty), name))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{} {}({})", ret, func.name, params)
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
                } => (dest.as_str(), ty.clone()),
                Instruction::Call { dest: None, .. } => continue,
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
            let _ = writeln!(out, "    {} {};", c_type_name(ty), name);
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
            ..
        } => {
            let _ = write!(out, "{} = ", dest);
            // String equality compares contents via strcmp; == on the C
            // side would compare pointers.
            if matches!(op, BinOp::Eq | BinOp::NotEq)
                && (value_type(left, types) == Type::String
                    || value_type(right, types) == Type::String)
            {
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
            let _ = write!(out, "\"{}\"", s);
        }
        Value::Local(name) => {
            let _ = write!(out, "{}", name);
        }
    }
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
