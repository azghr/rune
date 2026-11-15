# Rune Compiler Architecture

This document describes how the Rune compiler is structured and how a source
file flows through the pipeline. The code is small by design — the whole
compiler is under 1,600 lines of Rust — so it can serve as a readable example
of a classic compiler pipeline.

```
 source.rune
     │
     ▼
 ┌───────┐   ┌────────┐   ┌──────────┐   ┌──────────┐
 │ Lexer │──▶│ Parser │──▶│ Analyzer │──▶│  Lowerer │──▶ IR module
 └───────┘   └────────┘   └──────────┘   └──────────┘
                                                    │
                                       ConstFold + DeadCodeElim
                                                    │
                                                    ▼
                                              IR → C emitter ──▶ output.c
                                                                    │
                                                                    ▼
                                                        gcc + runtime/rune_rt.c
                                                                    │
                                                                    ▼
                                                                 ./output
```

The driver (src/driver) ties these stages together: `Compiler::compile` takes
source text and runs each stage in order, reporting diagnostics and stopping at
the first stage that fails.

There is also a second C emitter that goes straight from the AST
(`src/emit/c/generator.rs`); the driver no longer uses it, but it remains as
a reference and is exercised by the test suite. `cargo run --bin irdebug --
[--passes|--fold|--dce] file.rune` dumps the IR (optionally after the
passes) for inspection.

## Shared Infrastructure

### Spans (`src/shared/span.rs`)

A `Span` is a byte-offset range (`start..end`) into the source text, plus the
id of the source file it belongs to. Every token, AST node, and diagnostic
carries a span. Spans are merged with `Span::merge` as nodes are built, so by
the time an error is reported, its span covers exactly the construct that
caused it.

### Sources (`src/shared/source.rs`)

`SourceFile` holds the file name and contents. It is the reference the rest of
the compiler uses to turn byte offsets back into line/column positions when
printing diagnostics.

### Diagnostics (`src/diagnostics/`)

A `Diagnostic` is a message plus a span and severity (`Error` or `Warning`).
The `Reporter` pretty-prints diagnostics to stderr: it maps the span's byte
offset to a line/column via the `SourceFile` and prints the offending source
line with a caret underneath.

## Stage 1: Lexer (`src/language/lexer.rs`)

A hand-written lexer that scans the source character by character and produces
a `Vec<Token>` — each token is a `TokenKind` plus its `Span`.

- Keywords (`fn`, `let`, `if`, `while`, `return`, …) are recognized during
  identifier lexing via a lookup table.
- Literals: integer literals (parsed to `i64`), string literals in double
  quotes, and the `true` / `false` keywords.
- Operators are lexed greedily, so `==`, `!=`, `<=`, `>=`, `&&`, and `||` are
  single tokens.
- Comments start with `//` and run to the end of the line; newlines are skipped
  entirely, which is why statements can be separated by either `;` or a line
  break.

## Stage 2: Parser (`src/language/parser/`)

A recursive-descent parser producing the AST defined in `src/language/ast.rs`.

**Statements** (`stmt.rs`, `decl.rs`): function declarations, `let` and
`let mut` bindings (with an optional `: type` annotation), plain and compound
assignments (`+=` etc.), `if`/`else` (including `else if` chains, which
desugar into a block wrapping the inner if), `while`, C-style `for` (pure
sugar: it desugars into a block holding the initializer followed by a `while`
whose body ends with the step, so no later stage knows `for` exists),
`return`, blocks, and expression statements.
A value-less `return` is recognized by the next token not being able to start
an expression, so no semicolon is required before `}`. Statements are terminated by an
optional semicolon — a line break alone is enough, since the lexer discards
newlines.

**Expressions** (`expr.rs`) use one function per precedence level, from
loosest to tightest:

| Level      | Operators                                                  |
| ---------- | ---------------------------------------------------------- |
| logical    | `\|\|`, `&&`                                               |
| equality   | `==`, `!=`                                                 |
| comparison | `<`, `<=`, `>`, `>=`                                       |
| term       | `+`, `-`                                                   |
| factor     | `*`, `/`, `%`                                              |
| unary      | `-`, `!`                                                   |
| primary    | literals, names, calls, `print`, parenthesized expressions |

All levels build left-associative `Expr::Binary` nodes. Calls and the `print`
builtin share the `parse_call_args` helper.

## Stage 3: Analyzer (`src/language/analyze/`)

The type checker walks the AST top-down.

- **Scopes** (`scope.rs`): a `ScopeStack` of hash-map symbol tables. The
  global scope holds function declarations; each block pushes and pops a
  scope, implementing lexical scoping.
- **Checker** (`checker.rs`): every expression is annotated with a `Type`
  (`int`, `bool`, `string`, `void` — see `types.rs`). Function signatures are
  collected in a first pass, so recursion and calls before definition work.
  The checker verifies:
  - variables are declared before use and assignments match the declared type
  - only variables declared `let mut` may be reassigned; compound
    assignments (`x += v`) are checked as `x = x <op> v`
  - `let` values match an explicit type annotation when one is given, and are
    not `void`
  - calls resolve to a declared function with matching argument count and
    types; the return type flows into the surrounding expression
  - `+` accepts two strings (concatenation) but rejects mixing strings and
    numbers; other arithmetic requires numbers
  - `if`/`while` conditions are `bool`
  - `==`/`!=` accept same-type operands, including strings
- Errors are collected in a list and reported together when checking finishes.

