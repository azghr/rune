// SPDX-License-Identifier: MIT

//! Lowers the checked AST into the three-address IR.
//!
//! Every expression becomes a chain of instructions ending in a single
//! `Value`, and control flow becomes basic blocks connected by `Jump` and
//! `Branch` terminators. Logical operators lower to short-circuit control
//! flow so side effects behave the same as they would in C.

use crate::language::ast::{Block, Expr, Function as AstFunction, Param, Program, Stmt, Type};
use crate::language::token::TokenKind;
use crate::transform::ir::Function as IrFunction;
use crate::transform::ir::*;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};

/// Lowers a checked `Program` into an IR `Module`.
pub fn lower_program(program: &Program) -> Module {
    // The function type of every declared function, used to type call
    // expressions and function references.
    let functions: HashMap<String, Type> = program
        .functions
        .iter()
        .map(|f| {
            (
                f.name.clone(),
                Type::Function {
                    params: f.params.iter().map(|p| p.ty.clone()).collect(),
                    return_type: Box::new(f.return_type.clone().unwrap_or(Type::Void)),
                },
            )
        })
        .collect();

    // Anonymous functions are hoisted into synthesized top-level functions.
    // Lambda bodies can contain further lambdas, so lowering works through a
    // worklist that grows as lambdas are discovered.
    let counter = Cell::new(0usize);
    let mut worklist: Vec<AstFunction> = program.functions.clone();
    let mut lowered = Vec::new();
    let mut next = 0;
    while next < worklist.len() {
        let func = worklist[next].clone();
        lowered.push(lower_function(&func, &functions, &counter, &mut worklist));
        next += 1;
    }
    Module { functions: lowered }
}

/// Builds one function's basic blocks, keeping an "open" block to append
/// instructions to.
struct FuncBuilder {
    blocks: Vec<BasicBlock>,
    /// Instructions of the currently open block (not yet stored).
    pending: Vec<Instruction>,
    /// Index of the currently open block in `blocks`, or None if the last
    /// block was terminated and no instruction has opened a new one.
    open: Option<usize>,
    next_temp: usize,
    /// All variable names declared in the function, so generated temp names
    /// never collide with user variables.
    taken_names: HashSet<String>,
    /// Innermost loop targets: (break target, continue target).
    loops: Vec<(usize, usize)>,
}

impl FuncBuilder {
    fn new(params: &[Param], body: &Block) -> Self {
        let mut taken_names = HashSet::new();
        for param in params {
            taken_names.insert(param.name.clone());
        }
        collect_declared_names(&body.statements, &mut taken_names);

        Self {
            blocks: vec![BasicBlock {
                id: 0,
                instructions: Vec::new(),
                terminator: Terminator::Return(None),
            }],
            pending: Vec::new(),
            open: Some(0),
            next_temp: 0,
            taken_names,
            loops: Vec::new(),
        }
    }

    fn push(&mut self, inst: Instruction) {
        if self.open.is_none() {
            // Dead code after a terminator: open a fresh block for it.
            let id = self.reserve_block();
            self.open = Some(id);
        }
        self.pending.push(inst);
    }

    /// Appends an empty block without making it the open one. Use when the
    /// ids of several blocks are needed before control flow can be emitted.
    fn reserve_block(&mut self) -> usize {
        let id = self.blocks.len();
        self.blocks.push(BasicBlock {
            id,
            instructions: Vec::new(),
            terminator: Terminator::Return(None),
        });
        id
    }

    /// Opens a reserved block, flushing any pending instructions of the
    /// previously open block first.
    fn switch_to(&mut self, id: usize) {
        if let Some(prev) = self.open {
            let pending = std::mem::take(&mut self.pending);
            self.blocks[prev].instructions = pending;
        }
        self.open = Some(id);
    }

