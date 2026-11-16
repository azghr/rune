// SPDX-License-Identifier: MIT

//! LLVM backend: emits textual LLVM IR (`.ll`) from the IR module.
//!
//! Unlike the WASM backend, LLVM has real basic blocks and branches, so the
//! IR's CFG maps over directly: each block becomes a labeled block, and the
//! terminators become `br`/`ret`. Every variable and temp lives in an
//! `alloca` slot (loaded and stored on use) — the straightforward `-O0`
//! style, avoiding phi nodes entirely.
//!
//! Function values follow the C backend's convention exactly: a value is a
//! pointer to a closure block whose first word is the env-first wrapper's
//! code pointer (LLVM has real function pointers, so no function table is
//! needed), followed by one i64 slot per capture. Blocks are allocated with
//! `rune_closure_new`/`rune_closure_set` from `runtime/rune_rt.c`, which the
//! compiled module links against. Strings are pointers to NUL-terminated
//! globals; `rune_strcat` comes from the C runtime and `strcmp` from libc.
//!
//! Compile the output with a clang that supports textual IR:
//! `clang output.ll runtime/rune_rt.c -o output`.

use crate::language::ast::Type;
use crate::transform::ir::*;
use std::fmt::Write;

/// Emits a complete LLVM IR module (textual `.ll`).
pub fn generate(module: &Module) -> String {
    let mut out = String::new();

    // Host/runtime declarations: printing and strings from the Rune C
    // runtime, strcmp from libc, closures from the runtime's helpers.
    out.push_str("declare void @rune_print_int(i64)\n");
    out.push_str("declare void @rune_print_bool(i8)\n");
    out.push_str("declare void @rune_print_string(ptr)\n");
    out.push_str("declare ptr @rune_strcat(ptr, ptr)\n");
    out.push_str("declare i32 @strcmp(ptr, ptr)\n");
    out.push_str("declare i64 @rune_div(i64, i64)\n");
    out.push_str("declare i64 @rune_mod(i64, i64)\n");
    out.push_str("declare ptr @rune_closure_new(ptr, i32)\n");
    out.push_str("declare void @rune_closure_set(ptr, i32, i64)\n");
    out.push_str("declare ptr @rune_arr_new(i64)\n");
    out.push_str("declare i64 @rune_arr_len(ptr)\n");
    out.push_str("declare i64 @rune_arr_get(ptr, i64)\n");
    out.push_str("declare void @rune_arr_set(ptr, i64, i64)\n");

    // LLVM textual IR is not order-sensitive, so no forward declarations
    // are needed.
    let targets = closure_targets(module);
    out.push('\n');

    // String literals become private NUL-terminated globals.
    let literals = collect_string_literals(module);
    for (name, text, len) in &literals {
        let _ = writeln!(
            out,
            "@{name} = private unnamed_addr constant [{len} x i8] c\"{}\"",
            escape_llvm_data(text)
        );
    }
    if !literals.is_empty() {
        out.push('\n');
    }

    for func in &module.functions {
        emit_function(&mut out, func, &literals, &targets);
    }
    for (name, n_caps) in &targets {
        let target = module
            .functions
            .iter()
            .find(|f| &f.name == name)
            .expect("closure target must be a declared function");
        emit_closure_wrapper(&mut out, target, *n_caps);
    }

    // main is the entry point; provide a C-compatible @main when present.
    if let Some(main) = module.functions.iter().find(|f| f.name == "main") {
        let call = match llvm_type(&main.return_type) {
            "void" => "call void @f_main()".to_string(),
            ret => format!("%unused = call {ret} @f_main()"),
        };
        let _ = writeln!(out, "define i32 @main() {{\n  {call}\n  ret i32 0\n}}\n");
    }

    out
}

