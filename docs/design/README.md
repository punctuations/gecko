# Design (Phase 0)

Each file states a decision, the reasoning, and the open points. Later files
assume the earlier ones.

- 00-language.md: supported Python version and compatibility guarantees.
- 01-object-model.md: value and object representation.
- 02-bytecode.md: code object and instruction set.
- 03-gc.md: garbage collector.
- 04-concurrency.md: isolates, actors, channels.
- 05-embedding.md: host embedding API and the Rust/C boundary.
- 06-builtins.md: builtin functions, types and their methods, and what is not there yet.
- 07-data-engine.md: typed arrays, shared buffers, kernels, and parallel map/reduce/filter.
