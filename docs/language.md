# The Rune Language

A reference for the Rune programming language as of version 0.1.0. For the
compiler's internals, see [architecture.md](architecture.md).

A small, statically typed language with C-family syntax. A complete program:

```rust
fn main() {
    let name = "world"
    print("hello, " + name)
}
```

Run it with `cargo run -- examples/hello.rune` — the compiler emits C, builds
it with `gcc`, and produces an executable.

## Lexical structure

- **Comments** start with `//` and run to the end of the line.
- **Identifiers** match `[a-zA-Z_][a-zA-Z0-9_]*`.
- **Keywords**: `fn let mut if else while for break continue return true
false print int bool string`
- **Integer literals** are decimal `i64` values (`0`, `42`).
- **String literals** are double-quoted with the escapes `\n`, `\t`, `\"`,
  and `\\`. Any other character after `\` is that character itself.
- **Statements** are separated by newlines or semicolons. Semicolons are
  optional and may mix freely with line breaks.

## Types

| Type     | Values                                    | C equivalent  |
| -------- | ----------------------------------------- | ------------- |
| `int`    | 64-bit signed integers                    | `int64_t`     |
| `bool`   | `true`, `false`                           | `bool`        |
| `string` | immutable text values                     | `const char*` |
| `void`   | the type of functions that return nothing | `void`        |

Types are checked statically; there is no implicit conversion.

## Variables

```rust
let x = 10          // type inferred (int)
let y: bool = true  // explicit type annotation
let mut count = 0   // mutable; reassignment requires `mut`
count += 1
```

Variables are **immutable by default**. Assigning to a variable without
`mut` is a compile-time error, as is assigning a value of a different type.

## Functions

```rust
fn add(a: int, b: int) -> int {
    return a + b
}

fn greet(name: string) {
    print("hello, " + name)
    // no return statement: the function returns void
}
```

- Parameters and return types are annotated; the return type (after `->`)
  may be omitted for functions that return nothing.
- Recursion works, and functions may be called before they are defined.

## Operators

From loosest to tightest binding:

| Precedence | Operators                        | Result type                |
| ---------- | -------------------------------- | -------------------------- |
| 1          | `\|\|` (short-circuit)           | `bool`                     |
| 2          | `&&` (short-circuit)             | `bool`                     |
| 3          | `==` `!=`                        | `bool`                     |
| 4          | `<` `<=` `>` `>=`                | `bool`                     |
| 5          | `+` `-`                          | `int`, or `string` for `+` |
| 6          | `*` `/` `%`                      | `int`                      |
| 7          | unary `-` `!`                    | matches operand            |
| 8          | literals, names, calls, `(expr)` | —                          |

All binary operators are left-associative. Rules to know:

- `&&` and `||` short-circuit: the right operand is only evaluated when the
  left operand allows it.
- Comparisons and equality require two operands of the same type; string
  comparison compares **contents**.
- `+` concatenates two strings; mixing strings and numbers is an error.
- Compound assignment `+=` `-=` `*=` `/=` `%=` assigns to a mutable
  variable; `s += t` concatenates strings.

## Control flow

```rust
if x > 0 {
    print(1)
} else if x == 0 {
    print(0)
} else {
    print(-1)
}

while x < 10 {
    x += 1
}

for let mut i = 0; i < 10; i += 1 {
    print(i)
}
```

- Conditions must be `bool`.
- **`for`** is C-style with three optional clauses: `for init; cond; step { }`.
  The initializer's variables are scoped to the whole loop. `for ;;` is an
  infinite loop.
- **`break`** exits the innermost loop; **`continue`** skips to the next
  iteration. In a `for` loop, `continue` still runs the step clause
  (C semantics). Both are errors outside any loop.

## Returning

```rust
fn fib(n: int) -> int {
    if n < 2 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}
```

- `return expr` returns a value; bare `return` exits a function. The value's
  type must match the declared return type.
- A function without a return type returns `void`; `void` values cannot be
  assigned to variables or printed.

## Builtin functions

- `print(expr)` — prints an `int`, `bool`, or `string` followed by a
  newline. Takes exactly one argument; `void` values cannot be printed.
  Strings compare and concatenate but cannot be printed with escapes
  interpreted (the C runtime prints them verbatim).

## Program structure

A program is a sequence of function declarations. Execution starts at `main`
(the compiler gives it a C-compatible `int main()` with an implicit
`return 0`). All functions are emitted with forward declarations, so
definition order never matters.

## Current limitations

- No arrays, structs, tuples, or containers of any kind
- No functions-as-values, closures, or generics
- No type inference beyond `let` bindings; no enums or sum types
- Concatenated strings are heap-allocated and never freed (no ownership
  tracking yet)
- Integer arithmetic uses wrapping semantics at the optimization pass level
  (e.g. constant folding), matching Rust's release-mode behavior