    /// Seals the open block with `terminator`.
    fn terminate(&mut self, terminator: Terminator) {
        match self.open {
            Some(open) => {
                let pending = std::mem::take(&mut self.pending);
                self.blocks[open].instructions = pending;
                self.blocks[open].terminator = terminator;
                self.open = None;
            }
            None => {
                // Terminating after a terminator: seal a fresh dead block so
                // block indices stay valid.
                let id = self.reserve_block();
                self.blocks[id].terminator = terminator;
            }
        }
    }

    /// Jumps to `target` only if the open block has not already been
    /// terminated (e.g. by a `return`). A terminated arm contributes no
    /// fall-through edge.
    fn jump_if_open(&mut self, target: usize) {
        if self.open.is_some() {
            self.terminate(Terminator::Jump(target));
        }
    }

    fn fresh_temp(&mut self) -> String {
        loop {
            let name = format!("t{}", self.next_temp);
            self.next_temp += 1;
            if self.taken_names.insert(name.clone()) {
                return name;
            }
        }
    }

    fn finish(mut self) -> Vec<BasicBlock> {
        self.switch_to_open_flush();
        for (id, block) in self.blocks.iter_mut().enumerate() {
            block.id = id;
        }
        self.blocks
    }

    fn switch_to_open_flush(&mut self) {
        if let Some(prev) = self.open {
            let pending = std::mem::take(&mut self.pending);
            self.blocks[prev].instructions = pending;
            self.open = None;
        }
    }
}

fn collect_declared_names(statements: &[Stmt], names: &mut HashSet<String>) {
    for stmt in statements {
        match stmt {
            Stmt::Let { name, .. } => {
                names.insert(name.clone());
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_declared_names(&then_branch.statements, names);
                if let Some(b) = else_branch {
                    collect_declared_names(&b.statements, names);
                }
            }
            Stmt::While { body, .. } => collect_declared_names(&body.statements, names),
            Stmt::Block(b) => collect_declared_names(&b.statements, names),
            _ => {}
        }
    }
}

fn lower_function(
    func: &AstFunction,
    function_returns: &HashMap<String, Type>,
    lambda_counter: &Cell<usize>,
    worklist: &mut Vec<AstFunction>,
) -> IrFunction {
    let return_type = func.return_type.clone().unwrap_or(Type::Void);
    let mut builder = FuncBuilder::new(&func.params, &func.body);

    let mut lowerer = ExprLowerer {
        scopes: vec![
            func.params
                .iter()
                .map(|p| (p.name.clone(), p.ty.clone()))
                .collect(),
        ],
        function_returns: function_returns.clone(),
        lambda_counter,
        worklist,
    };

    lower_statements(&mut builder, &mut lowerer, &func.body.statements);
    // An unterminated function falls through: return nothing (main is
    // special-cased to `return 0` by the emitter).
    if builder.open.is_some() {
        builder.terminate(Terminator::Return(None));
    }

    IrFunction {
        name: func.name.clone(),
        params: func
            .params
            .iter()
            .map(|p| (p.name.clone(), p.ty.clone()))
            .collect(),
        return_type,
        body: builder.finish(),
    }
}

/// Type environment for expression typing during lowering. The checker has
/// already validated the program; this only recovers the types the IR needs.
struct ExprLowerer<'a> {
    scopes: Vec<HashMap<String, Type>>,
    function_returns: HashMap<String, Type>,
    /// Program-wide counter giving hoisted lambdas unique names.
    lambda_counter: &'a Cell<usize>,
    /// Hoisted lambda functions are appended here; `lower_program` lowers
    /// them in turn.
    worklist: &'a mut Vec<AstFunction>,
}

impl ExprLowerer<'_> {
    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str, ty: Type) {
        if let Some(top) = self.scopes.last_mut() {
            top.insert(name.to_string(), ty);
        }
    }

    fn var_type(&self, name: &str) -> Type {
        self.local_type(name).unwrap_or(Type::Int)
    }

    /// The type of a local variable, if one is in scope.
    fn local_type(&self, name: &str) -> Option<Type> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    fn call_type(&self, callee: &str) -> Type {
        if callee == "print" {
            return Type::Void;
        }
        // Calling through a function-typed local returns its return type.
        if let Some(Type::Function { return_type, .. }) = self
            .scopes
            .iter()
            .rev()
            .find_map(|s| s.get(callee).cloned())
        {
            return *return_type;
        }
        if let Some(Type::Function { return_type, .. }) = self.function_returns.get(callee) {
            return *return_type.clone();
        }
        Type::Int
    }
}

