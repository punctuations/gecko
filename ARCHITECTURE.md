# Architecture

Gecko is part Rust and part C. Rust turns source text into bytecode and provides
the tooling. C runs the bytecode and manages memory and concurrency. The
bytecode VM is named Setae.

```
Source Code
    |
    v
Lexer -> Parser -> AST        (Rust, crates/)
    |
    v
Compiler / SSA / Optimizer    (Rust, crates/)
    |
    v
Bytecode
    |
    v
Setae (VM) ----------+        (C, native/)
    |                 |
    v                 v
Garbage Collector   Scheduler --> Actors
```

## Why the split

Rust handles the frontend and the tooling. It is memory safe, and a parser,
compiler, optimizer, package manager, and import system are pleasant to write
in it.

C handles the runtime hot paths. It gives a small, portable, embeddable VM with
a predictable memory layout, which the object model, GC, and scheduler all need.

## Repository layout

```
crates/            Rust workspace (frontend and tooling)
  gecko/           the `gecko` binary, the entry point
  lexer/           tokenizer
  ast/             syntax tree types
  parser/          tokens to AST
  bytecode/        code objects and opcodes, shared by compiler and runtime
  compiler/        AST to bytecode
  runner/          the gecko-runner stub that frozen programs run on
  runtime/         safe API and FFI bindings over the C runtime
                   later, ssa and pkg
native/            C runtime, holding Setae (the VM), GC, scheduler, actors
  include/         public C ABI (setae.h)
  src/             value encoding, heap, code objects, interpreter, builtins
docs/design/       Phase 0 specifications
examples/          Python programs the runtime can run
```

Meson builds the C runtime as a static library. crates/runtime/build.rs runs
Meson as part of the build, so `cargo build` is the only command needed.
