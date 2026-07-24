# Builtins and runtime surface

This lists what a program can use without importing anything: the builtin
functions, the builtin types and their methods, and the operators. It also
records what is not there yet, so the gaps are in one place. Everything here is
checked against CPython; where behavior differs it says so.

## Builtin functions

| Function | Notes |
| --- | --- |
| `print(*args)` | Space-separated, newline at the end. No `sep`, `end`, `file`, or `flush` yet. |
| `len(x)` | str, list, tuple, dict, set, range. |
| `range(stop)` / `range(start, stop[, step])` | Lazy, iterable and indexable. |
| `type(x)` | Returns the type. `type(5)` reprs as `<class 'int'>`. |
| `isinstance(x, cls_or_tuple)` | Handles `bool` as a subclass of `int`, the exception hierarchy, and user classes. |
| `issubclass(cls, cls_or_tuple)` | Walks the single-inheritance chain; `bool` is a subclass of `int`. |
| `callable(x)` | True for functions, builtins, classes, bound methods, exception types. |
| `getattr(o, name[, default])`, `setattr(o, name, v)`, `hasattr(o, name)` | Instances, classes, modules. |
| `iter(x)`, `next(it)` | Iterators over the built-in iterables and generators. |
| `hash(x)`, `id(x)` | `hash` rejects unhashable values; the value is gecko's own, not CPython's. |
| `repr(x)`, `abs(x)`, `round(x[, n])`, `divmod(a, b)`, `pow(a, b[, mod])` | `round` uses banker's rounding; `pow` has the 3-argument modular form. |
| `ord(c)`, `chr(i)` | Full Unicode code points. |
| `hex(i)`, `oct(i)`, `bin(i)` | With sign and `0x`/`0o`/`0b` prefix. |
| `sorted(x, *, key, reverse)`, `reversed(x)` | `sorted` is stable; `reversed` is lazy. |
| `enumerate(x, *, start)`, `zip(*xs)`, `map(f, x)`, `filter(f, x)` | All lazy iterators. |
| `sum(x[, start])`, `min(...)`, `max(...)`, `any(x)`, `all(x)` | `min`/`max` take `key` and `default`. |
| `str`, `int`, `float`, `bool`, `list`, `tuple`, `dict`, `set`, `frozenset` | Constructors, also usable as types. `int` parses arbitrary-length decimal strings. |

The builtin exception types (`ValueError`, `KeyError`, `TypeError`, and the
rest) are also names in scope, both to raise and to catch.

Keyword arguments reach a builtin only where listed above: `enumerate(start=)`,
`sorted(key=, reverse=)`, `min`/`max(key=, default=)`, `dict(**kwargs)`. Other
builtins reject keyword arguments, as CPython does.

## Types and their methods

Integers are arbitrary precision. Arithmetic that leaves the 32-bit range
promotes to a wide integer rather than a float, so `2 ** 1000` and factorials
stay exact. The one rough edge is true division (`/`) of very large integers,
which can differ from CPython in the last floating-point digit.

### str

| Method | |
| --- | --- |
| `split`, `rsplit`, `splitlines`, `join` | `split()` with no argument splits on whitespace. |
| `strip`, `lstrip`, `rstrip` | Optional set of characters to strip. |
| `upper`, `lower`, `title`, `capitalize`, `swapcase` | ASCII only; accented letters are left unchanged. |
| `replace`, `startswith`, `endswith` | `startswith`/`endswith` accept a tuple. |
| `find`, `rfind`, `index`, `rindex`, `count` | Character offsets, not byte offsets. |
| `isdigit`, `isalpha`, `isalnum`, `isspace`, `isupper`, `islower` | |
| `zfill`, `ljust`, `rjust`, `center` | |
| `removeprefix`, `removesuffix` | |

### list

`append`, `extend`, `insert`, `remove`, `index`, `count`, `pop`, `sort` (with
`key` and `reverse`), `reverse`, `clear`, `copy`. Lists support `in`, indexing,
slicing (`a[i:j:k]`), and lexicographic comparison.

### dict

`get`, `keys`, `values`, `items`, `setdefault`, `pop`, `popitem`, `update`,
`clear`, `copy`, and the `dict.fromkeys(iterable[, value])` classmethod.
Iteration is in insertion order, matching CPython.

### set and frozenset

`add`, `discard`, `remove`, `pop`, `clear`, `copy`, `union`, `update`,
`intersection`, `difference`, `symmetric_difference`, `issubset`, `issuperset`,
`isdisjoint`. The operators `|`, `&`, `-`, `^` and the subset comparisons
(`<`, `<=`, `>`, `>=`) work too. Sets iterate in CPython's order, not insertion
order; see [01-object-model.md](01-object-model.md) for how that is matched and
the one large-literal boundary where it is not. A frozenset is hashable, so it
can be a dict key or a set element, and its mutating methods raise.

Tuples have no methods yet (`count` and `index` are missing), though they
support indexing, slicing, `in`, unpacking, and comparison.

## Operators

Arithmetic is `+`, `-`, `*`, `/` (true division, always a float), `//`, `%`,
and `**`. The bitwise operators are `&`, `|`, `^`, `<<`, `>>`, and unary `~`,
on integers and bools. Comparisons, `in`/`not in`, `is`/`is not`, and the
boolean `and`/`or`/`not` all work, and comparisons chain (`0 < x < 10`).
f-strings support the format-spec mini-language, so `f"{x:.2f}"`, `f"{n:>8}"`,
`f"{n:,}"`, and `f"{p:.1%}"` format as they do on CPython.

## Not available yet

Functions: `object`, `super`, `property`, `staticmethod`, `classmethod`,
`delattr`, `open`, `input`, `eval`, `exec`, `compile`, `globals`, `locals`,
`vars`, `dir`, `format`, `ascii`, `slice`, `complex`, `bytes`, `bytearray`,
`memoryview`.

str methods: `format`, `format_map`, `encode`, `expandtabs`, `partition`,
`rpartition`, `translate`, `maketrans`, `casefold`, `isidentifier`.

Types: complex numbers, real `bytes` and `bytearray` (a `b'...'` literal parses
but behaves like a limited string), and `memoryview`.

Operators and syntax: `%` string formatting, sequence repetition (`"ab" * 3`,
`[0] * 3`), dict merge (`d1 | d2`), the `@` operator, slice assignment
(`a[i:j] = ...`) and slice deletion, starred assignment targets (`a, *b = ...`),
unpacking inside literals (`[*a]`, `{**d}`, `{*s}`), keyword-only and
positional-only parameters, `raise X from Y`, `yield from`, `async for`, and
`async with`.

Classes: multiple inheritance, `super()`, `@classmethod`, `@staticmethod`,
`@property`, and operator overloading. Only `__init__`, `__enter__`/`__exit__`,
and `__await__` are honored on user classes; the other special methods
(`__str__`, `__repr__`, `__eq__`, `__hash__`, `__bool__`, `__len__`,
`__getitem__`, `__iter__`, `__contains__`, `__call__`, the comparison methods,
and the arithmetic methods) are ignored, so a class cannot yet override how it
prints, compares, or responds to operators. Exceptions carry a single message
argument; multiple arguments and the `.args` attribute are not there.

The standard library is absent. There is no `math`, `sys`, `os`, `json`, `re`,
`random`, `datetime`, `collections`, `itertools`, or `functools`. Only the
built-in `sandbox` module and the `gecko`/`actor` modules exist; importing your
own modules and packages works.
