# Data engine

Numeric work in Python leans on numpy and pandas, C extensions that hold their
data outside the interpreter. Gecko builds the same shape into the runtime:
typed arrays over columnar buffers, elementwise and reduction kernels that
vectorize, and data-parallel map, reduce, and filter over the v0.0.7 pool. The
data lives in immutable shared buffers, so an array passes between isolates by
handle instead of by copy.

A program that uses this API is still valid Python 3.12. The `gecko` package
provides the array type, and on CPython it falls back to numpy where present and
to plain Python lists otherwise, so the same source runs on both.

## Typed arrays

A typed array is a fixed-length, homogeneous sequence of numbers with a dtype.
The first cut carries four dtypes: `f64`, `f32`, `i64`, and `i32`. The set grows
later to the smaller integer widths and to bool.

```python
from gecko import array

xs = array([1.0, 2.0, 3.0, 4.0], dtype="f64")
ys = xs * 2.0 + 1.0
print(ys.sum())          # 20.0
print(ys[1])             # 5.0
zs = ys[1:3]             # a view, shares xs's buffer
```

An array is immutable. Indexing reads an element as a normal Python int or
float. Slicing returns another array that views the same buffer over a
sub-range, so a slice costs no copy. There is no in-place element assignment;
every operation that changes data returns a new array. Immutability is what lets
a buffer be shared, across a slice and across an isolate boundary, without a
lock or a copy.

The array object that lives in an isolate heap is small: a dtype, an offset, a
length, a stride, and a handle to the buffer that holds the bytes. The GC traces
the array object and drops the buffer handle when the array is collected. It
never traces into the buffer itself, since the buffer is not in the heap.

## Shared buffers

A shared buffer is a block of numbers allocated outside every isolate heap and
reference counted. It has a header (dtype, element count, byte capacity, and an
atomic refcount) and a payload of contiguous elements aligned to 64 bytes so the
kernels can load full SIMD registers off the start of the data.

The refcount is atomic because a buffer can be reachable from more than one
isolate at once. A handle is a pointer plus a refcount increment; dropping a
handle decrements, and the buffer is freed when the count reaches zero. An array
in a heap owns one handle, a slice of it owns another over the same buffer, and a
message that carries an array to another actor clones the handle rather than
copying the bytes.

Because a buffer is immutable once built, two isolates holding handles to it can
only read, so there is no data race on the elements and no lock on the read
path. This is the fast path 04-concurrency.md defers for messages: an actor that
sends an array sends a handle, and a pipeline of actors passing a column down a
chain moves a pointer at each hop instead of the column's bytes.

Building a buffer needs a brief mutable window. The constructor allocates the
payload, writes the elements, and only then publishes the handle; a kernel that
produces a buffer writes it fully on the worker that owns it before any other
isolate can name it. After publication the bytes never change.

## Kernels

The kernels are the elementwise and reduction operations over a dtype. Binary
elementwise: add, subtract, multiply, divide, and the comparisons, each in an
array-array form and an array-scalar broadcast form. Reductions: sum, product,
min, max, and count. Each kernel reads one or two input buffers and writes a
fresh output buffer of the result dtype.

A kernel is a tight loop over aligned, contiguous memory with no Python calls in
the body, so the C compiler auto-vectorizes it at `-O2` with the alignment known.
The first cut relies on that auto-vectorization rather than hand-written
intrinsics. Explicit SIMD with a runtime CPU-feature dispatch (SSE, AVX2, NEON)
is the step after, once the kernel set and the buffer layout are pinned and there
is a benchmark to hold it to.

Mixed dtypes promote by the usual numeric rules before the kernel runs: an
`i64` array times an `f64` scalar produces `f64`. Promotion allocates the
promoted input, so a caller that wants to avoid it keeps the dtypes matched.

## Parallel map, reduce, and filter

The kernels run data-parallel over the v0.0.7 scheduler. A parallel elementwise
op splits the index range into chunks and publishes the job to the pool; each
worker claims chunks from a shared counter and applies the kernel to its slice,
writing into a disjoint range of the output buffer. The chunks touch
non-overlapping output and read immutable input, so they never race, and the
caller blocks until the last chunk finishes. A reduction runs the same split,
each chunk reducing its slice to a partial and merging it into the accumulator.

