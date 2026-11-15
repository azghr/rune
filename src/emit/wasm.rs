// SPDX-License-Identifier: MIT

//! WebAssembly backend: emits the text format (WAT) from the IR.
//!
//! WASM has no goto, so the basic-block CFG is compiled into a dispatch
//! loop: every block becomes a case in a `br_table`, and each terminator
//! sets the next block's index and restarts the loop. This is correct for
//! arbitrary control-flow graphs without needing a relooping pass:
//!
//! ```text
//! (block $exit
//!   (loop $top
//!     (block $B1        ;; outer labels for higher block ids
//!       (block $B0
//!         (br_table $B0 $B1 (local.get $pc)))
//!       ... block 0's code, ending in `br $top` or `return` ...
//!     )
//!     ... block 1's code ...
//!   ))
//! ```
//!
//! Type mapping: `int` → `i64`, `bool` and `string` → `i32` (strings are
//! pointers into linear memory). String literals live in data segments; a
//! small runtime prelude provides concatenation (`rune_strcat`), contents
//! comparison (`rune_strcmp`), and a bump allocator with memory growth.
//! Concatenated strings are never freed, mirroring the C backend.
//! Function values are rejected with a clear error: they need the
//! reference-types proposal. `print` is provided by host imports.
//!
//! Validate the output with `wat2wasm` (from the wabt toolchain).

use crate::language::ast::Type;
use crate::transform::ir::*;
use std::fmt::Write;