fn bin_op_result(op: &TokenKind) -> Type {
    match op {
        TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Percent => Type::Int,
        _ => Type::Bool,
    }
}

/// Maps an expression or compound-assignment operator to its IR binary op.
fn bin_op(op: &TokenKind) -> BinOp {
    match op {
        TokenKind::Plus | TokenKind::PlusEq => BinOp::Add,
        TokenKind::Minus | TokenKind::MinusEq => BinOp::Sub,
        TokenKind::Star | TokenKind::StarEq => BinOp::Mul,
        TokenKind::Slash | TokenKind::SlashEq => BinOp::Div,
        TokenKind::Percent | TokenKind::PercentEq => BinOp::Mod,
        TokenKind::EqEq => BinOp::Eq,
        TokenKind::BangEq => BinOp::NotEq,
        TokenKind::Less => BinOp::Lt,
        TokenKind::LessEq => BinOp::Le,
        TokenKind::Greater => BinOp::Gt,
        TokenKind::GreaterEq => BinOp::Ge,
        TokenKind::AmpAmp => BinOp::And,
        TokenKind::PipePipe => BinOp::Or,
        _ => unreachable!("checker rejected invalid binary operator"),
    }
}

fn lower_statements(builder: &mut FuncBuilder, lowerer: &mut ExprLowerer, statements: &[Stmt]) {
    for stmt in statements {
        lower_statement(builder, lowerer, stmt);
    }
}

