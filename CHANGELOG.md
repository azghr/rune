# Changelog

All notable changes to Rune are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.2] - 2026-11-16

### Fixed

- The CLI locates the C runtime (`runtime/rune_rt.c`) by searching upward
  from the working directory and then from the executable, so `rune` works
  from any directory; `RUNE_RUNTIME_DIR` overrides the search and a bad
  value is reported instead of silently ignored
- `--help` and `--wat` no longer require the C runtime to be locatable
- A program without a `main` function is rejected with a diagnostic on the
  C and LLVM paths instead of failing in the linker (`--wat` still accepts
  library modules without `main`)
- A function with a return type that can fall off the end of its body is a
  compile error (`not all paths return a value`) instead of silently
  returning zero
- Empty array literals no longer produce a misleading secondary
  `type mismatch` error naming the wrong type

### Changed

- The check that every path in a non-`void` function returns is
  conservative: loops are never assumed to exit via `return`, so a `return`
  inside `for ;;` does not satisfy it — add a `return` after the loop

## [0.2.1] - 2026-11-16

### Fixed

- Identifiers that collide with C keywords (`double`, `register`, …) are
  emitted with a trailing underscore, so programs using them as names now
  compile through the C backend
- Constant folding no longer panics on `INT64_MIN / -1` (it wraps, matching
  the runtime); the smallest `i64` is emitted as a warning-free C literal
- Division and remainder go through runtime helpers in the C and LLVM
  backends: a zero divisor aborts with a diagnostic instead of hitting C's
  or `sdiv`'s undefined behavior, and `INT64_MIN / -1` wraps
- `&&` now binds tighter than `||`, matching the documented precedence
  table (`true || false && false` is `true`)
- `bool == bool` compares correctly in the WASM and LLVM backends (both
  previously emitted mismatched operand widths)
- `break`/`continue` inside an anonymous function are rejected by the
  analyzer instead of crashing the compiler
- Nested anonymous functions now capture transitively: a variable read only
  by an inner lambda is captured by the enclosing lambda
- Compound assignment (`x += v`) to a captured variable is rejected, like
  plain assignment
- Dead-code elimination keeps division and array reads whose result is
  unused, so their runtime traps (zero divisor, out-of-bounds index) still
  fire
- Duplicate parameter names in function declarations are a compile error
- `main` with parameters is rejected with a diagnostic
- Non-`void` functions that fall off the end return zero in the WASM and
  LLVM backends, matching the C backend (previously invalid output)
- Diagnostic columns count characters, so the caret lines up after
  multibyte characters

### Changed

- `rune --help` prints usage; the `--wat`/`--llvm` pipelines run the same
  transform passes as the default C pipeline
- Documentation refreshed: language reference gained anonymous functions
  and closures, and the architecture notes match the current code

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
