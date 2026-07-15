# Roadmap

The milestones are ordered and carry no dates. Each one assumes the ones before
it.

- Phase 0, specifications. Object model, bytecode format, GC, concurrency model,
  embedding API.
- v0.0.1, minimal interpreter. Lexer, parser, compiler, bytecode, VM. Types int,
  float, str, list, dict, and function. Variables and modules. Runs
  `print("hello world")`.
- v0.0.2, classes, exceptions, closures, comprehensions, decorators, imports.
- v0.0.3, packaging. site-packages, package imports, wheels, module resolution.
- v0.0.4, embedding. Many VMs per process, sandboxing, memory and time bounds.
- v0.0.5, optimization. Hidden classes, inline caches, bytecode specialization,
  constant folding, SSA passes. Shrinking the frozen-program runner below the
  Rust std floor of about 300 KB, through build-std or a no_std stub.
- v0.0.6, multicore. Isolates, actors, channels.
- v0.0.7, scheduler. Work stealing, thread pools, timers, message queues.
- v0.0.8, data engine. Typed arrays, SIMD, shared buffers, parallel map, reduce,
  and filter.
- v0.0.9, dataframes. CSV load, columnar storage, filter, join, group-by,
  aggregation, lazy evaluation.
- v0.1.0, distribution. `pip install gecko`, a CPython and PyPy compatibility
  layer, native acceleration.
- v0.2.0, HPy. Universal ABI, extension SDK, native modules.
- v0.3.0, production runtime.

Open after that. JIT, AOT, wasm target, distributed actors, incremental GC.
