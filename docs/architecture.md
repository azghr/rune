# Rune Compiler Architecture

This document describes how the Rune compiler is structured and how a source
file flows through the pipeline. The code is small by design — the whole
compiler is under 1,600 lines of Rust — so it can serve as a readable example
of a classic compiler pipeline.

```
 source.rune
     │
     ▼
 ┌───────┐   ┌────────┐   ┌──────────┐   ┌────────────┐
 │ Lexer │──▶│ Parser │──▶│ Analyzer │──▶│ C Generator │──▶ output.c
 └───────┘   └────────┘   └──────────┘   └────────────┘
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
`let mut` bindings (with an optional `: type` annotation), assignments,
`if`/`else` (including `else if` chains, which desugar into a block wrapping
the inner if), `while`, `return`, blocks, and expression statements.
A value-less `return` is recognized by the next token not being able to start
an expression, so no semicolon is required before `}`. Statements are terminated by an
optional semicolon — a line break alone is enough, since the lexer discards
newlines.

**Expressions** (`expr.rs`) use one function per precedence level, from
loosest to tightest:

| Level        | Operators      |
|--------------|----------------|
| logical      | `\|\|`, `&&`   |
| equality     | `==`, `!=`     |
| comparison   | `<`, `<=`, `>`, `>=` |
| term         | `+`, `-`       |
| factor       | `*`, `/`, `%`  |
| unary        | `-`, `!`       |
| primary      | literals, names, calls, `print`, parenthesized expressions |

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
  - only variables declared `let mut` may be reassigned
  - `let` values match an explicit type annotation when one is given, and are
    not `void`
  - calls resolve to a declared function with matching argument count and
    types; the return type flows into the surrounding expression
  - `if`/`while` conditions are `bool`
  - `==`/`!=` are rejected on strings (C would compare pointers, not contents)
- Errors are collected in a list and reported together when checking finishes.

Note: the current checker resolves functions globally, so recursion (see
`examples/fib.rune`) and forward references work.

## Stage 4: C Generation (`src/emit/c/generator.rs`)

`CGenerator` walks the checked AST and writes C99 to an in-memory string.

- Functions get a `signature()` helper producing `ret name(params)`; all
  signatures are also emitted as **forward declarations** at the top of the
  file, so definition order never matters.
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
built program. Today they only implement printing; the CLI compiles with:

```
gcc output.c runtime/rune_rt.c -o output -I.
```

## The IR and Passes (`src/transform/`) — not yet wired in

`src/transform/ir.rs` defines a small three-address IR: a `Module` of
`Function`s, each a list of `BasicBlock`s ending in a `Terminator`
(`Return`, `Jump`, or `Branch`). Instructions are `Binary`, `Unary`, `Move`,
and `Call` over `Value`s (constants or locals).

Two example passes exist against this IR:

- `ConstFold` — folds constant binary operations at compile time
- `DeadCodeElim` — removes assignments whose results are never used

These are **not part of the compile pipeline yet**: the driver goes straight
from the checked AST to C. Lowering the AST to this IR and running the passes
before emission is the planned next step, and is the natural seam for adding a
second backend.

## Design Decisions

- **Zero dependencies.** The crate has no external crates, keeping it easy to
  build and read.
- **Spans everywhere.** Every token and AST node carries a span, so all errors
  point at real source locations.
- **Emit C, not assembly.** Generating C delegates register allocation,
  instruction selection, and platform support to an existing compiler, keeping
  the Rune codebase focused on the front end.

## Current Limitations

- No first-class strings, arrays, or structs (strings cannot be compared,
  concatenated, or stored in containers)
- No functions-as-values or closures
- The IR passes are not connected to the pipeline (see above)
- The generator re-infers expression types instead of the checker annotating
  the AST — fine at this scale, but a types-on-AST refactor is the cleaner
  long-term design

See the [Roadmap](../README.md#roadmap) in the README for planned work.
