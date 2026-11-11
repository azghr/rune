<div align="center">

# Rune

![Rust](https://img.shields.io/badge/rust-1.85%2B-DEA584?logo=rust&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-blue)
![Status](https://img.shields.io/badge/status-in%20development-orange)

</div>

A small, statically typed programming language written in Rust. Rune uses a classic compiler pipeline: lexing, parsing, semantic analysis, transformation, and code generation. The compiler targets C, LLVM IR, and WebAssembly, and generated code links against a small C runtime.

## Features

- Statically typed language with a simple C-family syntax
- Multiple compilation backends (C, LLVM, WebAssembly)
- Span based diagnostics and error reporting
- Optimization passes including constant folding and dead code elimination
- Written in Rust with zero external dependencies

## Getting Started

### Prerequisites

- Rust 1.85 or newer (Rune uses edition 2024)
- A C compiler for building the Rune runtime

### Installation

Clone the repository and build the compiler:

```sh
git clone https://github.com/azghr/rune.git
cd rune
cargo build --release
```

## Contributing

Contributions are welcome. Open an issue to discuss a change before submitting a pull request, and make sure the test suite passes before submitting.

## License

This project is licensed under the [MIT License](LICENSE).