fn lower_statement(builder: &mut FuncBuilder, lowerer: &mut ExprLowerer, stmt: &Stmt) {
    match stmt {
        Stmt::Let {
            name, ty, value, ..
        } => {
            let declared = ty.clone();
            let (value, inferred) = lower_expr(builder, lowerer, value);
            let var_ty = declared.unwrap_or(inferred);
            lowerer.declare(name, var_ty.clone());
            builder.push(Instruction::Move {
                dest: name.clone(),
                value,
                ty: var_ty,
            });
        }
        Stmt::Assign {
            name, op, value, ..
        } => {
            let var_ty = lowerer.var_type(name);
            let (value, value_ty) = lower_expr(builder, lowerer, value);
            if *op == TokenKind::Eq {
                builder.push(Instruction::Move {
                    dest: name.clone(),
                    value,
                    ty: var_ty,
                });
            } else {
                // `x <op>= value` becomes `x = x <op> value`. String `+`
                // is concatenation and yields a string.
                let result_ty = if matches!(op, TokenKind::Plus | TokenKind::PlusEq)
                    && (var_ty == Type::String || value_ty == Type::String)
                {
                    Type::String
                } else {
                    Type::Int
                };
                let temp = builder.fresh_temp();
                builder.push(Instruction::Binary {
                    dest: temp.clone(),
                    left: Value::Local(name.clone()),
                    op: bin_op(op),
                    right: value,
                    ty: result_ty,
                });
                builder.push(Instruction::Move {
                    dest: name.clone(),
                    value: Value::Local(temp),
                    ty: var_ty,
                });
            }
        }
        Stmt::IndexAssign {
            array,
            index,
            op,
            value,
            ..
        } => {
            // The array expression is evaluated exactly once.
            let (array, array_ty) = lower_expr(builder, lowerer, array);
            let (index, _) = lower_expr(builder, lowerer, index);
            let Type::Array(elem_ty) = array_ty else {
                unreachable!("checker validated the index target is an array");
            };
            if *op == TokenKind::Eq {
                let (value, _) = lower_expr(builder, lowerer, value);
                builder.push(Instruction::IndexSet {
                    array,
                    index,
                    value,
                    ty: (*elem_ty).clone(),
                });
            } else {
                // `a[i] <op>= v` becomes `a[i] = a[i] <op> v`:
                // read, combine, write.
                let (value, _) = lower_expr(builder, lowerer, value);
                let read = builder.fresh_temp();
                builder.push(Instruction::IndexGet {
                    dest: read.clone(),
                    array: array.clone(),
                    index: index.clone(),
                    ty: (*elem_ty).clone(),
                });
                let result_ty = if *op == TokenKind::PlusEq && *elem_ty == Type::String {
                    // String `+` is concatenation; the IR binary's result
                    // type tracks string-ness.
                    Type::String
                } else {
                    Type::Int
                };
                let combined = builder.fresh_temp();
                builder.push(Instruction::Binary {
                    dest: combined.clone(),
                    left: Value::Local(read),
                    op: bin_op(op),
                    right: value,
                    ty: result_ty,
                });
                builder.push(Instruction::IndexSet {
                    array,
                    index,
                    value: Value::Local(combined),
                    ty: (*elem_ty).clone(),
                });
            }
        }
        Stmt::Expr(expr) => {
            // Statement expressions only need the side effects.
            match expr {
                Expr::Call { .. } => {
                    lower_call(builder, lowerer, expr, false);
                }
                other => {
                    lower_expr(builder, lowerer, other);
                }
            }
        }
        Stmt::Return { value, .. } => {
            let value = value.as_ref().map(|e| lower_expr(builder, lowerer, e).0);
            builder.terminate(Terminator::Return(value));
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let (cond, _) = lower_expr(builder, lowerer, condition);
            let then_id = builder.reserve_block();
            let else_id = builder.reserve_block();
            let merge_id = builder.reserve_block();
            builder.terminate(Terminator::Branch {
                condition: cond,
                then_block: then_id,
                else_block: else_id,
            });

            builder.switch_to(then_id);
            lowerer.enter_scope();
            lower_statements(builder, lowerer, &then_branch.statements);
            lowerer.exit_scope();
            builder.jump_if_open(merge_id);

            builder.switch_to(else_id);
            if let Some(else_b) = else_branch {
                lowerer.enter_scope();
                lower_statements(builder, lowerer, &else_b.statements);
                lowerer.exit_scope();
            }
            builder.jump_if_open(merge_id);

            builder.switch_to(merge_id);
        }
        Stmt::While {
            condition, body, ..
        } => {
            let cond_id = builder.reserve_block();
            builder.terminate(Terminator::Jump(cond_id));

            builder.switch_to(cond_id);
            let (cond, _) = lower_expr(builder, lowerer, condition);
            let body_id = builder.reserve_block();
            let exit_id = builder.reserve_block();
            builder.terminate(Terminator::Branch {
                condition: cond,
                then_block: body_id,
                else_block: exit_id,
            });

            builder.switch_to(body_id);
            lowerer.enter_scope();
            builder.loops.push((exit_id, cond_id));
            lower_statements(builder, lowerer, &body.statements);
            builder.loops.pop();
            lowerer.exit_scope();
            builder.jump_if_open(cond_id);

            builder.switch_to(exit_id);
        }
        Stmt::For {
            init,
            condition,
            step,
            body,
            ..
        } => {
            // init, condition, and step share one scope around the loop.
            lowerer.enter_scope();
            if let Some(init) = init {
                lower_statement(builder, lowerer, init);
            }

            let cond_id = builder.reserve_block();
            builder.terminate(Terminator::Jump(cond_id));
            builder.switch_to(cond_id);
            let (cond, _) = match condition {
                Some(cond) => lower_expr(builder, lowerer, cond),
                None => (Value::ConstBool(true), Type::Bool),
            };
            let body_id = builder.reserve_block();
            let step_id = builder.reserve_block();
            let exit_id = builder.reserve_block();
            builder.terminate(Terminator::Branch {
                condition: cond,
                then_block: body_id,
                else_block: exit_id,
            });

            // `continue` runs the step before re-testing the condition,
            // matching C semantics.
            builder.switch_to(body_id);
            lowerer.enter_scope();
            builder.loops.push((exit_id, step_id));
            lower_statements(builder, lowerer, &body.statements);
            builder.loops.pop();
            lowerer.exit_scope();
            builder.jump_if_open(step_id);

            builder.switch_to(step_id);
            if let Some(step) = step {
                lower_statement(builder, lowerer, step);
            }
            builder.jump_if_open(cond_id);

            builder.switch_to(exit_id);
            lowerer.exit_scope();
        }
        Stmt::Break { .. } => {
            // The checker guarantees a loop is active.
            let (break_target, _) = *builder.loops.last().unwrap();
            builder.terminate(Terminator::Jump(break_target));
        }
        Stmt::Continue { .. } => {
            let (_, continue_target) = *builder.loops.last().unwrap();
            builder.terminate(Terminator::Jump(continue_target));
        }
        Stmt::Block(block) => {
            lowerer.enter_scope();
            lower_statements(builder, lowerer, &block.statements);
            lowerer.exit_scope();
        }
    }
}

