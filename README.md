# Gecko

Gecko is a Python runtime aimed at short-lived scripts, embedded scripting, edge
and serverless code, CLI tools, and data processing. These are cases where
CPython spends a lot of its time on startup and interpreter overhead.

Gecko implements standard Python, with no language extensions. A Gecko program
is a valid Python program and still runs on CPython, so the usual tools, meaning
LSPs, formatters, linters, and type checkers, go on working.

## Goals

- Low startup cost and low memory use.
- No GIL. Concurrency comes from isolates, actors, and channels instead.
- Typed arrays, columnar storage, and DataFrames in the runtime itself, with
  SIMD and zero-copy operations.

## Architecture

The Rust frontend holds the lexer, parser, compiler, and tooling, and lives in
[`crates/`](crates/). The C runtime holds the Setae VM, the object model, the
GC, and the scheduler, and lives in [`native/`](native/).

See [`ARCHITECTURE.md`](ARCHITECTURE.md) and [`ROADMAP.md`](ROADMAP.md).

## Status

The project is early. The pipeline runs end to end on a subset of Python:
literals, names, assignment (including `x[i] = v` and augmented forms),
arithmetic with `%` and `//`, comparisons and membership, `and`, `or`, `not`,
`if`, `elif`, `else`, `while`, `for`, functions with positional parameters and
recursion, lists, dicts, subscripting, iteration over lists, dicts, strings,
and ranges, the methods `append`, `pop`, `get`, `keys`, and `values`, and the
builtins `print`, `len`, and `range`. Anything outside that subset (closures,
defaults, keyword arguments, tuples, `break`, `continue`, classes, exceptions)
is rejected at compile time. There is no GC, so the heap frees everything at
once when it is destroyed.

```sh
cargo run -p gecko -- -c 'print("hello world")'
# hello world

cargo run -p gecko -- examples/fib.py
# the first ten Fibonacci numbers
```

## License

MIT.