/// The LLVM value type for a Rune type. Function values are closure
/// pointers, and strings are pointers to NUL-terminated bytes.
fn llvm_type(ty: &Type) -> &'static str {
    match ty {
        Type::Int => "i64",
        Type::Bool => "i1",
        Type::Void => "void",
        Type::String | Type::Function { .. } | Type::Array(_) => "ptr",
        Type::Error => unreachable!("error type never reaches code generation"),
    }
}

/// The env-first wrapper's signature: the closure pointer followed by the
/// target's user parameters (the captures appended by lowering are loaded
/// from the closure block, not passed by the caller).
fn wrapper_signature(target: &Function, n_caps: usize) -> String {
    let n_user = target.params.len() - n_caps;
    let params = std::iter::once("ptr %env".to_string())
        .chain(
            target.params[..n_user]
                .iter()
                .map(|(n, ty)| format!("{} %arg_{n}", llvm_type(ty))),
        )
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} @f_rune_closure_{}({})",
        llvm_type(&target.return_type),
        target.name,
        params
    )
}

/// Unique functions referenced by MakeClosure instructions, in first-use
/// order, with their capture counts.
fn closure_targets(module: &Module) -> Vec<(String, usize)> {
    let mut names: Vec<(String, usize)> = Vec::new();
    for func in &module.functions {
        for block in &func.body {
            for inst in &block.instructions {
                if let Instruction::MakeClosure {
                    func: target,
                    captures,
                    ..
                } = inst
                    && !names.iter().any(|(n, _)| n == target)
                {
                    names.push((target.clone(), captures.len()));
                }
            }
        }
    }
    names
}

/// Global names for every string literal in the module, deduped in
/// first-use order as `(global name, text, byte length including NUL)`.
fn collect_string_literals(module: &Module) -> Vec<(String, String, u32)> {
    let mut out: Vec<(String, String, u32)> = Vec::new();
    for func in &module.functions {
        for block in &func.body {
            for inst in &block.instructions {
                let check_value = |value: &Value, out: &mut Vec<(String, String, u32)>| {
                    if let Value::ConstString(text) = value
                        && !out.iter().any(|(_, t, _)| t == text)
                    {
                        let name = format!(".str.{}", out.len());
                        out.push((name, text.clone(), text.len() as u32 + 1));
                    }
                };
                match inst {
                    Instruction::Move { value, .. } => check_value(value, &mut out),
                    Instruction::Binary { left, right, .. } => {
                        check_value(left, &mut out);
                        check_value(right, &mut out);
                    }
                    Instruction::Unary { value, .. } => check_value(value, &mut out),
                    Instruction::Call { args, .. } => {
                        for arg in args {
                            check_value(arg, &mut out);
                        }
                    }
                    Instruction::CallIndirect { args, .. } => {
                        for arg in args {
                            check_value(arg, &mut out);
                        }
                    }
                    Instruction::MakeClosure { captures, .. } => {
                        for cap in captures {
                            check_value(cap, &mut out);
                        }
                    }
                    Instruction::MakeArray { elems, .. } => {
                        for elem in elems {
                            check_value(elem, &mut out);
                        }
                    }
                    Instruction::IndexGet { array, index, .. }
                    | Instruction::IndexSet { array, index, .. } => {
                        check_value(array, &mut out);
                        check_value(index, &mut out);
                        if let Instruction::IndexSet { value, .. } = inst {
                            check_value(value, &mut out);
                        }
                    }
                    Instruction::ArrLen { array, .. } => {
                        check_value(array, &mut out);
                    }
                }
            }
        }
    }
    out
}

/// Escapes a Rune string as an LLVM `c"..."` array body (printable bytes
/// literal, `"` and `\` and everything non-printable as hex).
fn escape_llvm_data(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            b'"' => out.push_str("\\22"),
            b'\\' => out.push_str("\\5C"),
            0x20..=0x7e => out.push(b as char),
            other => {
                let _ = write!(out, "\\{other:02X}");
            }
        }
    }
    out.push_str("\\00");
    out
}

