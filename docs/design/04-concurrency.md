# Concurrency

No GIL and no shared-memory threads. Parallelism comes from running many
isolates at once.

## Isolates

An isolate is an independent runtime, with one VM, one heap, one collector, and
one set of globals. Isolates share no mutable state. An isolate runs on at most
one OS thread at a time, so reaching an object inside it needs no locks.

## Actors

An actor is an isolate with a mailbox and a message handler, following Gleam's
shape. It holds state and runs the handler once per message, in order, threading
the state through:

```python
def handle(state, message):
    return new_state

actor = gecko.spawn(initial_state, handle)
actor.send(message)                  # cast, fire and forget
answer = actor.call(build, timeout)  # request-reply, see Subjects
```

An actor processes one message at a time, so its own state never races, and
because actors share no mutable objects there are no data races on values.
`spawn` returns a subject bound to the actor's mailbox.

## Messages

Sending a value to another actor sends it by value, so the object graph is deep
copied into the receiver's heap. Two things instead pass by handle:

- A subject rides inside a message as a handle to the same mailbox, not a copy,
  which is how a reply routes back to the caller that sent it.
- Immutable shared buffers, holding columnar data, pass by handle and are
  reference counted, so large data moves without copying its bytes (see
  03-gc.md). This one is deferred; copying is the right default until it pays.

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

## Implementation for v0.0.6

This is the plan for the first cut. It ships isolates on real OS threads,
stateful actors in Gleam's shape, subjects that pass by handle, deep-copied
messages, and both cast and call. It defers the immutable columnar-buffer fast
path, the work-stealing scheduler (that is v0.0.7), bounded mailboxes, and
supervision.

### What crosses an isolate boundary

An isolate owns its heap, so nothing in it can be reached from another thread by
pointer. Three things cross the boundary: the handler's code, once, at spawn;
each message, on send and call; and subjects, which travel inside messages by
handle rather than by copy.

Code objects cannot be shared, even read-only, because inline caches write into a
code object's cache array at run time and those entries hold shapes and values
from one particular heap. Two isolates running the same code object would race on
that array and read each other's heaps. So each isolate gets its own code.

### The actor shape

An actor holds state and a handler and runs the handler once per message, in
order, threading the state through. The handler is `handle(state, message)` and
returns the next state:

```python
def handle(state, message):
    kind = message[0]
    if kind == "add":
        return state + message[1]
    if kind == "get":
        message[1].send(state)   # message[1] is a reply subject
        return state
    return state

counter = gecko.spawn(0, handle)
counter.send(("add", 5))
answer = counter.call(lambda reply: ("get", reply), 1000)
```

`gecko.spawn(initial_state, handle)` starts the actor on its own thread and
returns a subject bound to its mailbox. Returning `gecko.stop()` from the handler
ends the actor; otherwise the return is the next state.

### Subjects

A subject is a handle to a mailbox that some actor receives from, and sending to
it appends to that mailbox. The subject `spawn` returns is the actor's own
mailbox. A subject is the one value that crosses an isolate boundary by handle:
put a subject in a message and the receiver gets a handle to the same mailbox,
not a copy, which is what lets a reply find its way back.

Underneath, a subject wraps a multi-producer sender for a mailbox and the owning
actor holds the single receiver. A sender is safe to move and clone across
threads, so any isolate holding a subject can send to it.

### Cast and call

`actor.send(message)` is a cast: it drops the message in the mailbox and returns
at once.

`actor.call(build, timeout)` is request-reply. It makes a fresh one-shot reply
subject, calls `build(reply)` so the caller can place that reply subject inside
the message, sends the message, and blocks on the reply subject until the handler
answers or the timeout elapses. The handler pulls the reply subject out of the
message and does `reply.send(result)` before returning its next state, the same
way a Gleam handler replies to the subject a call passed in.

### Messages

A message is deep copied out of the sender's heap and rebuilt in the receiver's,
except subjects, which pass by handle. The transfer walks a value into a
heap-neutral form the mailbox can carry, and the receiver walks that back into
its own heap. Copyable types are None, bool, int, float, str, list, tuple, and
dict. A subject in the graph is carried as its live sender, not copied. Any other
type, a function, class, instance, module, range, or iterator, raises TypeError
at the send, naming it. The walk keeps an identity map, so a graph with shared
sub-objects or a cycle transfers once and rebuilds with the same sharing.

The neutral form is a Rust value tree, not a byte buffer, because it has to hold
both the copied data and the live senders for any subjects it carries. That tree
is what the mailbox moves between threads, and it is Send since its leaves are
scalars, owned strings, and senders.

### Function transfer

`spawn` serializes the handler's code with the bytecode writer, the same format
`gecko build` freezes, and the child rebuilds it into its own code tree with a
fresh, empty cache array, so no code object and no inline cache is shared.

The first cut requires the handler to be a plain top-level function: no free
variables to capture, and no module globals beyond its parameters and the
builtins, since only the handler's own code subtree travels, not the module
around it. Nested functions and classes defined inside it do travel, because they
are child code objects the writer already includes. Closures and module-global
access are a later step that transfers more of the surrounding code.

### Isolates on threads

An actor is one OS thread that creates its own VM and heap inside the thread, so
no VM ever moves between threads. Thread per actor is the v0.0.6 model; v0.0.7
puts many actors on a fixed pool. The loop is: block on the mailbox receiver,
rebuild the message in the local heap, run `handle(state, message)`, take its
return as the next state, and repeat, until a stop or the last sender is gone.

### Native wiring

A spawn hook installs on the VM the way the sandbox hook does: the gecko crate,
which has both the compiler and the runtime, provides it, and `_gecko.spawn`
calls through it. A subject is a native object in the heap whose `send` and
`call` methods push to and wait on its mailbox. It is reference counted, and the
actor's thread is joined when its subject and every copy of it are gone.

### Errors

If the handler raises during a call, the failure is sent to the pending reply
subject and call re-raises it at the caller, so it stays visible at the call
site. If it raises during a cast there is no one waiting, so the actor stops and
its mailbox closes, and later sends see a closed mailbox. Supervision, restart
and failure trees, is deferred.

### Portable API

The pure-Python `gecko` package implements the same `spawn`, `send`, and `call`
on CPython over threads, so a program written against the API runs on both. The
native path lands first and the fallback follows once its behavior is pinned
down.

## Open

- What a message copy costs on a large object graph, and where the immutable
  fast path starts to pay off.
- Backpressure and bounded mailboxes.
- Supervision and failure propagation between actors.
- Transferring closures and the module globals a function reads, so spawn takes
  any function and not only a self-contained one.