Note: the current checker resolves functions globally, so recursion (see
`examples/fib.rune`) and forward references work.

## Stage 4: Lowering (`src/transform/lower.rs`)

The checked AST is lowered into the three-address IR described in
[`The IR and Passes`](#the-ir-and-passes-srctransform) — that section's
caveat about the passes not being wired in is now historical; the driver runs
them.

- Expressions become instruction chains: each binary/unary operation writes
  its result to a generated temp (`t0`, `t1`, …). Temp names are chosen to
  never collide with user variables.
- `if` becomes a `Branch` to then/else blocks that jump to a shared merge
  block; `while` becomes a condition block whose body jumps back to it.
- `&&` and `||` lower to **short-circuit control flow**: the right operand is
  evaluated in its own block, reached only if the left operand allows it, so
  side effects behave exactly as they would in C.
- Every function's fall-through end gets an implicit `Return(None)`.
- A `type` environment is rebuilt during lowering (parameters, `let` bindings,
  block scoping) so each instruction carries the type its C emission needs.

## Stage 5: Passes (`src/transform/passes/`)

Two passes run over the IR before emission:

- `ConstFold` — folds constant arithmetic (with wrapping semantics and
  division-by-zero left unevaluated), propagates constants within a block
  (so `2 * 3 + 4` folds all the way to `10`), folds branches whose condition
  became a constant into unconditional jumps, and removes blocks that became
  unreachable, renumbering the rest.
- `DeadCodeElim` — backward **liveness analysis** over the whole CFG (a
  standard fixpoint): an assignment is removed when its destination is not
  read afterwards on any path, so dead stores — including values overwritten
  before ever being read — disappear, while loop-carried and cross-block
  values are preserved. Calls are always kept (they may have side effects).

## Stage 6: C Generation (`src/emit/c/`)

Two emitters exist:

**`ir.rs` — IR to C (used by the driver).** Each basic block becomes a `goto`
label; `Branch` becomes `if (cond) goto L_then;` / `goto L_else;`. All locals
and temps are declared at the top of the function with their C types, and
`print` dispatches to the right runtime function using the locals' recorded
types; string `==`/`!=` become `strcmp` comparisons (a raw C `==` on
`const char*` would compare pointers, not contents), and string `+` becomes
a call to the runtime's `rune_strcat`. The output is less readable than the
AST emitter's, but it reflects exactly what the transform passes produced.

**`generator.rs` — AST to C (reference).** The original direct emitter,
described below.

`CGenerator` walks the checked AST and writes C99 to an in-memory string.

- Functions get a `signature()` helper producing `ret name(params)`; both
  emitters also emit all signatures as **forward declarations** at the top of
  the file, so definition order never matters.
- `main` is special-cased: its return type becomes C `int` and a `return 0;`
  is emitted, satisfying C's requirements regardless of the Rune signature.
- Rune types map to C as: `int` → `int64_t`, `bool` → `bool`,
  `string` → `const char*`, `void`/missing → `void`.
- While emitting, the generator maintains its own scope stack of variable
  types plus a function return-type table, letting it infer the static type of
  each expression. `let` declarations get the matching C type, and `print(x)`
  dispatches to `rune_print_int` / `rune_print_bool` / `rune_print_string`
  based on the argument's type.
- Expressions become parenthesized C expressions; `if`/`while` become C
  control flow with generated blocks.

## Runtime (`runtime/`)

`rune_rt.c` / `rune_rt.h` are a handful of small C functions linked into every
built program: the `rune_print_*` functions and `rune_strcat` (string
concatenation). Concatenation results are heap-allocated and never freed —
Rune does not track string ownership, so the compiler currently leaks
concatenated strings by design. The CLI compiles with:

```
gcc output.c runtime/rune_rt.c -o output -I.
```

## The IR (`src/transform/ir.rs`)

`src/transform/ir.rs` defines a small three-address IR: a `Module` of
`Function`s, each a list of `BasicBlock`s ending in a `Terminator`
(`Return`, `Jump`, or `Branch`). Instructions are `Binary`, `Unary`, `Move`,
and `Call` over `Value`s (constants or locals), and every instruction records
its result type.

This IR is the natural seam for adding a second backend (LLVM, WASM): new
emitters read the same `Module` the C emitter does.

## Design Decisions

- **Zero dependencies.** The crate has no external crates, keeping it easy to
  build and read.
- **Spans everywhere.** Every token and AST node carries a span, so all errors
  point at real source locations.
- **Emit C, not assembly.** Generating C delegates register allocation,
  instruction selection, and platform support to an existing compiler, keeping
  the Rune codebase focused on the front end.

## Current Limitations

- No first-class strings, arrays, or structs (strings support comparison
  and concatenation, but cannot be stored in containers; concatenation
  results are leaked by design — see the runtime section)
- No functions-as-values or closures
- `DeadCodeElim` uses a function-global used-set, so dead stores whose
  variables are read _somewhere_ are kept; a precise liveness analysis would
  remove more
- `ConstFold` propagation is per-block, so constants never cross block
  boundaries
- IR-based `output.c` uses `goto` labels rather than structured `if`/`while`
  (see `generator.rs` for the readable direct-AST alternative)
- Both C emitters re-derive expression types during their walks instead of
  the checker annotating the AST — fine at this scale, but a types-on-AST
  refactor is the cleaner long-term design

See the [Roadmap](../README.md#roadmap) in the README for planned work.