fn lower_expr(builder: &mut FuncBuilder, lowerer: &mut ExprLowerer, expr: &Expr) -> (Value, Type) {
    match expr {
        Expr::Int { value, .. } => (Value::ConstInt(*value), Type::Int),
        Expr::Bool { value, .. } => (Value::ConstBool(*value), Type::Bool),
        Expr::String { value, .. } => (Value::ConstString(value.clone()), Type::String),
        Expr::Variable { name, .. } => {
            // A declared function used as a value becomes a closure over
            // it (no captures); locals pass through unchanged.
            if lowerer.local_type(name).is_none()
                && let Some(ty) = lowerer.function_returns.get(name)
            {
                let dest = builder.fresh_temp();
                builder.push(Instruction::MakeClosure {
                    dest: dest.clone(),
                    func: name.clone(),
                    captures: Vec::new(),
                    ty: ty.clone(),
                });
                (Value::Local(dest), ty.clone())
            } else {
                let ty = lowerer.var_type(name);
                (Value::Local(name.clone()), ty)
            }
        }
        Expr::Grouping { expr, .. } => lower_expr(builder, lowerer, expr),
        Expr::Unary {
            op, expr: inner, ..
        } => {
            let (value, _) = lower_expr(builder, lowerer, inner);
            let ty = match op {
                TokenKind::Minus => Type::Int,
                _ => Type::Bool,
            };
            let dest = builder.fresh_temp();
            builder.push(Instruction::Unary {
                dest: dest.clone(),
                op: match op {
                    TokenKind::Minus => UnaryOp::Neg,
                    _ => UnaryOp::Not,
                },
                value,
                ty: ty.clone(),
            });
            (Value::Local(dest), ty)
        }
        Expr::Binary {
            op, left, right, ..
        } => {
            // Logical operators short-circuit, which needs control flow
            // rather than a single instruction.
            if matches!(op, TokenKind::AmpAmp | TokenKind::PipePipe) {
                let (result, _) =
                    lower_short_circuit(builder, lowerer, left, right, *op == TokenKind::PipePipe);
                return (result, Type::Bool);
            }

            let (left, left_ty) = lower_expr(builder, lowerer, left);
            let (right, right_ty) = lower_expr(builder, lowerer, right);
            // `+` on strings is concatenation and yields a string;
            // comparisons always yield bools.
            let ty = if matches!(
                op,
                TokenKind::Plus
                    | TokenKind::Minus
                    | TokenKind::Star
                    | TokenKind::Slash
                    | TokenKind::Percent
            ) && (left_ty == Type::String || right_ty == Type::String)
            {
                Type::String
            } else {
                bin_op_result(op)
            };
            let dest = builder.fresh_temp();
            builder.push(Instruction::Binary {
                dest: dest.clone(),
                left,
                op: bin_op(op),
                right,
                ty: ty.clone(),
            });
            (Value::Local(dest), ty)
        }
        Expr::ArrayLiteral { elems, .. } => {
            // Allocate a heap block of [length, elems...] slots; the checker
            // guarantees at least one element and a uniform element type.
            let mut values = Vec::with_capacity(elems.len());
            let mut elem_ty = Type::Void;
            for (i, elem) in elems.iter().enumerate() {
                let (value, ty) = lower_expr(builder, lowerer, elem);
                if i == 0 {
                    elem_ty = ty;
                }
                values.push(value);
            }
            let ty = Type::Array(Box::new(elem_ty));
            let dest = builder.fresh_temp();
            builder.push(Instruction::MakeArray {
                dest: dest.clone(),
                elems: values,
                ty: ty.clone(),
            });
            (Value::Local(dest), ty)
        }
        Expr::Index { array, index, .. } => {
            let (array, array_ty) = lower_expr(builder, lowerer, array);
            let (index, _) = lower_expr(builder, lowerer, index);
            let Type::Array(elem_ty) = array_ty else {
                unreachable!("checker validated the indexed value is an array");
            };
            let dest = builder.fresh_temp();
            builder.push(Instruction::IndexGet {
                dest: dest.clone(),
                array,
                index,
                ty: *elem_ty.clone(),
            });
            (Value::Local(dest), *elem_ty)
        }
        Expr::Call { .. } => lower_call(builder, lowerer, expr, true),
        Expr::AnonymousFunction {
            params,
            return_type,
            body,
            ..
        } => {
            // Hoist the body into a synthesized top-level function whose
            // parameters are the user's parameters followed by the captured
            // variables (by value); the expression builds a closure over it.
            let captures = collect_captures(body, params, lowerer);
            let name = format!("rune_lambda_{}", lowerer.lambda_counter.get());
            lowerer.lambda_counter.set(lowerer.lambda_counter.get() + 1);
            let ty = Type::Function {
                params: params.iter().map(|p| p.ty.clone()).collect(),
                return_type: Box::new(return_type.clone().unwrap_or(Type::Void)),
            };
            let mut hoisted_params = params.clone();
            for (cap_name, cap_ty) in &captures {
                hoisted_params.push(Param {
                    name: cap_name.clone(),
                    ty: cap_ty.clone(),
                    span: expr.span(),
                });
            }
            lowerer.worklist.push(AstFunction {
                name: name.clone(),
                params: hoisted_params,
                return_type: return_type.clone(),
                body: body.clone(),
                span: expr.span(),
            });
            let dest = builder.fresh_temp();
            builder.push(Instruction::MakeClosure {
                dest: dest.clone(),
                func: name,
                captures: captures
                    .into_iter()
                    .map(|(name, _)| Value::Local(name))
                    .collect(),
                ty: ty.clone(),
            });
            (Value::Local(dest), ty)
        }
    }
}

