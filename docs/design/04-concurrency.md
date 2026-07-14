# Concurrency

No GIL and no shared-memory threads. Parallelism comes from running many
isolates at once.

## Isolates

An isolate is an independent runtime, with one VM, one heap, one collector, and
one set of globals. Isolates share no mutable state. An isolate runs on at most
one OS thread at a time, so reaching an object inside it needs no locks.

## Actors

An actor is an isolate with a mailbox and an event loop. The Python-level API:

```python
worker = gecko.spawn(function)
worker.send(message)
result = worker.recv()
```

An actor processes one message at a time. Because actors share no mutable
objects, there are no data races on Python values.

## Messages

Sending a value to another actor sends it by value, so the object graph is deep
copied into the receiver's heap. Two things avoid that copy:

- Immutable shared buffers, holding columnar data, pass by handle and are
  reference counted, so large data moves without copying its bytes. See
  03-gc.md.
- A value that is already immutable and shareable may pass by handle as an
  optimization. This is deferred, since copying is the right default.

The copy walk handles cycles in the graph it transfers.

## Channels

A channel is a typed queue between actors. A send appends to it, and a recv
blocks or yields until a value arrives. A mailbox is the built-in channel each
actor has, and an explicit channel generalizes that for fan-in and fan-out.

## Scheduling

v0.0.6 brings in isolates, actors, and channels, running on a thread per actor
or on a small fixed pool. v0.0.7 replaces that with an M:N work-stealing
scheduler, where many actors run over a pool of OS threads and an actor is
pinned to one thread only while it handles a message.

## Portable API

The same `gecko` API runs on CPython by falling back to multiprocessing and
thread pools, so code written against it stays portable. On Gecko it uses the
native scheduler instead.

## Open

- What a message copy costs on a large object graph, and where the immutable
  fast path starts to pay off.
- Backpressure and bounded mailboxes.
- Supervision and failure propagation between actors.
