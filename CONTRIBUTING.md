# Contributing to Rune

Thank you for your interest in contributing to Rune! This document explains how
to set up a development environment and submit changes.

## Development Setup

You need:

- **Rust 1.85+** (Rune uses edition 2024) — install via [rustup](https://rustup.rs)
- **A C compiler** (`gcc`) on your `PATH` — the compiler emits C and links a small runtime

```sh
git clone https://github.com/azghr/rune.git
cd rune
cargo build
```

## Making Changes

1. Open an issue first to discuss significant changes before working on them.
2. Create a branch for your work.
3. Make your changes, with tests where possible.
4. Run the full checklist below.
5. Open a pull request describing what changed and why.

## Checks

Every PR must pass all of the following:

```sh
cargo fmt -- --check   # formatting
cargo clippy           # lints (no warnings allowed)
cargo build            # compiles
cargo test             # test suite
```

A quick smoke test of the full pipeline:

```sh
cargo run -- examples/hello.rune && ./output
```

## Code Style

- Run `cargo fmt` before committing — formatting is enforced by CI.
- Keep `cargo clippy` clean; new warnings are treated as errors.
- Every source file starts with an `// SPDX-License-Identifier: MIT` header.
- Each compiler stage lives in its own module under `src/` — see
  [docs/architecture.md](docs/architecture.md) for how the pipeline fits together.

## Reporting Bugs

Open a [bug report](https://github.com/azghr/rune/issues/new?template=bug_report.yml)
and include:

- A minimal `.rune` program that reproduces the issue
- What you expected vs. what happened
- The relevant compiler output (diagnostics or generated C)

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE) that covers this project.