/// Emits one Rune function: `alloca` slots for the parameters and every
/// local/temp in the entry block, then a labeled block per IR block.
fn emit_function(
    out: &mut String,
    func: &Function,
    literals: &[(String, String, u32)],
    targets: &[(String, usize)],
) {
    let _ = writeln!(
        out,
        "define {} @f_{}({}) {{",
        llvm_type(&func.return_type),
        func.name,
        func.params
            .iter()
            .map(|(n, ty)| format!("{} %arg_{n}", llvm_type(ty)))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let locals = local_types(func);
    // Entry block: alloca slots for everything, parameters initialized
    // from their argument registers.
    for (name, ty) in &locals {
        let _ = writeln!(out, "  %l_{name} = alloca {}", llvm_type(ty));
    }
    for (name, ty) in &func.params {
        let _ = writeln!(out, "  store {} %arg_{name}, ptr %l_{name}", llvm_type(ty));
    }
    let first = func.body.first().map_or(0, |b| b.id);
    let _ = writeln!(out, "  br label %L{first}");

    let mut emitter = Emitter {
        reg: 0,
        literals,
        targets,
        locals: &locals,
        return_type: func.return_type.clone(),
    };
    for block in &func.body {
        let _ = writeln!(out, "L{}:", block.id);
        for inst in &block.instructions {
            emitter.instruction(out, inst);
        }
        emitter.terminator(out, &block.terminator);
    }
    out.push_str("}\n\n");
}

/// Emits the env-first wrapper for a closure target: takes the closure
/// pointer as `%env`, forwards the user parameters plus the captures loaded
/// from the closure block's i64 slots.
fn emit_closure_wrapper(out: &mut String, target: &Function, n_caps: usize) {
    let n_user = target.params.len() - n_caps;
    let _ = writeln!(out, "define {} {{", wrapper_signature(target, n_caps));
    // The forwarded argument list: user parameters directly, then each
    // capture loaded from the closure block and narrowed to its type.
    let mut args = target.params[..n_user]
        .iter()
        .map(|(n, ty)| format!("{} %arg_{n}", llvm_type(ty)))
        .collect::<Vec<_>>();
    for (i, (_, ty)) in target.params[n_user..].iter().enumerate() {
        // Captures live in the closure block after the code word
        // (rune_closure_set stores them at index i + 1).
        let _ = writeln!(
            out,
            "  %cap{i} = getelementptr i64, ptr %env, i64 {}",
            i + 1
        );
        let _ = writeln!(out, "  %raw{i} = load i64, ptr %cap{i}");
        match ty {
            Type::Int => args.push(format!("i64 %raw{i}")),
            Type::Bool => {
                let _ = writeln!(out, "  %narrow{i} = trunc i64 %raw{i} to i1");
                args.push(format!("i1 %narrow{i}"));
            }
            _ => {
                let _ = writeln!(out, "  %narrow{i} = inttoptr i64 %raw{i} to ptr");
                args.push(format!("ptr %narrow{i}"));
            }
        }
    }
    out.push_str("  ");
    if target.return_type != Type::Void {
        out.push_str("%ret = ");
    }
    let _ = writeln!(
        out,
        "call {} @f_{}({})",
        llvm_type(&target.return_type),
        target.name,
        args.join(", ")
    );
    if target.return_type != Type::Void {
        let _ = writeln!(out, "  ret {} %ret", llvm_type(&target.return_type));
    } else {
        out.push_str("  ret void\n");
    }
    out.push_str("}\n\n");
}

/// Per-function instruction emitter: fresh register names, the module-level
/// literal/target tables, and the function's local types.
struct Emitter<'a> {
    reg: u32,
    literals: &'a [(String, String, u32)],
    targets: &'a [(String, usize)],
    locals: &'a [(String, Type)],
    return_type: Type,
}

