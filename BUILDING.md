# Building Gecko

## Prerequisites

- Rust 1.85 or newer, for edition 2024. Install from https://rustup.rs.
- Meson and Ninja, to build the C runtime under `native/`, via
  `pip install meson ninja`.
- A C compiler whose objects match the Rust host ABI. On Windows that means
  clang-cl. A MinGW clang or gcc will not link.

`cargo build` runs Meson as part of the build, so it is the only command needed.

## Supported platforms

| Platform | Status                                      |
| -------- | ------------------------------------------- |
| Linux    | Built and tested on every change            |
| macOS    | Built and tested on every change            |
| Windows  | Builds with clang-cl, not covered by CI     |

## Build

```sh
cargo build --release
./target/release/gecko --version
./target/release/gecko examples/fib.py
./target/release/gecko -c 'print("hello world")'
```

## Tests

```sh
cargo test
```

Tests sit next to the code they cover. The tests that run source text through
the whole pipeline live in `crates/gecko`. `crates/runtime` holds VM-level tests
that hand-assemble bytecode, so the interpreter is covered without the compiler
in front of it.

Much of the suite asserts that a program's output matches CPython's, which is
the compatibility guarantee the project is built on.

## Freezing a program

`gecko build` freezes a program into a standalone executable. The compiled
bytecode is appended to a copy of `gecko-runner`, a stub holding only the VM and
the bytecode reader, so the result starts without parsing or compiling anything.

```sh
cargo build --release
./target/release/gecko build examples/fib.py -o fib
./fib
```

A plain `cargo build --release` runner links the full Rust standard library.
`scripts/build-runner.sh` rebuilds it against a size-optimized std, using
nightly `build-std` with the immediate-abort panic strategy, and drops the
result at `target/release/gecko-runner`. It needs a nightly toolchain with
`rust-src`:

```sh
rustup toolchain install nightly --component rust-src
./scripts/build-runner.sh
./target/release/gecko build examples/fib.py -o fib
```

That runner is about 268 KB, so a frozen program lands near 268 KB plus its
bytecode. CI holds it under 300 KiB, and the margin is thin, so a change that
adds much to the runtime will trip that gate.

gecko looks for the release runner next to itself, then in the cargo target
layout, so freezing from a debug gecko still embeds the small release runner.
`gecko build --debug` embeds a debug runtime instead, for debugging the runtime
itself.

## Installing packages

`gecko install` unpacks a pure-Python wheel into site-packages so any program
can import it.

```sh
gecko install some_package-1.0-py3-none-any.whl
```

site-packages lives under `GECKO_HOME` (default `~/.gecko`), and is searched
after the importing directory and `GECKO_PATH`. Pass `--to dir` to install
somewhere else. Wheels with compiled C extensions do not run, since gecko has no
CPython C ABI.

## Layout

Rust frontend and tooling under `crates/`, C runtime under `native/`. See
[ARCHITECTURE.md](ARCHITECTURE.md).
