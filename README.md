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
`if`, `elif`, `else`, `while`, `for` with `break` and `continue`, functions
with the full call convention (positional and keyword arguments, defaults,
`*args`, `**kwargs`, and `*`/`**` spreads at the call), recursion, closures with
`nonlocal`, lists, dicts,
tuples with unpacking, list and dict comprehensions, `try`/`except`/`else`/
`finally` with `raise` and the builtin exception types, classes with single
inheritance, `__init__`, methods, and attributes, decorators on functions and
classes, importing modules and packages, including dotted subpackages, with
`import` and `from ... import` (resolved along the importing directory,
`GECKO_PATH`, and site-packages), subscripting, iteration over lists, tuples,
dicts, strings, and ranges,
the methods `append`, `pop`, `get`, `keys`, `values`, and `items`, and the
builtins `print`, `len`, and `range`. Anything outside that subset (generator
expressions, ternary expressions, keyword-only parameters, multiple inheritance,
`super`, bare `raise`) is rejected at compile time. A precise,
non-moving mark-sweep collector reclaims garbage when allocation passes a
threshold that grows with the live size. An embedding host can run many
isolated VMs and cap each one's steps, wall-clock time, and heap, so untrusted
code cannot loop or allocate without bound, and can register native host
functions that scripts call like builtins. A program can also run other code
under those limits through the builtin `sandbox` module (`from gecko import
sandbox`): `sandbox.run(source, steps, memory, millis)` runs the source in a
fresh isolated VM and returns its output.

```sh
cargo run -p gecko -- -c 'print("hello world")'
# hello world

cargo run -p gecko -- examples/fib.py
# the first ten Fibonacci numbers
```

## Building a binary

`gecko build` freezes a program into a standalone executable. The compiled
bytecode is appended to a copy of `gecko-runner`, a stub holding only the VM
and the bytecode reader, so the result starts without parsing or compiling
anything.

```sh
cargo build --release
./target/release/gecko build examples/fib.py -o fib
./fib
```

A plain `cargo build --release` runner links the full Rust standard library and
weighs about 330 KB. `scripts/build-runner.sh` rebuilds it against a
size-optimized std, using nightly `build-std` with the immediate-abort panic
strategy, and drops the result at `target/release/gecko-runner`. That runner is
about 100 KB, so a frozen program lands near 100 KB plus its bytecode. It needs
a nightly toolchain with `rust-src`:

```sh
rustup toolchain install nightly --component rust-src
./scripts/build-runner.sh
./target/release/gecko build examples/fib.py -o fib
```

gecko looks for the release runner next to itself, then in the cargo target
layout, so freezing from a debug gecko still embeds the small release runner.
`gecko build --debug` embeds a debug runtime instead, for debugging the
runtime itself.

## Installing packages

`gecko install` unpacks a pure-Python wheel into site-packages so any program
can import it.

```sh
gecko install some_package-1.0-py3-none-any.whl
```

site-packages lives under `GECKO_HOME` (default `~/.gecko`), and is searched
after the importing directory and `GECKO_PATH`. Pass `--to dir` to install
somewhere else. Wheels with compiled C extensions do not run, since gecko has
no CPython C ABI.

## License

MIT.