impl Emitter<'_> {
    fn fresh(&mut self) -> String {
        let name = format!("%v{}", self.reg);
        self.reg += 1;
        name
    }

    fn local_ref(name: &str) -> String {
        format!("%l_{name}")
    }

    /// Emits code computing `value`, returning the register (or literal)
    /// holding it.
    fn value(&mut self, out: &mut String, value: &Value) -> String {
        match value {
            Value::ConstInt(v) => v.to_string(),
            Value::ConstBool(b) => if *b { "1" } else { "0" }.to_string(),
            Value::ConstString(text) => {
                let (global, len) = self
                    .literals
                    .iter()
                    .find(|(_, t, _)| t == text)
                    .map(|(n, _, l)| (n.to_string(), *l))
                    .unwrap_or((".str.0".to_string(), 1));
                let reg = self.fresh();
                let _ = writeln!(
                    out,
                    "  {reg} = getelementptr inbounds [{len} x i8], ptr @{global}, i64 0, i64 0"
                );
                reg
            }
            Value::Local(name) => {
                let ty = value_type(value, self.locals);
                let reg = self.fresh();
                let _ = writeln!(
                    out,
                    "  {reg} = load {}, ptr {}",
                    llvm_type(&ty),
                    Self::local_ref(name)
                );
                reg
            }
        }
    }

    /// Widens a value to i64 for storage in a closure capture slot.
    fn widen_to_i64(&mut self, out: &mut String, value: &str, ty: &Type) -> String {
        match ty {
            Type::Int => value.to_string(),
            Type::Bool => {
                let reg = self.fresh();
                let _ = writeln!(out, "  {reg} = zext i1 {value} to i64");
                reg
            }
            _ => {
                let reg = self.fresh();
                let _ = writeln!(out, "  {reg} = ptrtoint ptr {value} to i64");
                reg
            }
        }
    }

    fn instruction(&mut self, out: &mut String, inst: &Instruction) {
        match inst {
            Instruction::Binary {
                dest,
                left,
                op,
                right,
                ty,
            } => {
                let l = self.value(out, left);
                let r = self.value(out, right);
                let string_operands = value_type(left, self.locals) == Type::String
                    || value_type(right, self.locals) == Type::String;
                let result = if matches!(op, BinOp::Eq | BinOp::NotEq | BinOp::Add)
                    && string_operands
                {
                    // String operations go through the runtime/libc.
                    if *op == BinOp::Add {
                        let reg = self.fresh();
                        let _ = writeln!(out, "  {reg} = call ptr @rune_strcat(ptr {l}, ptr {r})");
                        reg
                    } else {
                        let cmp = self.fresh();
                        let _ = writeln!(out, "  {cmp} = call i32 @strcmp(ptr {l}, ptr {r})");
                        let eq = self.fresh();
                        let _ = writeln!(out, "  {eq} = icmp eq i32 {cmp}, 0");
                        if *op == BinOp::NotEq {
                            let neq = self.fresh();
                            let _ = writeln!(out, "  {neq} = xor i1 {eq}, 1");
                            neq
                        } else {
                            eq
                        }
                    }
                } else if *ty == Type::Bool {
                    // Comparisons produce i1; bool operands are themselves
                    // i1, every other operand type is i64.
                    let operand_ty = if value_type(left, self.locals) == Type::Bool {
                        "i1"
                    } else {
                        "i64"
                    };
                    let reg = self.fresh();
                    let _ = writeln!(
                        out,
                        "  {reg} = icmp {} {} {l}, {r}",
                        icmp_op(op),
                        operand_ty
                    );
                    reg
                } else if matches!(op, BinOp::Div | BinOp::Mod) {
                    // Division goes through the runtime helpers, which abort
                    // on a zero divisor instead of hitting `sdiv`'s UB.
                    let helper = if *op == BinOp::Div {
                        "rune_div"
                    } else {
                        "rune_mod"
                    };
                    let reg = self.fresh();
                    let _ = writeln!(out, "  {reg} = call i64 @{helper}(i64 {l}, i64 {r})");
                    reg
                } else {
                    let reg = self.fresh();
                    let _ = writeln!(out, "  {reg} = {} i64 {l}, {r}", arith_op(op));
                    reg
                };
                let _ = writeln!(
                    out,
                    "  store {} {result}, ptr {}",
                    llvm_type(ty),
                    Self::local_ref(dest)
                );
            }
            Instruction::Unary {
                dest,
                op,
                value,
                ty,
            } => {
                let v = self.value(out, value);
                let result = match op {
                    UnaryOp::Neg => {
                        let reg = self.fresh();
                        let _ = writeln!(out, "  {reg} = sub i64 0, {v}");
                        reg
                    }
                    UnaryOp::Not => {
                        let reg = self.fresh();
                        let _ = writeln!(out, "  {reg} = xor i1 {v}, 1");
                        reg
                    }
                };
                let _ = writeln!(
                    out,
                    "  store {} {result}, ptr {}",
                    llvm_type(ty),
                    Self::local_ref(dest)
                );
            }
            Instruction::Move { dest, value, .. } => {
                let v = self.value(out, value);
                let ty = value_type(value, self.locals);
                let _ = writeln!(
                    out,
                    "  store {} {v}, ptr {}",
                    llvm_type(&ty),
                    Self::local_ref(dest)
                );
            }
            Instruction::Call {
                dest,
                name,
                args,
                ty,
            } => {
                let emitted = args
                    .iter()
                    .map(|a| (self.value(out, a), value_type(a, self.locals)))
                    .collect::<Vec<_>>();
                if name == "print" {
                    // Dispatch on the argument's static type.
                    match emitted.first() {
                        Some((v, Type::Bool)) => {
                            let wide = self.fresh();
                            let _ = writeln!(out, "  {wide} = zext i1 {v} to i8");
                            let _ = writeln!(out, "  call void @rune_print_bool(i8 {wide})");
                        }
                        Some((v, Type::String)) => {
                            let _ = writeln!(out, "  call void @rune_print_string(ptr {v})");
                        }
                        Some((v, _)) => {
                            let _ = writeln!(out, "  call void @rune_print_int(i64 {v})");
                        }
                        None => {
                            let _ = writeln!(out, "  call void @rune_print_int(i64 0)");
                        }
                    }
                } else {
                    let params = emitted
                        .iter()
                        .map(|(v, t)| format!("{} {v}", llvm_type(t)))
                        .collect::<Vec<_>>()
                        .join(", ");
                    match llvm_type(ty) {
                        "void" => {
                            let _ = writeln!(out, "  call void @f_{name}({params})");
                        }
                        ret => {
                            let reg = self.fresh();
                            let _ = writeln!(out, "  {reg} = call {ret} @f_{name}({params})");
                            if let Some(dest) = dest {
                                let _ = writeln!(
                                    out,
                                    "  store {ret} {reg}, ptr {}",
                                    Self::local_ref(dest)
                                );
                            }
                        }
                    }
                }
            }
            Instruction::CallIndirect {
                dest, callee, args, ..
            } => {
                // Function values are closures: the code word is the first
                // field of the block. Call it with the closure itself as the
                // environment.
                let callee_ref = match callee {
                    Value::Local(name) => Self::local_ref(name),
                    other => unreachable!("non-local callee {other:?} in CallIndirect"),
                };
                let (mut param_tys, ret) = match value_type(callee, self.locals) {
                    Type::Function {
                        params,
                        return_type,
                    } => (params, *return_type),
                    other => unreachable!("indirect callee has non-function type {other:?}"),
                };
                let closure = self.fresh();
                let _ = writeln!(out, "  {closure} = load ptr, ptr {callee_ref}");
                let code = self.fresh();
                let _ = writeln!(out, "  {code} = load ptr, ptr {closure}");
                let emitted = args
                    .iter()
                    .map(|a| {
                        let v = self.value(out, a);
                        format!("{} {v}", llvm_type(&param_tys.remove(0)))
                    })
                    .collect::<Vec<_>>();
                // A void call has no result to name (LLVM forbids naming a
                // void instruction); a valued call stores its result when
                // one is wanted.
                match llvm_type(&ret) {
                    "void" => {
                        let _ = writeln!(
                            out,
                            "  call void {code}(ptr {closure}{})",
                            if emitted.is_empty() {
                                String::new()
                            } else {
                                format!(", {}", emitted.join(", "))
                            }
                        );
                    }
                    ret_ty => {
                        let call = self.fresh();
                        let _ = writeln!(
                            out,
                            "  {call} = call {ret_ty} {code}(ptr {closure}{})",
                            if emitted.is_empty() {
                                String::new()
                            } else {
                                format!(", {}", emitted.join(", "))
                            }
                        );
                        if let Some(dest) = dest {
                            let _ = writeln!(
                                out,
                                "  store {ret_ty} {call}, ptr {}",
                                Self::local_ref(dest)
                            );
                        }
                    }
                }
            }
            Instruction::MakeClosure {
                dest,
                func,
                captures,
                ..
            } => {
                // Allocate a closure block: [wrapper code, capture values...],
                // mirroring the C backend's rune_closure_new/rune_closure_set.
                let n_caps = self
                    .targets
                    .iter()
                    .find(|(name, _)| name == func)
                    .map_or(captures.len(), |(_, n)| *n);
                let closure = self.fresh();
                let _ = writeln!(
                    out,
                    "  {closure} = call ptr @rune_closure_new(ptr @f_rune_closure_{func}, i32 {n_caps})"
                );
                let _ = writeln!(out, "  store ptr {closure}, ptr {}", Self::local_ref(dest));
                for (idx, cap) in captures.iter().enumerate() {
                    let v = self.value(out, cap);
                    let cap_ty = value_type(cap, self.locals);
                    let wide = self.widen_to_i64(out, &v, &cap_ty);
                    let _ = writeln!(
                        out,
                        "  call void @rune_closure_set(ptr {closure}, i32 {idx}, i64 {wide})"
                    );
                }
            }
            Instruction::MakeArray { dest, elems, .. } => {
                // Allocate an array block of [length, elems...] 8-byte slots,
                // mirroring the C runtime's rune_arr_new/rune_arr_set.
                let arr = self.fresh();
                let _ = writeln!(out, "  {arr} = call ptr @rune_arr_new(i64 {})", elems.len());
                let _ = writeln!(out, "  store ptr {arr}, ptr {}", Self::local_ref(dest));
                for (idx, elem) in elems.iter().enumerate() {
                    let v = self.value(out, elem);
                    let elem_ty = value_type(elem, self.locals);
                    let wide = self.widen_to_i64(out, &v, &elem_ty);
                    let _ = writeln!(
                        out,
                        "  call void @rune_arr_set(ptr {arr}, i64 {idx}, i64 {wide})"
                    );
                }
            }
            Instruction::IndexGet {
                dest,
                array,
                index,
                ty,
            } => {
                // Bounds-checked read returning an i64 slot; narrow back to
                // the element's LLVM type (pointers come back via inttoptr).
                let a = self.value(out, array);
                let i = self.value(out, index);
                let raw = self.fresh();
                let _ = writeln!(out, "  {raw} = call i64 @rune_arr_get(ptr {a}, i64 {i})");
                let narrowed = match ty {
                    Type::Int => raw,
                    Type::Bool => {
                        let r = self.fresh();
                        let _ = writeln!(out, "  {r} = trunc i64 {raw} to i1");
                        r
                    }
                    _ => {
                        let r = self.fresh();
                        let _ = writeln!(out, "  {r} = inttoptr i64 {raw} to ptr");
                        r
                    }
                };
                let _ = writeln!(
                    out,
                    "  store {} {narrowed}, ptr {}",
                    llvm_type(ty),
                    Self::local_ref(dest)
                );
            }
            Instruction::IndexSet {
                array,
                index,
                value,
                ty,
            } => {
                let a = self.value(out, array);
                let i = self.value(out, index);
                let v = self.value(out, value);
                let wide = self.widen_to_i64(out, &v, ty);
                let _ = writeln!(
                    out,
                    "  call void @rune_arr_set(ptr {a}, i64 {i}, i64 {wide})"
                );
            }
            Instruction::ArrLen { dest, array, .. } => {
                let a = self.value(out, array);
                let len = self.fresh();
                let _ = writeln!(out, "  {len} = call i64 @rune_arr_len(ptr {a})");
                let _ = writeln!(out, "  store i64 {len}, ptr {}", Self::local_ref(dest));
            }
        }
    }

    fn terminator(&mut self, out: &mut String, term: &Terminator) {
        match term {
            Terminator::Return(Some(value)) => {
                let v = self.value(out, value);
                let ty = value_type(value, self.locals);
                let _ = writeln!(out, "  ret {} {v}", llvm_type(&ty));
            }
            Terminator::Return(None) => {
                // Falling off the end returns zero, matching the C
                // backend (and satisfying functions declared non-void).
                if self.return_type == Type::Void {
                    out.push_str("  ret void\n");
                } else {
                    let _ = writeln!(out, "  ret {} 0", llvm_type(&self.return_type));
                }
            }
            Terminator::Jump(target) => {
                let _ = writeln!(out, "  br label %L{target}");
            }
            Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                let c = self.value(out, condition);
                let _ = writeln!(
                    out,
                    "  br i1 {c}, label %L{then_block}, label %L{else_block}"
                );
            }
        }
    }
}

