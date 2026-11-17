# Security Policy

## Reporting a Vulnerability

Please report security issues **privately** — do not open a public issue for
anything that could be exploited:

- **Preferred:** use GitHub's
  [private vulnerability reporting](https://github.com/azghr/rune/security/advisories/new)
  to disclose directly to the maintainer.
- Otherwise, use the contact method listed on the maintainer's GitHub profile.

Include what you found, how to reproduce it, and the impact. You will get an
acknowledgment within a few days and updates as the fix progresses. Credit is
given in the release notes unless you prefer to stay anonymous.

## Scope

Rune is a compiler: it reads `.rune` source files, emits C / WebAssembly /
LLVM IR, and invokes external toolchains (`gcc`, `clang`). Vulnerabilities in
scope include:

- The compiler crashing, panicking, or miscompiling on input a user could not
  have anticipated beyond writing unusual Rune code (panics on malformed
  source are bugs, but report ones with security impact first)
- Generated code executing something other than what the source semantics
  describe — especially escapes from the language's documented safety
  guarantees (bounds checking, immutability, type safety)
- The bundled C runtime (`runtime/rune_rt.c`) misbehaving on inputs produced
  by the compiler
- Anything that lets a `.rune` program read, write, or execute outside the
  process's normal abilities

Out of scope: bugs in `gcc`, `clang`, or WASM hosts themselves; resource
exhaustion from a program the user chose to run (Rune programs may loop and
allocate freely — it is not a sandbox); and the known limitations documented
in the [language reference](docs/language.md), such as leaked string
concatenations.

## Supported Versions

Rune is in early development and only the latest release receives fixes.

| Version | Supported |
| ------- | --------- |
| 0.2.x   | ✅        |
| < 0.2   | ❌        |