/// Lowers `left && right` (or `left || right` when `is_or`) with proper
/// short-circuit control flow, producing a bool local holding the result.
fn lower_short_circuit(
    builder: &mut FuncBuilder,
    lowerer: &mut ExprLowerer,
    left: &Expr,
    right: &Expr,
    is_or: bool,
) -> (Value, Type) {
    let (left, _) = lower_expr(builder, lowerer, left);
    let dest = builder.fresh_temp();

    let eval_right = builder.reserve_block();
    let short_circuit = builder.reserve_block();
    let end = builder.reserve_block();

    // For `&&` the right side runs when the left is true; for `||` when the
    // left is false.
    let (then_block, else_block) = if is_or {
        (short_circuit, eval_right)
    } else {
        (eval_right, short_circuit)
    };
    builder.terminate(Terminator::Branch {
        condition: left,
        then_block,
        else_block,
    });

    builder.switch_to(eval_right);
    let (right, _) = lower_expr(builder, lowerer, right);
    builder.push(Instruction::Move {
        dest: dest.clone(),
        value: right,
        ty: Type::Bool,
    });
    builder.terminate(Terminator::Jump(end));

    builder.switch_to(short_circuit);
    builder.push(Instruction::Move {
        dest: dest.clone(),
        value: Value::ConstBool(is_or),
        ty: Type::Bool,
    });
    builder.terminate(Terminator::Jump(end));

    builder.switch_to(end);
    (Value::Local(dest), Type::Bool)
}