/// Fixed WAT runtime prelude appended to every module: a bump allocator over
/// the linear memory (growing it as needed), strlen, strcat, and strcmp for
/// the string operations the IR can lower to. Allocation addresses come
/// after the string-literal data segments (the heap global is initialized
/// by `generate`).
const RUNE_RUNTIME: &str = r#"
  (func $f_rune_alloc (param $sz i32) (result i32) (local $ptr i32)
    (local.set $sz (i32.and (i32.add (local.get $sz) (i32.const 7)) (i32.const -8)))
    (local.set $ptr (global.get $rune_heap))
    (global.set $rune_heap (i32.add (local.get $ptr) (local.get $sz)))
    ;; grow memory when the allocation runs past the current pages
    (if (i32.gt_u (global.get $rune_heap) (i32.mul (memory.size) (i32.const 65536)))
      (then
        (drop
          (memory.grow
            (i32.div_u
              (i32.add
                (i32.sub (global.get $rune_heap) (i32.mul (memory.size) (i32.const 65536)))
                (i32.const 65535))
              (i32.const 65536))))))
    (local.get $ptr))

  (func $f_rune_strlen (param $s i32) (result i32) (local $n i32)
    (block $done
      (loop $next
        (br_if $done (i32.eqz (i32.load8_u (i32.add (local.get $s) (local.get $n)))))
        (local.set $n (i32.add (local.get $n) (i32.const 1)))
        (br $next)))
    (local.get $n))

  (func $f_rune_strcat (param $a i32) (param $b i32) (result i32)
    (local $out i32) (local $la i32) (local $lb i32) (local $i i32)
    (local.set $la (call $f_rune_strlen (local.get $a)))
    (local.set $lb (call $f_rune_strlen (local.get $b)))
    (local.set $out (call $f_rune_alloc (i32.add (i32.add (local.get $la) (local.get $lb)) (i32.const 1))))
    (local.set $i (i32.const 0))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $la)))
        (i32.store8 (i32.add (local.get $out) (local.get $i))
                    (i32.load8_u (i32.add (local.get $a) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (local.set $i (i32.const 0))
    (block $done
      (loop $next
        (br_if $done (i32.ge_u (local.get $i) (local.get $lb)))
        (i32.store8 (i32.add (i32.add (local.get $out) (local.get $la)) (local.get $i))
                    (i32.load8_u (i32.add (local.get $b) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.store8 (i32.add (i32.add (local.get $out) (local.get $la)) (local.get $lb))
                (i32.const 0))
    (local.get $out))

  (func $f_rune_strcmp (param $a i32) (param $b i32) (result i32)
    (local $i i32) (local $ca i32) (local $cb i32)
    (local.set $i (i32.const 0))
    (block $done
      (loop $next
        (local.set $ca (i32.load8_u (i32.add (local.get $a) (local.get $i))))
        (local.set $cb (i32.load8_u (i32.add (local.get $b) (local.get $i))))
        (br_if $done (i32.ne (local.get $ca) (local.get $cb)))
        (br_if $done (i32.eqz (local.get $ca)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.sub (local.get $ca) (local.get $cb)))
"#;

/// Emits a complete WAT module, or an error describing the first unsupported
/// construct.
pub fn generate(module: &Module) -> Result<String, String> {
    for func in &module.functions {
        check_function(func)?;
    }

    let mut out = String::new();
    out.push_str("(module\n");

    // Host imports for the print builtin (all three variants, for
    // simplicity).
    out.push_str("  (import \"rune\" \"print_int\" (func $f_rune_print_int (param i64)))\n");
    out.push_str("  (import \"rune\" \"print_bool\" (func $f_rune_print_bool (param i32)))\n");
    out.push_str("  (import \"rune\" \"print_string\" (func $f_rune_print_string (param i32)))\n");

    // Linear memory: exported so hosts can read string values. Literals
    // live in data segments; the runtime prelude bump-allocates past them.
    out.push_str("  (memory (export \"rune_mem\") 1)\n");

    let literals = collect_string_literals(module);
    let heap_base = string_heap_base(&literals);
    let _ = writeln!(
        out,
        "  (global $rune_heap (mut i32) (i32.const {heap_base}))"
    );
    for (addr, text) in &literals {
        let _ = writeln!(
            out,
            "  (data (i32.const {addr}) \"{}\")",
            escape_wat_data(text)
        );
    }
    if !literals.is_empty() {
        out.push('\n');
    }
    // Runtime prelude: allocation, strlen, strcat, strcmp (fixed WAT text).
    out.push_str(RUNE_RUNTIME);

    for func in &module.functions {
        out.push('\n');
        emit_function(&mut out, func, &literals);
    }

    // main is the entry point; export it when present.
    if module.functions.iter().any(|f| f.name == "main") {
        out.push_str("\n  (export \"main\" (func $f_main))\n");
    }

    out.push_str(")\n");
    Ok(out)
}

/// Rejects constructs the backend does not support yet, with the function
/// name in the message.
fn check_function(func: &Function) -> Result<(), String> {
    let unsupported = |what: &str| -> String {
        format!(
            "the WASM backend does not support {} yet (in function `{}`)",
            what, func.name
        )
    };

    for (_, ty) in &func.params {
        check_type(ty).map_err(|_| unsupported("function values in parameters"))?;
    }
    check_type(&func.return_type).map_err(|_| unsupported("function values in return types"))?;

    for block in &func.body {
        for inst in &block.instructions {
            if let Instruction::CallIndirect { .. } = inst {
                return Err(unsupported("calls through function values"));
            }
            let tys = match inst {
                Instruction::Binary { ty, .. }
                | Instruction::Unary { ty, .. }
                | Instruction::Move { ty, .. }
                | Instruction::Call { ty, .. } => vec![ty.clone()],
                Instruction::CallIndirect { .. } => unreachable!("checked above"),
            };
            for ty in tys {
                check_type(&ty)
                    .map_err(|_| unsupported("strings or function values in instructions"))?;
            }
        }
    }
    Ok(())
}

fn check_type(ty: &Type) -> Result<(), ()> {
    match ty {
        Type::Int | Type::Bool | Type::Void | Type::String => Ok(()),
        Type::Function { .. } => Err(()),
    }
}

/// The WASM value type for a supported Rune type.
fn wasm_type(ty: &Type) -> &'static str {
    match ty {
        Type::Int => "i64",
        Type::Bool => "i32",
        Type::Void => "",
        Type::String => "i32", // pointer into linear memory
        Type::Function { .. } => unreachable!("rejected by check_function"),
    }
}

/// The local identifier for a variable or temp.
fn local_ref(name: &str) -> String {
    format!("$l_{name}")
}

fn emit_function(out: &mut String, func: &Function, literals: &[(u32, String)]) {
    let _ = write!(out, "  (func $f_{}", func.name);
    for (name, ty) in &func.params {
        let _ = write!(out, " (param {} {})", local_ref(name), wasm_type(ty));
    }
    if func.return_type != Type::Void {
        let _ = write!(out, " (result {})", wasm_type(&func.return_type));
    }

    // All non-parameter locals (variables and temps).
    let locals = local_types(func);
    let param_names: Vec<&str> = func.params.iter().map(|(n, _)| n.as_str()).collect();
    for (name, ty) in &locals {
        if !param_names.contains(&name.as_str()) {
            let _ = write!(out, " (local {} {})", local_ref(name), wasm_type(ty));
        }
    }
    // The dispatch register for the control-flow loop.
    out.push_str(" (local $rune_pc i32)\n");

    emit_body(out, func, &locals, literals);
    out.push_str("  )\n");
}

/// Collects the type of every local (variable or temp) written by the
/// function's instructions, in first-write order.
fn local_types(func: &Function) -> Vec<(String, Type)> {
    let mut types: Vec<(String, Type)> = func
        .params
        .iter()
        .map(|(n, ty)| (n.clone(), ty.clone()))
        .collect();
    for block in &func.body {
        for inst in &block.instructions {
            let dest = match inst {
                Instruction::Binary { dest, .. }
                | Instruction::Unary { dest, .. }
                | Instruction::Move { dest, .. }
                | Instruction::Call {
                    dest: Some(dest), ..
                } => dest,
                Instruction::Call { dest: None, .. } | Instruction::CallIndirect { .. } => continue,
            };
            if !types.iter().any(|(n, _)| n == dest) {
                let ty = match inst {
                    Instruction::Binary { ty, .. }
                    | Instruction::Unary { ty, .. }
                    | Instruction::Move { ty, .. }
                    | Instruction::Call {
                        dest: Some(_), ty, ..
                    } => ty.clone(),
                    Instruction::Call { dest: None, .. } | Instruction::CallIndirect { .. } => {
                        continue;
                    }
                };
                types.push((dest.clone(), ty));
            }
        }
    }
    types
}

fn emit_body(
    out: &mut String,
    func: &Function,
    locals: &[(String, Type)],
    literals: &[(u32, String)],
) {
    let blocks = &func.body;
    let n = blocks.len();
    let ind = "    "; // depth 1: inside the func

    let _ = writeln!(out, "{ind}(block $rune_exit");
    let _ = writeln!(out, "{ind}  (loop $rune_top");

    // Nest the per-block labels: $B{n-1} outermost, $B0 innermost, so the
    // br_table can exit to any of them.
    for id in (0..n).rev() {
        let _ = writeln!(out, "{ind}    (block $B{id}");
    }

    // The dispatch: pc k exits to $Bk, and control resumes just after that
    // block's `end` — so each case's code must sit immediately AFTER its
    // label's end, not inside it.
    let labels = (0..n)
        .map(|i| format!("$B{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    let _ = writeln!(out, "{ind}      (block $rune_dispatch");
    let _ = writeln!(out, "{ind}        (br_table {labels} (local.get $rune_pc))");
    let _ = writeln!(out, "{ind}      )");

    for block in blocks {
        let _ = writeln!(out, "{ind}    )");
        emit_block_case(out, block, &format!("{ind}    "), locals, literals);
    }

    let _ = writeln!(out, "{ind}  )");
    let _ = writeln!(out, "{ind})");
    // Every path through the dispatch loop exits via `return`; the code
    // after the exit block is unreachable, but the validator still requires
    // non-void functions to leave a value here.
    let _ = writeln!(out, "{ind}unreachable");
}

/// Emits one case: the block's instructions and a terminator that either
/// returns from the function or sets the dispatch register and restarts the
/// loop.
fn emit_block_case(
    out: &mut String,
    block: &BasicBlock,
    ind: &str,
    locals: &[(String, Type)],
    literals: &[(u32, String)],
) {
    let case = ind.to_string(); // cases sit one level deeper than their label
    for inst in &block.instructions {
        emit_instruction(out, inst, &case, locals, literals);
    }
    match &block.terminator {
        Terminator::Return(Some(value)) => {
            emit_value(out, value, ind, locals, literals);
            let _ = writeln!(out, "{ind}return");
        }
        Terminator::Return(None) => {
            let _ = writeln!(out, "{ind}return");
        }
        Terminator::Jump(target) => {
            let _ = writeln!(out, "{ind}(i32.const {target})");
            let _ = writeln!(out, "{ind}(local.set $rune_pc)");
            let _ = writeln!(out, "{ind}br $rune_top");
        }
        Terminator::Branch {
            condition,
            then_block,
            else_block,
        } => {
            emit_value(out, condition, ind, locals, literals);
            let _ = writeln!(out, "{ind}(if");
            let _ = writeln!(out, "{ind}  (then");
            let _ = writeln!(out, "{ind}    (i32.const {then_block})");
            let _ = writeln!(out, "{ind}    (local.set $rune_pc)");
            let _ = writeln!(out, "{ind}  )");
            let _ = writeln!(out, "{ind}  (else");
            let _ = writeln!(out, "{ind}    (i32.const {else_block})");
            let _ = writeln!(out, "{ind}    (local.set $rune_pc)");
            let _ = writeln!(out, "{ind}  )");
            let _ = writeln!(out, "{ind})");
            let _ = writeln!(out, "{ind}br $rune_top");
        }
    }
}

fn emit_instruction(
    out: &mut String,
    inst: &Instruction,
    ind: &str,
    locals: &[(String, Type)],
    literals: &[(u32, String)],
) {
    match inst {
        Instruction::Binary {
            dest,
            left,
            op,
            right,
            ty,
        } => {
            emit_value(out, left, ind, locals, literals);
            emit_value(out, right, ind, locals, literals);
            let string_operands = value_type(left, locals) == Type::String
                || value_type(right, locals) == Type::String;
            if matches!(op, BinOp::Eq | BinOp::NotEq | BinOp::Add) && string_operands {
                // String operations go through the runtime prelude.
                if *op == BinOp::Add {
                    let _ = writeln!(out, "{ind}call $f_rune_strcat");
                } else {
                    let _ = writeln!(out, "{ind}call $f_rune_strcmp");
                    // strcmp == 0 means equal; flip for `!=`.
                    let _ = writeln!(out, "{ind}i32.eqz");
                    if *op == BinOp::NotEq {
                        let _ = writeln!(out, "{ind}i32.eqz");
                    }
                }
            } else {
                let _ = writeln!(out, "{ind}{}", wasm_bin_op(op, ty));
            }
            let _ = writeln!(out, "{ind}local.set {}", local_ref(dest));
        }
        Instruction::Unary {
            dest, op, value, ..
        } => {
            match op {
                UnaryOp::Neg => {
                    // 0 - value
                    let _ = writeln!(out, "{ind}i64.const 0");
                    emit_value(out, value, ind, locals, literals);
                    let _ = writeln!(out, "{ind}i64.sub");
                }
                UnaryOp::Not => {
                    emit_value(out, value, ind, locals, literals);
                    let _ = writeln!(out, "{ind}i32.eqz");
                }
            }
            let _ = writeln!(out, "{ind}local.set {}", local_ref(dest));
        }
        Instruction::Move { dest, value, .. } => {
            emit_value(out, value, ind, locals, literals);
            let _ = writeln!(out, "{ind}local.set {}", local_ref(dest));
        }
        Instruction::Call {
            dest,
            name,
            args,
            ty,
        } => {
            for arg in args {
                emit_value(out, arg, ind, locals, literals);
            }
            let callee = if name == "print" {
                // Dispatch on the argument's static type.
                match args.first().map(|a| value_type(a, locals)) {
                    Some(Type::Bool) => "$f_rune_print_bool",
                    Some(Type::String) => "$f_rune_print_string",
                    _ => "$f_rune_print_int",
                }
            } else {
                &format!("$f_{name}")
            };
            let _ = writeln!(out, "{ind}call {callee}");
            if let Some(dest) = dest {
                let _ = writeln!(out, "{ind}local.set {}", local_ref(dest));
            } else {
                let _ = ty;
            }
        }
        Instruction::CallIndirect { .. } => unreachable!("rejected by check_function"),
    }
}

/// Comparison and arithmetic ops operate on `int` (i64) operands; bool
/// operands never reach a Binary (And/Or are lowered to control flow).
fn wasm_bin_op(op: &BinOp, ty: &Type) -> &'static str {
    match (op, ty) {
        (BinOp::Eq, _) => "i64.eq",
        (BinOp::NotEq, _) => "i64.ne",
        (BinOp::Lt, _) => "i64.lt_s",
        (BinOp::Le, _) => "i64.le_s",
        (BinOp::Gt, _) => "i64.gt_s",
        (BinOp::Ge, _) => "i64.ge_s",
        (_, Type::Int) => match op {
            BinOp::Add => "i64.add",
            BinOp::Sub => "i64.sub",
            BinOp::Mul => "i64.mul",
            BinOp::Div => "i64.div_s",
            BinOp::Mod => "i64.rem_s",
            _ => unreachable!("string and logical operators are lowered elsewhere"),
        },
        _ => unreachable!("unexpected binary operand type {ty:?}"),
    }
}

fn value_type(value: &Value, locals: &[(String, Type)]) -> Type {
    match value {
        Value::ConstInt(_) => Type::Int,
        Value::ConstBool(_) => Type::Bool,
        Value::ConstString(_) => Type::String,
        Value::Local(name) => locals
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, ty)| ty.clone())
            .unwrap_or(Type::Int),
    }
}

/// Addresses for every string literal in the module, laid out from a fixed
/// base so the runtime heap starts after them.
fn collect_string_literals(module: &Module) -> Vec<(u32, String)> {
    const STRING_BASE: u32 = 1024;
    let mut out: Vec<(u32, String)> = Vec::new();
    let mut next = STRING_BASE;
    for func in &module.functions {
        for block in &func.body {
            for inst in &block.instructions {
                let mut check_value = |value: &Value| {
                    if let Value::ConstString(text) = value
                        && !out.iter().any(|(_, t)| t == text)
                    {
                        out.push((next, text.clone()));
                        next += text.len() as u32 + 1; // NUL terminator
                    }
                };
                match inst {
                    Instruction::Move { value, .. } => check_value(value),
                    Instruction::Binary { left, right, .. } => {
                        check_value(left);
                        check_value(right);
                    }
                    Instruction::Unary { value, .. } => check_value(value),
                    Instruction::Call { args, .. } => {
                        for arg in args {
                            check_value(arg);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

/// Where the runtime bump allocator may start: after all literal bytes,
/// 8-byte aligned.
fn string_heap_base(literals: &[(u32, String)]) -> u32 {
    let end = literals
        .last()
        .map_or(1024, |(addr, text)| addr + text.len() as u32 + 1);
    (end + 7) & !7
}

/// Escapes a Rune string as a WAT data-segment string (bytes with `\"` and
/// `\\` literal, everything non-printable as hex).
fn escape_wat_data(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            b'"' => out.push_str("\\22"),
            b'\\' => out.push_str("\\5C"),
            b'\n' => out.push_str("\\0a"),
            b'\t' => out.push_str("\\09"),
            0x20..=0x7e => out.push(b as char),
            other => {
                let _ = write!(out, "\\{other:02x}");
            }
        }
    }
    out.push_str("\\00");
    out
}

fn emit_value(
    out: &mut String,
    value: &Value,
    _ind: &str,
    _locals: &[(String, Type)],
    literals: &[(u32, String)],
) {
    match value {
        Value::ConstInt(v) => {
            let _ = writeln!(out, "    (i64.const {v})");
        }
        Value::ConstBool(b) => {
            let _ = writeln!(out, "    (i32.const {})", if *b { 1 } else { 0 });
        }
        Value::ConstString(text) => {
            // The literal's address in linear memory (from the data segments).
            let addr = literals
                .iter()
                .find(|(_, t)| t == text)
                .map(|(a, _)| *a)
                .unwrap_or(0);
            let _ = writeln!(out, "    (i32.const {addr})");
        }
        Value::Local(name) => {
            let _ = writeln!(out, "    local.get {}", local_ref(name));
        }
    }
}
