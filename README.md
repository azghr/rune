<div align="center">

# Rune

A small, statically typed programming language written in Rust.

[![CI](https://github.com/azghr/rune/actions/workflows/ci.yml/badge.svg)](https://github.com/azghr/rune/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Status](https://img.shields.io/badge/status-in%20development-orange)]()

</div>

Rune uses a classic compiler pipeline — lexing, parsing, semantic analysis,
lowering to a three-address IR, transformation passes, and code generation.
The compiler emits C (via the IR), which is compiled with the system `gcc`
and linked against a small C runtime.

> **Status: early development.** The pipeline is scaffolded end to end and the
> language is usable for small programs, but Rune is still missing many
> features. See [Roadmap](#roadmap) for what's done and what's planned.

## Features

- Statically typed with `int`, `bool`, `string`, and `void`
- C-family syntax: functions, `let`/`let mut` bindings, `if`/`else if`/`else`,
  `while`, `for`, `break`/`continue`, `return`, and compound assignment
  (`+=` etc.)
- Arrays `[T]` with bounds-checked indexing and the `len` builtin
- Anonymous functions (`fn(n: int) -> int { ... }` as an expression) with
  by-value closure capture
- Logical operators (`&&`, `||`) with correct precedence
- Immutability by default: only `mut` variables can be reassigned
- String comparison with `==`/`!=` compares contents
- String concatenation with `+` (backed by a small C runtime helper)
- Lexical scoping with a scope stack and symbol tables
- Span-based diagnostics with source-context reporting
- Compiles through a typed three-address IR with constant folding (including
  branch elimination), unreachable-block removal, and liveness-based dead
  code elimination
- Three backends: C (via `gcc`), WebAssembly text format (`rune --wat`,
  runnable in any WASM host, with strings in linear memory), and LLVM
  textual IR (`rune --llvm`, compiled with `clang`)
- Short-circuit `&&`/`||` lowered to real control flow, not eager evaluation
- Written in Rust with zero external dependencies

## Getting Started

### Prerequisites

- Rust 1.85 or newer (Rune uses edition 2024)
- A C compiler (`gcc`) on your `PATH`
- For `rune --llvm`, `clang`; for `rune --wat`, any WASM host
  ([wat2wasm](https://github.com/WebAssembly/wabt) validates the output)

### Building

```sh
git clone https://github.com/azghr/rune.git
cd rune
cargo build --release
```

### Running a Program

The driver compiles a `.rune` file to C, invokes `gcc`, and produces an
executable in the working directory:

```sh
cargo run --release -- examples/hello.rune
./output
```

`output.c` is kept alongside the executable so you can inspect the generated C.

The compiler locates the C runtime (`runtime/rune_rt.c`) by searching upward
from the working directory and then from the executable. When neither works
(e.g. for an installed binary), set `RUNE_RUNTIME_DIR` to the directory that
contains the `runtime` folder. Only the C and LLVM backends need the runtime;
`rune --wat` and `rune --help` work anywhere.

## Language Overview

```rust
fn main() {
    let x = 10
    let mut y = 20
    let sum = x + y * 2
    print(sum)

    if sum > 40 {
        print(1)
    } else if sum > 20 {
        print(2)
    } else {
        print(0)
    }

    y += 1
    for ; y < 23; y += 1 {
        print(y)
    }
    print(true)
    print("done")
}
```

- `print(expr)` prints an integer, bool, or string followed by a newline
  (via the C runtime); `len(arr)` returns an array's length.
- Variables are immutable by default; declare them `let mut` to reassign.
- Variables are immutable by default; declare them `let mut` to reassign.
- Statements are newline/semicolon separated; semicolons are accepted.
- All integers are 64-bit signed (`i64` / C `int64_t`).

More examples live in [`examples/`](examples).

## Project Structure

```
├── src/
│   ├── language/      # lexer, parser, AST, type checker, scopes
│   ├── diagnostics/   # diagnostics and source-context reporting
│   ├── transform/     # AST-to-IR lowering, three-address IR, passes
│   ├── emit/          # backends: C (AST and IR emitters) and WebAssembly (WAT)
│   ├── driver/        # high-level compile pipeline
│   └── shared/        # source files and spans
├── runtime/           # small C runtime linked into every binary
├── examples/          # example Rune programs
├── docs/              # design and architecture notes
└── tests/             # lexer, parser, analysis, and emit tests
```

## Documentation

- [Language reference](docs/language.md) — the Rune language: syntax, types,
  operators, and control flow
- [Architecture](docs/architecture.md) — how the compiler pipeline fits together

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) to
get started, and note that this project follows the
[Code of Conduct](CODE_OF_CONDUCT.md). Notable changes are tracked in the
[Changelog](CHANGELOG.md).

## License

This project is licensed under the [MIT License](LICENSE).
