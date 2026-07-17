# Object model

## Value representation

A Gecko value is a 64-bit word, encoded by NaN boxing.

An IEEE-754 binary64 has 2^52 - 1 bit patterns that are NaN. Any pattern that is
not a NaN is a float, stored inline. The NaN patterns encode everything else in
the payload, as a type tag plus 48 bits.

Encodings:

- float: any non-NaN double, stored as is.
- fixnum: a small signed integer, inline. Covers at least the signed 32-bit
  range.
- pointer: a heap object address of 48 bits, aligned so the low bits are free.
- singleton: None, True, False, and internal sentinels, each a fixed pattern.

Scalar float arithmetic is common in the target workloads, such as data
conditioning and numerics, so floats must not allocate. NaN boxing keeps floats,
small ints, and singletons immediate, and it keeps a value in a single register.
That also makes a value trivial to pass across the Rust and C boundary as a
`u64`.

Two alternatives were rejected:

- Low-bit tagged pointers are portable, but floats then box on the heap, which
  costs too much in float-heavy code.
- Handles, an indirection table, are safe for a moving GC and for FFI, but they
  add a load on every access. Worth reconsidering only if a moving GC needs them.

NaN boxing assumes 48-bit canonical user-space pointers, which holds on x86-64
and AArch64. A boxed-value fallback is possible for exotic targets and is out of
scope for now.

## Heap objects

Every value that is not immediate is a heap object with a one-word header. The
header holds:

- the type index: int, str, list, dict, tuple, function, code, module, type,
  instance, and so on.
- the GC bits: mark, color, and flags. See 03-gc.md.
- a length or size hint, where the type wants one.

The payload follows the header and depends on the type.

The types present at v0.0.1 are bignum int, str, list, dict, tuple, function,
code, and module. bool and None are singletons rather than heap objects.

## Integers

A fixnum covers the common case inline. Arithmetic that overflows the fixnum
range promotes to a heap bignum. Equality and hashing treat a fixnum and a
bignum of the same mathematical value as equal.

## Dictionaries

A dict keeps its entries in a dense array in insertion order, which is what
iteration, keys, values, and items walk. Lookup starts as a linear scan over
that array. Once a dict grows past a small threshold it also builds an open
addressing hash index beside the array: a table of entry positions probed by the
key's hash. The index gives O(1) lookup for the large dicts, module namespaces,
and class bodies that dominate name and attribute resolution, while a tiny dict
pays nothing and stays cache-friendly. Dicts never delete, so the index needs no
tombstones, and a value hashes the same however it was written, so an integer
key and an equal float key land in the same slot.

The index holds positions, not values, so the collector ignores it and a resize
of the entry array leaves it valid.

## Open

- The exact fixnum width, whether 30, 32, or wider, and the pointer tag layout.
  Fix both once the NaN-box constant table is written.
- The internal layout of str: ASCII against UTF-8 storage, and whether to inline
  small strings.
- Whether to intern small ints and short strings.