/// The static type of an IR value, from its constant kind or the function's
/// local table (locals are typed by `local_types`).
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

/// Comparison operators map to signed icmp predicates on i64 operands.
fn icmp_op(op: &BinOp) -> &'static str {
    match op {
        BinOp::Eq => "eq",
        BinOp::NotEq => "ne",
        BinOp::Lt => "slt",
        BinOp::Le => "sle",
        BinOp::Gt => "sgt",
        BinOp::Ge => "sge",
        _ => unreachable!("non-comparison operator in icmp"),
    }
}

/// Non-dividing arithmetic on i64 operands; division and remainder go
/// through the runtime helpers (see the Binary emission).
fn arith_op(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        _ => unreachable!("non-arithmetic operator in arith"),
    }
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
                }
                | Instruction::CallIndirect {
                    dest: Some(dest), ..
                }
                | Instruction::MakeClosure { dest, .. }
                | Instruction::MakeArray { dest, .. }
                | Instruction::IndexGet { dest, .. }
                | Instruction::ArrLen { dest, .. } => dest,
                Instruction::Call { dest: None, .. }
                | Instruction::CallIndirect { dest: None, .. }
                | Instruction::IndexSet { .. } => continue,
            };
            if !types.iter().any(|(n, _)| n == dest) {
                let ty = match inst {
                    Instruction::Binary { ty, .. }
                    | Instruction::Unary { ty, .. }
                    | Instruction::Move { ty, .. }
                    | Instruction::Call {
                        dest: Some(_), ty, ..
                    }
                    | Instruction::CallIndirect {
                        dest: Some(_), ty, ..
                    }
                    | Instruction::MakeClosure { ty, .. }
                    | Instruction::MakeArray { ty, .. }
                    | Instruction::IndexGet { ty, .. }
                    | Instruction::ArrLen { ty, .. } => ty.clone(),
                    Instruction::Call { dest: None, .. }
                    | Instruction::CallIndirect { dest: None, .. }
                    | Instruction::IndexSet { .. } => continue,
                };
                types.push((dest.clone(), ty));
            }
        }
    }
    types
}
