# Contributing to Rune

Thank you for your interest in contributing to Rune! This document explains how
to set up a development environment and submit changes.

## Development Setup

You need:

- **Rust 1.85+** (Rune uses edition 2024) — install via [rustup](https://rustup.rs)
- **A C compiler** (`gcc`) on your `PATH` — the compiler emits C and links a small runtime

Optional, for working on the other backends and running their checks:

- **clang** — compiles the `rune --llvm` output (the LLVM backend emits textual IR)
- **wabt** (`wat2wasm`) — validates the `rune --wat` output; `brew install wabt`

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

## Releases and Versioning

Rune follows [Semantic Versioning](https://semver.org): while the project is
0.x, **new features bump the minor version** (`v0.1.0` → `v0.2.0`) and bug
fixes bump the patch version. These rules are non-negotiable:

- **Never move or delete a pushed tag.** A tag that has been pushed is
  published: moving it rewrites history, breaks everyone who fetched it, and
  detaches any GitHub Release pinned to it. If a tagged release has a
  problem, fix forward with a new tag instead.
- **`Cargo.toml` version must match the tag** (`v0.2.0` ↔ `version = "0.2.0"`).
- **The changelog drives the release.** Work lands under `## [Unreleased]`;
  at release time that section is renamed to `## [x.y.z] - YYYY-MM-DD` (with
  the version bump and the release's changes) and a fresh empty
  `## [Unreleased]` is added above it. Released sections are never edited —
  if something is missing from a past release, mention it in the next one.
- **Tag only from a green tree**: `cargo fmt -- --check`, `cargo clippy
--all-targets -- -D warnings`, and `cargo test` must all pass on the exact
  commit being tagged.
- Cut the tag as an annotated tag (`git tag -a vX.Y.Z -m "..."`) and push it
  explicitly (`git push origin vX.Y.Z`) — tags are not pushed by
  `git push origin main` alone.

## Backend Verification

Rune has three backends (C via `gcc`, WebAssembly text via `rune --wat`, and
LLVM text via `rune --llvm`). Language features must work on all of them.
`cargo test` covers this where `clang` and `wat2wasm` (wabt) are installed;
those checks skip gracefully when the tools are absent, so prefer verifying
locally with them installed before touching the emitter or runtime. A good
sanity check for any language change: run the same `.rune` program through
the default path and `--llvm`, and diff the outputs.

## Reporting Bugs

Open a [bug report](https://github.com/azghr/rune/issues/new?template=bug_report.yml)
and include:

- A minimal `.rune` program that reproduces the issue
- What you expected vs. what happened
- The relevant compiler output (diagnostics or generated C)

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE) that covers this project.
