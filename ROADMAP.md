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
- v0.0.5, optimization. Hidden classes, inline caches, computed-goto dispatch,
  constant folding, hash-indexed dicts and namespaces, and pooled call frames.
  The frozen-program runner drops below the Rust std floor of about 300 KB
  through build-std. Bytecode specialization was tried and dropped, the release
  build already inlines the dispatch, and SSA passes are deferred to their own
  milestone.
- v0.0.6, multicore. Isolates on OS threads, stateful actors in Gleam's shape,
  and message passing. `from gecko import actor` gives `actor.spawn(state,
  handle, args)`, which returns a subject; the subject casts with `send` and does
  request-reply with `call(build, timeout)`, raising TimeoutError past the
  deadline. Messages deep-copy across the heap boundary, subjects travel inside
  them by handle so a reply routes back. A stop sentinel, routing a handler
  failure back through a call's reply, and the CPython fallback are deferred.
- v0.0.7, scheduler. Work stealing, thread pools, timers, message queues.
- v0.0.8, data engine. Typed arrays, SIMD, shared buffers, parallel map, reduce,
  and filter.
- v0.0.9, dataframes. CSV load, columnar storage, filter, join, group-by,
  aggregation, lazy evaluation.
- v0.1.0, distribution. `pip install gecko`, a CPython and PyPy compatibility
  layer, native acceleration.
- v0.1.1, reflection. A `types` module in the spirit of CPython's, naming the
  runtime type objects (function, module, code, and the concurrency types like
  Subject) so a program can use them in isinstance checks and annotations and
  construct them at runtime.
- v0.2.0, HPy. Universal ABI, extension SDK, native modules.
- v0.3.0, production runtime.

Open after that. JIT, AOT, wasm target, distributed actors, incremental GC.
