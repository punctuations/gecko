# gecko standard library

The gecko builtins, as a pure-Python package. This is scaffolding for the
distributable that a future milestone ships to PyPI, so `from gecko import
sandbox` works on plain CPython, not only on the gecko runtime. It is not built
or tested by the Rust workspace.

On CPython the native module `_gecko` is absent, so `import _gecko` raises
ImportError and each function runs a pure-Python fallback. When `_gecko` is
present a function forwards to it for the native fast path. gecko can compile and
run this package directly (point `GECKO_PATH` at `src`), in which case the native
path is taken; in a normal install gecko serves the same import from its own
builtins and this package is the CPython side.

```python
from gecko import sandbox

out = sandbox.run("print(6 * 7)")
```

## Layout

Each future gecko builtin gets its own module under `src/gecko/`. Today that is
`sandbox`. The pattern for a module is:

- import `native` from `gecko._native`
- if `native` is not None, call into `_gecko`
- otherwise run the pure-Python fallback

`gecko._native` does the one `try: import _gecko` and hands back either the
module or None.

## Open

The pure-Python sandbox fallback runs without the step and memory limits, since
enforcing those in plain CPython needs machinery this scaffold does not carry.
The distribution name can differ from the import name if PyPI already has a
`gecko`, the way `pillow` installs as `PIL`.

On gecko, `class SandboxError(Exception)` here produces the same exception the
native sandbox raises, so `except SandboxError` catches both. Matching a
subclass against a base other than Exception is not modeled yet, so a future
exception that subclasses, say, ValueError would not be caught by `except
ValueError` on gecko.
