# Embedding

Gecko is embeddable. A host program creates one or more runtimes and runs
scripts inside them, under resource limits.

## Layers

- Setae, the C VM, exposes a stable C ABI, declared in native/include.
- A Rust crate wraps that ABI in a safe interface:

```rust
let vm = Gecko::new();
vm.execute(script);
```

The C ABI is the contract. The Rust wrapper and the CLI are both clients of it,
and another language can bind the same C header.

## Instances

Each `Gecko` instance is one isolate, with its own heap, globals, and collector.
See 04-concurrency.md. A host may create many of them and run them on separate
threads, but a single instance is never shared across threads at once.

## Values across the boundary

Values are NaN-boxed `u64` words, see 01-object-model.md. The host goes through
accessor functions and never depends on struct layout:

- classify: is_int, is_float, is_str, is_none, and so on.
- read: as_i64, as_f64, str_bytes, and so on.
- build: new_int, new_float, new_str, new_list, and so on.

A value carrying a pointer stays rooted for as long as the host holds its
handle, so a collection will not free it. See 03-gc.md.

## Errors

A call that can fail returns a status, and on failure an exception handle with
it. The host asks the exception for its type and message. No exception crosses
the C ABI by unwinding.

## Limits and sandboxing

Each instance has:

- memory: a live-object cap set with setae_heap_set_limit. Allocating past it,
  after a collection has run, raises a MemoryError the program can catch.
- execution: a step budget set with setae_vm_set_step_limit. The VM counts
  instructions and stops when the count passes the limit. The interrupt is not
  an exception the program can catch, so a runaway loop cannot swallow it.
- time: a wall-clock budget in milliseconds set with setae_vm_set_time_limit.
  The VM checks a monotonic clock against a deadline captured at run start,
  every few thousand instructions. Like the step budget the interrupt is not
  catchable. The step budget is deterministic where the time budget is not, so
  the two guard different things: a fixed cost regardless of machine, and a real
  ceiling on how long a run may take.
- builtins: a policy that turns off or narrows what a script can reach, such as
  the filesystem, the network, and imports.

The memory cap counts live objects rather than bytes, which is a coarse proxy.
Byte-accurate accounting waits for the allocator rewrite in v0.0.5.

## The sandbox module

A program can run other gecko code under limits through `sandbox`, reached with
`from gecko import sandbox`. `sandbox.run(source, steps, memory, millis)`
compiles and runs the source in a fresh isolated VM and returns its captured
output as a string. The three limits are optional and default to unlimited.
Failure inside the sandbox, including a step, memory, or time limit, surfaces as
a catchable SandboxError in the caller. Sandboxed code cannot import files, since it is
compiled with no source directory. The host installs the compile-and-run hook,
so a bare runtime with no compiler reports that the sandbox is unavailable.

The native capability lives in a reserved builtin module named `_gecko`, which
follows CPython's split between a C accelerator like `_socket` and its Python
wrapper `socket`. The plain name `gecko` resolves the search path first, so an
installed `gecko` package wins, and falls back to `_gecko` when none is present.
A later pure-Python `gecko` package will provide the public API on top of
`_gecko` when it is present, and its own implementation otherwise, so the same
code runs on gecko and on CPython.

## Host functions

A host registers a native function with `setae_vm_register_builtin`, wrapped by
`Vm::register_fn(name, f)` on the Rust side. The function has the builtin
signature, taking the VM and the argument values and returning a value, so a
script calls it like any builtin. It reads and builds values through the same
accessor functions the runtime uses across the boundary. The sandbox hook is a
specialized instance of the same mechanism.

## Open

- The exact C header surface and its naming.
- Passing host state to a registered function, rather than a bare function
  pointer.
- Byte-accurate memory accounting. The cap counts live objects, not bytes.