/// Lowers a call expression. In value position (`want_value`) a non-void
/// call writes its result to a temp; statement-position calls get no dest.
fn lower_call(
    builder: &mut FuncBuilder,
    lowerer: &mut ExprLowerer,
    expr: &Expr,
    want_value: bool,
) -> (Value, Type) {
    let Expr::Call { callee, args, .. } = expr else {
        unreachable!("lower_call only handles calls");
    };
    let mut values = Vec::new();
    for arg in args {
        values.push(lower_expr(builder, lowerer, arg).0);
    }

    // A plain-name callee resolves to print, a local, or a declared
    // function; anything else is a function value produced by an
    // expression (e.g. a chained call) and is called indirectly.
    let callee_name = match callee.as_ref() {
        Expr::Variable { name, .. } => Some(name.clone()),
        _ => None,
    };

    // `len(a)` is a builtin over arrays, like `print` is over printable
    // values; it lowers to its own instruction instead of a call.
    if callee_name.as_deref() == Some("len") {
        let dest = builder.fresh_temp();
        builder.push(Instruction::ArrLen {
            dest: dest.clone(),
            array: values.first().cloned().unwrap_or(Value::ConstInt(0)),
            ty: Type::Int,
        });
        return (Value::Local(dest), Type::Int);
    }
    // The callee is lowered at most once, here: a plain name needs no value
    // unless it resolves to a function-typed local, any other expression
    // produces the function value to call.
    let (callee_value, ty) = match &callee_name {
        Some(name) => (None, lowerer.call_type(name)),
        None => {
            let (value, callee_ty) = lower_expr(builder, lowerer, callee);
            let ty = match callee_ty {
                Type::Function { return_type, .. } => *return_type,
                _ => Type::Int,
            };
            (Some(value), ty)
        }
    };
    let dest = if want_value && ty != Type::Void {
        Some(builder.fresh_temp())
    } else {
        None
    };

    // A local holding a function value is an indirect call; a declared
    // function is a direct one.
    let callee_is_local = callee_name.as_ref().is_some_and(|name| {
        name != "print"
            && lowerer.scopes.iter().rev().any(|s| {
                s.get(name)
                    .is_some_and(|ty| matches!(ty, Type::Function { .. }))
            })
    });
    match callee_name {
        Some(name) if !callee_is_local => {
            builder.push(Instruction::Call {
                dest: dest.clone(),
                name,
                args: values,
                ty: ty.clone(),
            });
        }
        Some(name) => {
            builder.push(Instruction::CallIndirect {
                dest: dest.clone(),
                callee: Value::Local(name),
                args: values,
                ty: ty.clone(),
            });
        }
        None => {
            builder.push(Instruction::CallIndirect {
                dest: dest.clone(),
                callee: callee_value.expect("expression callee must produce a value"),
                args: values,
                ty: ty.clone(),
            });
        }
    }
    match dest {
        Some(d) => (Value::Local(d), ty),
        // The checker never lets a void call appear in value position; this
        // dummy keeps the type system happy.
        None => (Value::ConstBool(false), Type::Void),
    }
}

/// The free variables of a lambda body: names it reads that are neither its
/// parameters, locally declared, nor declared functions — with their types
/// from the enclosing scope, in first-use order. Captures are by value.
fn collect_captures(
    body: &Block,
    params: &[Param],
    lowerer: &ExprLowerer<'_>,
) -> Vec<(String, Type)> {
    let mut declared: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
    collect_declared_names(&body.statements, &mut declared);

    let mut captures: Vec<(String, Type)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    walk_capture_reads(
        &body.statements,
        &declared,
        lowerer,
        &mut captures,
        &mut seen,
    );
    captures
}

