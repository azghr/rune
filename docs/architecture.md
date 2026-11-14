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

**Statements** (`stmt.rs`, `decl.rs`): function declarations, `let` bindings
(with an optional `: type` annotation), assignments, `if`/`else`, `while`,
`return`, blocks, and expression statements. Statements are terminated by an
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
  (`int`, `bool`, `string`, `void` — see `types.rs`). The checker verifies:
  - variables are declared before use and assignments match the declared type
  - `let` values match an explicit type annotation when one is given
  - call arguments and return values match the function's signature
  - `if`/`while` conditions are `bool`
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
- Expressions become parenthesized C expressions; `if`/`while` become C
  control flow with generated blocks.
- `print(x)` becomes a call to `rune_print_int` in the runtime. (The runtime
  also provides `rune_print_bool` and `rune_print_string`, but the generator
  does not dispatch on the argument type yet.)

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

- No first-class strings, arrays, or structs
- No functions-as-values or closures
- The IR passes are not connected to the pipeline (see above)
- `print` always calls `rune_print_int`; bool and string arguments are not
  dispatched yet

See the [Roadmap](../README.md#roadmap) in the README for planned work.
