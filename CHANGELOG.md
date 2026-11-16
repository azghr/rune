# Changelog

All notable changes to Rune are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Nothing yet.

## [0.2.0] - 2026-11-16

### Language

- Arrays: `[T]` types, `[e0, e1, ...]` literals, indexing (`a[i]`,
  `a[i] = v`, `a[i] += v`), and the `len` builtin. Values are references
  to bounds-checked heap blocks of `[length, elems...]` slots; arrays
  nest, cross function boundaries, and capture into closures (sharing
  the underlying storage). Unsupported: empty literals (no inferable
  type), comparison, and printing

### Compiler

- Small C runtime: bounds-checked `rune_arr_*` array helpers added
  alongside the existing closure and string functions
- Array elements participate in constant folding (elements are evaluated
  eagerly, so known constants propagate into literals)

### Fixed

- Closure wrappers now cast pointer-typed captures (strings, arrays,
  function values) from their i64 slots back to the parameter's type in
  the emitted C; previously capturing a pointer value in a lambda
  produced C that failed to compile

### Tooling

- Test suite (207 tests) across lexer, parser, checker, IR passes, codegen,
  and end-to-end binaries that compile with `gcc` and check output

## [0.1.0] - 2026-11-16

First tagged release.

### Language

- Statically typed: `int` (64-bit signed), `bool`, `string`, `void`
- Functions with typed parameters and return types; recursion and calls
  before definition work
- `let` bindings with optional type annotations; immutability by default
  with `let mut` for reassignable variables
- Control flow: `if`/`else if`/`else`, `while`, C-style `for` with optional
  clauses, `break`/`continue` (innermost loop, with C `continue` semantics),
  and `return` (valued and bare)
- Operators: arithmetic, comparison, equality, short-circuit `&&`/`||`, and
  compound assignment (`+=`, `-=`, `*=`, `/=`, `%=`; `+=` concatenates
  strings); statements are separated by newlines or semicolons
- `print(expr)` builtin for integers, bools, and strings
- Strings: comparison with `==`/`!=` (contents, via `strcmp`) and
  concatenation with `+`/`+=` (via a runtime helper; results are currently
  leaked by design)
- Anonymous functions (`fn(x: int) -> int { ... }` as an expression) and
  `fn(...) -> ...` function types: functions are first-class values, with
  by-value capture (assigning to a captured variable is rejected)

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
- Test suite (178 tests) across lexer, parser, checker, IR passes, codegen,
  and end-to-end binaries that compile with `gcc` and check output
- `rune --wat`: a WebAssembly text-format backend emitting from the same
  IR, with host imports for `print` and strings living in linear memory
  (data segments plus a bump-allocating runtime prelude). Function values
  work on all backends: a value is a closure block (`[code word, captures]`)
  reached through `call_indirect` over a funcref table of env-first wrappers
  — baseline WASM 1.0, no reference-types proposal needed
- `rune --llvm`: an LLVM backend emitting textual LLVM IR from the same IR
  module, compiled with `clang` against the C runtime — no LLVM libraries,
  keeping the zero-dependency rule. The IR's CFG maps directly to LLVM
  basic blocks; locals are `alloca` slots; function values use the same
  closure convention via real function pointers
- CI on every push and PR: formatting, clippy, build, tests, and a pipeline
  smoke test; the test suite validates the generated WAT with `wat2wasm`
  (wabt) and compiles/executes the generated LLVM IR with `clang` when
  those tools are installed, skipping the checks otherwise

### Fixed

- Constant propagation no longer keeps a stale constant across a
  non-constant reassignment (`x = f()` now correctly invalidates `x`)
- String escapes (`\n`, `\t`, `\"`, `\\`) are re-encoded when emitting C;
  previously a string containing an escaped quote produced invalid C
- Env-first closure wrappers now load captured values from the closure
  block; previously capturing lambdas forwarded garbage for their captures
  (they worked only when a register happened to still hold the value)
- The WASM backend no longer leaves a stray value on the operand stack when
  a non-void call is made as a statement, and string literals passed as
  arguments to indirect calls now get data segments

### Known limitations

- No arrays, structs, or containers; strings are immutable values without
  ownership tracking (concatenation leaks)
- No generics
- No standard library to speak of (`std/core.rune` is a placeholder)
- The IR-to-C output uses labels and gotos rather than structured control
  flow