fn walk_capture_reads(
    statements: &[Stmt],
    declared: &HashSet<String>,
    lowerer: &ExprLowerer<'_>,
    captures: &mut Vec<(String, Type)>,
    seen: &mut HashSet<String>,
) {
    for stmt in statements {
        match stmt {
            Stmt::Let { value, .. }
            | Stmt::Return {
                value: Some(value), ..
            } => {
                walk_capture_expr(value, declared, lowerer, captures, seen);
            }
            Stmt::Assign { value, .. } => {
                walk_capture_expr(value, declared, lowerer, captures, seen);
            }
            Stmt::IndexAssign {
                array,
                index,
                value,
                ..
            } => {
                // Reading the array and the index captures them (the array
                // by pointer); the written value too.
                walk_capture_expr(array, declared, lowerer, captures, seen);
                walk_capture_expr(index, declared, lowerer, captures, seen);
                walk_capture_expr(value, declared, lowerer, captures, seen);
            }
            Stmt::Expr(expr) => {
                walk_capture_expr(expr, declared, lowerer, captures, seen);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                walk_capture_expr(condition, declared, lowerer, captures, seen);
                walk_capture_reads(&then_branch.statements, declared, lowerer, captures, seen);
                if let Some(b) = else_branch {
                    walk_capture_reads(&b.statements, declared, lowerer, captures, seen);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                walk_capture_expr(condition, declared, lowerer, captures, seen);
                walk_capture_reads(&body.statements, declared, lowerer, captures, seen);
            }
            Stmt::For {
                init,
                condition,
                step,
                body,
                ..
            } => {
                if let Some(init) = init {
                    walk_capture_reads(
                        std::slice::from_ref(init),
                        declared,
                        lowerer,
                        captures,
                        seen,
                    );
                }
                if let Some(cond) = condition {
                    walk_capture_expr(cond, declared, lowerer, captures, seen);
                }
                if let Some(step) = step {
                    walk_capture_reads(
                        std::slice::from_ref(step),
                        declared,
                        lowerer,
                        captures,
                        seen,
                    );
                }
                walk_capture_reads(&body.statements, declared, lowerer, captures, seen);
            }
            Stmt::Block(block) => {
                walk_capture_reads(&block.statements, declared, lowerer, captures, seen);
            }
            Stmt::Return { value: None, .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
        }
    }
}

fn walk_capture_expr(
    expr: &Expr,
    declared: &HashSet<String>,
    lowerer: &ExprLowerer<'_>,
    captures: &mut Vec<(String, Type)>,
    seen: &mut HashSet<String>,
) {
    match expr {
        Expr::Variable { name, span } => {
            // Skip names declared inside the lambda and declared functions.
            if declared.contains(name) || lowerer.function_returns.contains_key(name) {
                return;
            }
            if seen.contains(name) {
                return;
            }
            // A capture only if the enclosing scope declares it; anything
            // else stays the checker's `undefined variable` problem.
            if let Some(ty) = lowerer
                .scopes
                .iter()
                .rev()
                .find_map(|s| s.get(name).cloned())
            {
                seen.insert(name.clone());
                captures.push((name.clone(), ty));
            } else {
                let _ = span;
            }
        }
        Expr::Grouping { expr, .. } => {
            walk_capture_expr(expr, declared, lowerer, captures, seen);
        }
        Expr::Unary { expr, .. } => {
            walk_capture_expr(expr, declared, lowerer, captures, seen);
        }
        Expr::Binary { left, right, .. } => {
            walk_capture_expr(left, declared, lowerer, captures, seen);
            walk_capture_expr(right, declared, lowerer, captures, seen);
        }
        Expr::Call { callee, args, .. } => {
            walk_capture_expr(callee, declared, lowerer, captures, seen);
            for arg in args {
                walk_capture_expr(arg, declared, lowerer, captures, seen);
            }
        }
        Expr::ArrayLiteral { elems, .. } => {
            for elem in elems {
                walk_capture_expr(elem, declared, lowerer, captures, seen);
            }
        }
        Expr::Index { array, index, .. } => {
            walk_capture_expr(array, declared, lowerer, captures, seen);
            walk_capture_expr(index, declared, lowerer, captures, seen);
        }
        _ => {}
    }
}
