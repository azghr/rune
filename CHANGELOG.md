# Changelog

All notable changes to Rune are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-15

First open-source release: a complete end-to-end compiler pipeline for a
small statically typed language.

### Language

- Statically typed: `int` (64-bit signed), `bool`, `string`, `void`
- Functions with typed parameters and return types; recursion and calls
  before definition work
- `let` bindings with optional type annotations; immutability by default
  with `let mut` for reassignable variables
- Control flow: `if`/`else if`/`else`, `while`, and `return` (valued and
  bare)
- Operators: arithmetic, comparison, equality, and short-circuit `&&`/`||`;
  statements are separated by newlines or semicolons
- `print(expr)` builtin for integers, bools, and strings
- Strings: comparison with `==`/`!=` (contents, via `strcmp`) and
  concatenation with `+` (via a runtime helper; results are currently
  leaked by design)

### Compiler

- Hand-written lexer with span tracking; recursive-descent parser with one
  function per precedence level
- Type checker with lexical scopes and a symbol stack; function signatures
  collected in a first pass so forward references and recursion check
- Span-based diagnostics with source-context reporting (line, caret)
- Lowering to a typed three-address IR: basic blocks, `Branch`/`Jump`
  terminators, generated temps that never collide with user variables, and
  short-circuit operators lowered to real control flow
- Optimization passes: constant folding and propagation (with wrapping
  semantics), constant-condition branch folding with unreachable-block
  removal, and dead code elimination driven by a backward liveness fixpoint
- C emission: the driver emits C99 from the IR (`goto`-label style); a
  direct AST-to-C generator is included as a readable reference
- Small C runtime (`rune_print_*`, `rune_strcat`) linked into every program

### Tooling

- `rune` CLI: compiles `.rune` files to C, invokes `gcc`, produces a binary
- `irdebug` binary: dumps the IR before and after each pass
- Test suite (~103 tests) across lexer, parser, checker, codegen, IR
  passes, and end-to-end binaries that compile with `gcc` and check output
- CI on every push and PR: formatting, clippy, build, tests, and a
  pipeline smoke test

### Known limitations

- No arrays, structs, or containers; strings are immutable values without
  ownership tracking (concatenation leaks)
- No functions-as-values or closures
- No standard library to speak of (`std/core.rune` is a placeholder)
- The IR-to-C output uses labels and gotos rather than structured control
  flow

[0.1.0]: https://github.com/azghr/rune/releases/tag/v0.1.0
