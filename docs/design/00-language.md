# Language and compatibility

## Target

Gecko implements the Python 3.12 language.

The parser accepts the full 3.12 grammar. Anything the runtime cannot yet run is
rejected at compile time with a located error, never run incorrectly and in
silence.

## Compatibility guarantees

1. A valid Gecko program is a valid Python 3.12 program. It parses and runs on
   CPython 3.12 and on PyPy.
2. For supported features, observable behavior matches CPython 3.12, apart from
   the deviations below.
3. Standard tools such as LSPs, formatters, linters, and type checkers work,
   because the source is ordinary Python.

## Deviations from CPython

- No C ABI compatibility. Extensions target the Gecko native API or HPy, not the
  CPython C API.
- No reference-counting semantics. Finalization has no fixed order or timing, so
  the timing of `__del__` is unspecified. See 03-gc.md.
- No GIL to observe. Threading follows the isolate and actor model rather than
  shared-memory threads. See 04-concurrency.md.
- No introspection into the running frame stack. `sys._getframe`, `settrace`,
  and `setprofile` are out of scope for v0.0.x.

## Core type semantics

- int: arbitrary precision. Small values stay immediate as a fixnum and promote
  to a heap bignum on overflow. See 01-object-model.md.
- float: IEEE-754 binary64.
- bool: a subtype of int, with True and False as singletons.
- str: a sequence of Unicode code points, stored as UTF-8 with a fast path for
  ASCII.
- None: a single instance.

## Open

- Indexing a str. UTF-8 storage makes code-point indexing O(n) without a side
  table of offsets, where CPython gives O(1). Either cache an index on the first
  random access or accept O(n). The bias is to accept O(n) and optimize later.
- Which stdlib modules are in scope for v0.0.x, and which wait for the packaging
  phase in v0.0.3.
