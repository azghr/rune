<div align="center">

# Rune

![Rust](https://img.shields.io/badge/rust-1.85%2B-DEA584?logo=rust&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-blue)
![Status](https://img.shields.io/badge/status-in%20development-orange)

</div>

A small, statically typed programming language written in Rust. Rune uses a
classic compiler pipeline — lexing, parsing, semantic analysis, IR
transformation, and code generation. The compiler currently emits C, which is
compiled with the system `gcc` and linked against a small C runtime.

> **Status: early development.** The pipeline is scaffolded end to end, but the
> crate does not yet compile. See [Roadmap](#roadmap) for what's done and what's
> missing.

## Features

- Statically typed with `int`, `bool`, `string`, and `void`
- C-family syntax: functions, `let` bindings, `if`/`else`, `while`, `return`
- Lexical scoping with a scope stack and symbol tables
- Span-based diagnostics with source-context reporting
- A simple three-address IR with constant folding and dead code elimination
- Written in Rust with zero external dependencies

## Getting Started

### Prerequisites

- Rust 1.85 or newer (Rune uses edition 2024)
- A C compiler (`gcc`) on your `PATH` for building the runtime

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
cargo run -- examples/hello.rune
./output
```

`output.c` is kept alongside the executable so you can inspect the generated C.

## Language Overview

```rust
fn main() {
    let x = 10
    let y = 20
    let sum = x + y * 2
    print(sum)

    if sum > 40 {
        print(1)
    } else {
        print(0)
    }
}
```

- `print(expr)` is the only builtin; it prints an integer followed by a newline
  (via the C runtime).
- Statements are newline/semicolon separated; semicolons are accepted.
- All integers are 64-bit signed (`i64` / C `int64_t`).

## Contributing

Contributions are welcome. Open an issue to discuss a change before submitting
a pull request, and make sure the test suite passes before submitting.

## License

This project is licensed under the [MIT License](LICENSE).
