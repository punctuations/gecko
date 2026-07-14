# Gecko

Gecko is a Python runtime aimed at short-lived scripts, embedded scripting, edge
and serverless code, CLI tools, and data processing. These are cases where
CPython spends a lot of its time on startup and interpreter overhead.

It implements Python rather than extending it. A Gecko program is a valid Python
program and still runs on CPython, so the usual tools, meaning LSPs, formatters,
linters, and type checkers, go on working.

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

The project is early. The pipeline runs end to end on a subset of Python,
covering literals, names, assignment at module level, arithmetic, comparisons,
`and`, `or`, `not`, `if`, `elif`, `else`, `while`, and calls to builtins such as
`print`. Anything outside that subset is rejected at compile time. Functions,
containers, and `for` do not run yet. There is no GC, so the heap frees
everything at once when it is destroyed.

```sh
cargo run -p gecko -- -c 'print("hello world")'
# hello world

cargo run -p gecko -- examples/sum.py
# sum: 55
```

## License

MIT.
