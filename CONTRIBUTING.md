# Contributing

## Build and run

See BUILDING.md for the prerequisites, the supported platforms, and how to
freeze a program into a standalone binary.

```sh
cargo build
cargo run -p gecko -- --version
cargo run -p gecko -- examples/sum.py
cargo run -p gecko -- -c 'print("hello world")'
```

## Tests

```sh
cargo test
```

Tests sit next to the code they cover. The tests that run source text through
the whole pipeline live in crates/gecko. crates/runtime holds VM-level tests
that hand-assemble bytecode, so the interpreter is covered without the compiler
in front of it.

## Layout

Rust frontend and tooling under crates/, C runtime under native/. See
ARCHITECTURE.md.

## Style

- Match the surrounding code in naming, formatting, and idiom.
- Run `cargo fmt` and `cargo clippy` before opening a change.
- Write code without comments. Let naming carry intent, and put explanations in
  the docs or the change description.
- ASCII only, in source and docs.

## Commits

- One logical change per commit.
- Subject in the imperative, under about 72 characters. When the reason for the
  change is not obvious, give it in the body.

## Versioning

Versions are MAJOR.MINOR.PATCH.

- A small change bumps PATCH, so 0.0.1 becomes 0.0.2.
- A large change bumps MINOR, so 0.1.0 becomes 0.2.0.
- A large change bumps MAJOR instead when MINOR would pass 9, so 0.9.0 becomes
  1.0.0.

Bumping a field resets the smaller ones to 0.