The calling thread claims chunks too, rather than only waiting. That is what
makes the kernels safe to call from inside an actor handler: the handler is
already occupying a pool worker, and if every worker were busy the same way, a
caller that only waited would deadlock. Because the caller always makes progress
on its own job, the work completes whether or not any other worker picks it up,
and the parallelism is opportunistic.

A parallel op only pays off once the array is large enough to cover the
scheduling cost, so below a threshold the kernel runs inline on the calling
thread. Elementwise ops are memory-bandwidth-bound, so their speedup is bounded
well under the core count; reductions, which read without writing a result
buffer, scale further.

Float sum and product stay sequential. Reassociating them changes the rounding,
and a reduction that returns a different last bit depending on how the chunks
happened to be scheduled is not worth the speed. Min, max, and the integer sum
and product are exact under reassociation, so those run in parallel.

`array.map`, `array.reduce`, and `array.filter` are the higher-order surface over
this. They take a Python function and so run sequentially in the calling isolate,
for the reason a `call` blocks its worker in 04-concurrency.md: the function runs
in one isolate's VM and that VM cannot be shared across threads. The parallel
path is the one the operators and the reduction methods take, where the body is a
native kernel with no VM in it. Pushing a compiled kernel or a closure into
worker isolates is what would open the higher-order surface up, and that is
deferred.

`map` returns an array. It keeps the source dtype when the source is integral and
every result is an integer, and promotes to float when the source is float or any
result is. `filter` returns a shorter array of the same dtype, in two passes:
one to test each element and record which survive, one to compact the survivors
into the output buffer. `reduce` threads an accumulator through the function and
returns a scalar, taking an optional initial value.

Every op is eager: it materializes its output buffer before returning. Fusing a
chain of ops into one pass over the data, so `xs * 2.0 + 1.0` avoids the
intermediate buffer, needs an expression graph and a fusion pass, which is
deferred. It pairs naturally with the lazy evaluation the v0.0.9 dataframes call
for, so the two land together.

## Portable API

The array type lives in the `gecko` package and bridges to a native `_gecko`
module when it is present, the same way `actor` does. On Gecko the native side
allocates shared buffers and runs the kernels. On CPython, where the native
module is absent, `gecko.array` wraps numpy when it is installed and falls back
to a plain Python list otherwise, matching the eager semantics so results agree
across the two. The immutability and handle behavior are invisible on CPython,
where there are no isolates and an array is an ordinary object.

## Implementation for v0.0.8

The first cut ships the four dtypes, immutable reference-counted shared buffers
with 64-byte alignment, the elementwise and reduction kernels leaning on
auto-vectorization, parallel execution of those kernels over the existing pool,
the map, reduce, and filter surface, and array transfer by handle across actor
messages. It defers explicit SIMD intrinsics and CPU dispatch, the smaller dtypes
and bool, parallel execution of an arbitrary Python function, and kernel fusion.

A spawned isolate binds `array` the way it binds `actor`, so a handler can build
and compute on arrays without its module globals.

### Buffer allocation and the GC

Shared buffers are allocated by an allocator that is separate from the isolate
heaps and outlives any one isolate. The array object in a heap is a normal heap
object with one extra field, the buffer handle, and one extra step in its
finalizer, the handle drop. The collector already runs finalizers on sweep, so
the buffer's refcount rides on that path. A buffer never holds a pointer back
into any heap, so it is never a GC root and never traced.

### Transfer across a boundary

The message transfer in 04-concurrency.md gains one copyable kind: an array. In
the neutral node array a value carries its buffer handle with the refcount
already bumped, so the receiver installs a new array object over the same buffer
and the send copies no elements. This is the columnar fast path the concurrency
doc names; it only applies to arrays, whose immutability makes sharing safe,
while ordinary lists and dicts still deep copy.

## Open

- Whether an array over a Python-object dtype belongs here or waits for the
  dataframe layer, since object columns cannot share a buffer or vectorize.
- How a parallel op chooses its chunk count against array length and core count,
  and the length below which the sequential path wins.
- Whether filter's two-pass compaction beats a single pass that writes per-chunk
  outputs and concatenates, once there is a benchmark.
- The alignment and false-sharing behavior when many small arrays pack into one
  allocator region.
