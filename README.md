# 🦎 Gecko [![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/punctuations/gecko)

_A gecko is a tiny reptile. This is a tiny Python._

Gecko is a Python runtime built from scratch, for the cases where CPython spends
most of its time starting up. <br>
It runs the same programs, in a fraction of the startup time and the space.

```bash
$ ls -lh target/release/gecko
-rwxr-xr-x  737K  gecko*

# a frozen program, size-optimized runner
-rwxr-xr-x  265K  fib*
```

## Table of contents

- [Why Gecko?](#why-gecko)
- [Installation](#installation)
- [Benchmarks](#benchmarks)
- [Compatibility](#compatibility)
- [Concurrency](#concurrency)
- [Embedding](#embedding)
- [Building Gecko](#building-gecko)
- [Security](#security)
- [Contributing to Gecko](#contributing-to-gecko)

## Why Gecko?

|                         | Gecko               | CPython 3.14            |
| ----------------------- | ------------------- | ----------------------- |
| Startup, hello world    | **3.0 ms**          | 19.5 ms                 |
| Peak memory             | **1.8 MB**          | 13.5 MB                 |
| Install size            | **737 KB**          | 273 MB                  |
| Standalone binary       | **265 KB**          | none                    |
| Concurrency             | isolates and actors | threads, GIL by default |
| Arbitrary-precision int | yes                 | yes                     |
| C extension modules     | no                  | yes                     |

Gecko is built for environments where size and startup time decide things:
serverless functions, edge workers, embedded scripting, CLI tools, and short
scripts that do a bit of work and exit.

Gecko implements standard Python with no language extensions. A Gecko program is
a valid Python program and still runs on CPython, so LSPs, formatters, linters,
and type checkers go on working. The runtime is hand-built: a Rust frontend for
the lexer, parser, and compiler, and Setae, a C VM with a computed-goto
interpreter, NaN-boxed values, inline caches, and a precise mark-sweep collector.

## Installation

There is no prebuilt binary yet. Build from source with Rust 1.85 or newer, plus
Meson and Ninja for the C runtime:

```bash
pip install meson ninja
git clone https://github.com/punctuations/gecko
cd gecko
cargo build --release
./target/release/gecko examples/fib.py
```

To put `gecko` on your PATH:

```bash
cargo install --path crates/gecko
```

See [BUILDING.md](BUILDING.md) for supported platforms, the size-optimized
runner, and how to freeze a program into a standalone binary.

## Benchmarks

Scripts are in [benchmarks/](benchmarks/). They run unmodified on both runtimes
with byte-identical output, apart from `arrays.py`, which uses the gecko-only
typed-array API. Measured with hyperfine against CPython 3.14.6.

### Startup

The case Gecko is built for. Loads a script, prints, and exits.

```bash
hyperfine -N --warmup 20 --runs 300 \
  'target/release/gecko benchmarks/startup.py' \
  'python3.14 benchmarks/startup.py'
```

| Runtime      | Mean       | Min     | Max     | Relative     |
| ------------ | ---------- | ------- | ------- | ------------ |
| **Gecko**    | **3.0 ms** | 2.7 ms  | 3.5 ms  | **1.00**     |
| CPython 3.14 | 19.5 ms    | 18.7 ms | 21.9 ms | 6.62x slower |

A frozen binary starts in 2.9 ms, since it parses and compiles nothing.

### Compute

Ahead on all five.

| Benchmark                     | Gecko    | CPython 3.14 | Result       |
| ----------------------------- | -------- | ------------ | ------------ |
| `arithmetic.py`, 3M-iter loop | 143.0 ms | 232.7 ms     | 1.63x faster |
| `calls.py`, 600k calls        | 77.8 ms  | 115.9 ms     | 1.49x faster |
| `fib.py`, recursive `fib(25)` | 20.4 ms  | 33.5 ms      | 1.64x faster |
| `sieve.py`, primes to 1M      | 129.3 ms | 132.9 ms     | 1.03x faster |
| `wordcount.py`, dict and str  | 122.8 ms | 135.5 ms     | 1.10x faster |

Integers above 140737488355327 leave the unboxed range and slow down by about
3x.

<details>
<summary>Environment</summary>

| Detail    | Value                       |
| --------- | --------------------------- |
| Hardware  | Apple M1, 8 GB RAM, 8 cores |
| OS        | macOS 26.4.1 (arm64)        |
| Gecko     | 0.0.8                       |
| CPython   | 3.14.6                      |
| hyperfine | 20 warmup, 300 timed runs   |

</details>

## Compatibility

Gecko runs a large subset of Python end to end: the full expression and
statement grammar (f-strings with format specs, comprehensions and generator
expressions, slicing, the walrus operator, `match`, `with`, decorators, and
`try`/`except`/`else`/`finally`), the whole call convention (keyword arguments,
defaults, `*args`, `**kwargs`, spreads), closures with `nonlocal`, generators,
`async`/`await`, classes with single inheritance, and `import`/`from ... import`.

Built-in types are int, float, bool, str, list, tuple, dict, set, frozenset,
range, and typed arrays. Integers are arbitrary precision. Sets of integers
iterate in CPython's order.

Constructs outside the supported grammar are rejected at compile time with a
located error. There is no standard library yet, and wheels with compiled C
extensions do not run. The test suite is 303 tests, most asserting that output
matches CPython's.

For the full runtime surface, the builtins, the types and their methods, and
what is missing, see [docs/design/06-builtins.md](docs/design/06-builtins.md).

## Concurrency

No GIL. Concurrency comes from isolates: independent runtimes with their own
heap and collector that share no mutable state. An actor is an isolate with a
mailbox and a handler, in Gleam's shape, and they run on an M:N work-stealing
thread pool.

```python
from gecko import actor

def handle(state, message):
    message[1].send(state + message[0])
    return state + message[0]

counter = actor.spawn(0, handle)
print(counter.call(lambda reply: [7, reply], 1000))
```

Messages are deep copied between isolates, except subjects and typed arrays,
which pass by handle. See
[docs/design/04-concurrency.md](docs/design/04-concurrency.md).

## Embedding

A host can run many isolated VMs, cap each one's steps, wall-clock time, and
heap, and register native functions that scripts call like builtins. A program
can run other code under those limits through the builtin `sandbox` module:

```python
from gecko import sandbox

print(sandbox.run('print(2 ** 64)', 100000, 1000, 50))
```

See [docs/design/05-embedding.md](docs/design/05-embedding.md).

## Building Gecko

See [BUILDING.md](BUILDING.md) for instructions on building Gecko from source
and a list of supported platforms.

## Security

For information on reporting security vulnerabilities in Gecko, see
[SECURITY.md](SECURITY.md).

## Contributing to Gecko

Contributions are welcome through pull requests. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the prerequisites, the layout, and the
style the project follows. <br>
For the design decisions behind the runtime, see
[ARCHITECTURE.md](ARCHITECTURE.md) and [docs/design/](docs/design/), and for what
is planned next, [ROADMAP.md](ROADMAP.md).

## License

MIT. See [LICENSE](LICENSE).
