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

Values are NaN-boxed `u64` words, see 01-object-model.md. The host never relies
on struct layout, it goes through accessor functions:

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

- memory: a heap cap. Allocating past it raises MemoryError.
- execution: a budget in steps or time. Going over it interrupts the VM.
- builtins: a policy that turns off or narrows what a script can reach, such as
  the filesystem, the network, and imports.

## Open

- The exact C header surface and its naming.
- Whether a host can register native functions that Python can call, in v0.0.4
  or later.
- How fine the interrupt granularity for the execution budget needs to be.
