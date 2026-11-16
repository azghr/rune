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
| `[T]`    | arrays of `T`                             | `void*`       |

Types are checked statically; there is no implicit conversion.

## Arrays

```rust
let a = [10, 20, 30]    // type inferred as [int]
let names: [string] = ["x", "y"]
print(a[0])             // indexing reads an element
print(len(a))           // 3
let mut b = [1, 2]
b[0] = 9                // element assignment
b[1] += 5               // compound assignment works on elements too
```

- **Literals** are `[e0, e1, ...]`; all elements must have the same type,
  and the element type is inferred from them. Empty literals are rejected
  (there is no way to name their type yet).
- **Semantics**: an array value is a reference to a heap block holding its
  length and elements. Arrays can be passed to and returned from functions,
  stored in other arrays (nested `[int]`), and captured by closures — all
  sharing the same underlying storage.
- **Mutability**: element writes go through the reference and are always
  allowed, like C arrays behind a pointer. Only rebinding the *variable*
  needs `let mut` (`a = [4, 5]` on an immutable `a` is an error).
- **Bounds**: every access is checked at runtime; an out-of-bounds index
  prints a diagnostic and aborts the program.
- **Restrictions**: arrays cannot be compared with `==`/`!=` (element-wise
  comparison does not exist yet) and cannot be printed.

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

## Anonymous functions and closures

```rust
fn main() {
    let double = fn(n: int) -> int { return n * 2 }
    print(double(21))       // 42

    let base = 10
    let addbase = fn(n: int) -> int { return n + base }
    print(addbase(5))       // 15: `base` is captured
}
```

- An anonymous function is an expression: `fn(params) -> ret { body }`, with
  the same syntax as a named function minus the name. It can appear anywhere
  a value can and is stored in a variable to be called later.
- The body's reads of outer variables **capture** them by value at the time
  the closure is created; later changes to the original are not visible.
- Assigning to a captured variable inside the closure is a compile-time
  error — captures are snapshots.
- Captures work across arrays and nested closures; function values can be
  stored in arrays.

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
- `<`, `<=`, `>`, `>=` require two `int` operands. `==`/`!=` accept any two
  operands of the same type; string comparison compares **contents**.
- Division (or remainder) by zero aborts the program with a diagnostic;
  integer arithmetic wraps on overflow.
- Equality and comparisons require two operands of the same type; string
  equality compares **contents**.
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
  (C semantics). Both are errors outside any loop — including inside an
  anonymous function nested in a loop, since the lambda's body is its own
  function.

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
- A function with a return type must return on **every** path: a body that
  can fall off the end is a compile-time error. The check is conservative —
  an `if` counts only when it has an `else` whose branches both return, and
  loops are never assumed to exit via `return`, so put a `return` after the
  loop (it is unreachable if the loop never exits).

## Builtin functions

- `len(arr)` — returns the number of elements in an array as an `int`.
- `print(expr)` — prints an `int`, `bool`, or `string` followed by a
  newline. Takes exactly one argument; `void` values cannot be printed.
  Strings compare and concatenate but cannot be printed with escapes
  interpreted (the C runtime prints them verbatim).

## Program structure

A program is a sequence of function declarations. Execution starts at `main`,
which must take no parameters (the compiler gives it a C-compatible
`int main()` with an implicit `return 0`). All functions are emitted with
forward declarations, so definition order never matters.

## Current limitations

- No structs or tuples (arrays exist; see above)
- No generics
- No type inference beyond `let` bindings; no enums or sum types
- Concatenated strings are heap-allocated and never freed (no ownership
  tracking yet)
- Integer arithmetic wraps on overflow (both in constant folding and at
  runtime, matching Rust's release-mode behavior)
