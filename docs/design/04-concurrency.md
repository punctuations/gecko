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
from gecko import actor

def handle(state, message):
    return new_state

worker = actor.spawn(initial_state, handle)
worker.send(message)                  # cast, fire and forget
answer = worker.call(build, timeout)  # request-reply, see Subjects
```

An actor processes one message at a time, so its own state never races, and
because actors share no mutable objects there are no data races on values.
`actor.spawn` returns a subject bound to the actor's mailbox.

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

v0.0.6 brought in isolates, actors, and channels on a thread per actor. v0.0.7
replaces that with an M:N work-stealing scheduler: a fixed pool of worker threads
(one per core) runs many actors, and an actor is pinned to one worker only while
it handles a message. See "Scheduler for v0.0.7" below.

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

from gecko import actor

counter = actor.spawn(0, handle)
counter.send(("add", 5))
answer = counter.call(lambda reply: ("get", reply), 1000)
```

`actor.spawn(initial_state, handle)` registers the actor with the scheduler and
returns a subject bound to its mailbox; the actor runs on a pool worker whenever
it has a message. A third argument, `actor.spawn(state,
handle, args)`, passes a list or tuple whose items reach the handler as extra
parameters after `state` and `message`. Returning `actor.stop()` from the handler
ends the actor; otherwise the return is the next state. Without a stop an actor
runs until the last sender to its mailbox drops. A handler runs without its module
globals (see Function transfer), so the child binds `actor` for it, which is how
`actor.stop()` resolves inside the handler.

### Subjects

A subject is a handle to a mailbox that some actor receives from, and sending to
it appends to that mailbox. The subject `spawn` returns is the actor's own
mailbox. A subject is the one value that crosses an isolate boundary by handle:
put a subject in a message and the receiver gets a handle to the same mailbox,
not a copy, which is what lets a reply find its way back.

Underneath, an actor subject is a reference-counted handle to the actor and its
mailbox, so any isolate holding one can push a message and schedule the actor. A
reply subject is instead a one-shot sender that a blocked `call` waits on. Both
are safe to move and clone across threads.

### Cast and call

`subject.send(message)` is a cast: it drops the message in the mailbox and
returns at once.

`subject.call(build, timeout)` is request-reply. It makes a fresh one-shot reply
subject, calls `build(reply)` so the caller can place that reply subject inside
the message, sends the message, and blocks on the reply subject until the handler
answers or the timeout in milliseconds elapses, which raises TimeoutError. The
handler pulls the reply subject out of the message and does `reply.send(result)`
before returning its next state, the same way a Gleam handler replies to the
subject a call passed in.

### Messages

A message is deep copied out of the sender's heap and rebuilt in the receiver's,
except subjects, which pass by handle. The transfer walks a value into a
heap-neutral form the mailbox can carry, and the receiver walks that back into
its own heap. Copyable types are None, bool, int, float, str, list, tuple, and
dict. A subject in the graph is carried as its live sender, not copied. Any other
type, a function, class, instance, module, range, or iterator, raises TypeError
at the send, naming it. The walk keeps an identity map, so a graph with shared
sub-objects or a cycle transfers once and rebuilds with the same sharing.

The neutral form is a flat node array the transfer allocates outside any heap. It
holds the copied scalars and owned strings, and for each subject in the graph a
cloned sender in place of a copy. That array is what the mailbox moves between
threads, wrapped in a send envelope. It carries no heap pointers, only scalars,
owned strings, and senders, so moving it across threads is safe.

### Function transfer

`spawn` serializes the handler's code to the same bytecode format `gecko build`
freezes. By spawn time the handler is already a compiled C code object, so a C
serializer walks that object and emits the format directly, and a round-trip test
holds it to the Rust writer so the two cannot drift. The child reads the bytes
back, wraps the handler in a two-instruction module that makes the function and
returns it, and lowers that into its own code tree with a fresh, empty cache
array, so no code object and no inline cache is shared.

The first cut requires the handler to be a plain top-level function: no free
variables to capture, and no module globals beyond its parameters and the
builtins, since only the handler's own code subtree travels, not the module
around it. Nested functions and classes defined inside it do travel, because they
are child code objects the writer already includes. Closures and module-global
access are a later step that transfers more of the surrounding code.

### Isolates on threads

In v0.0.6 an actor was one OS thread that created its own VM and heap and blocked
on its mailbox. v0.0.7 detaches the isolate from the thread: an actor is a
heap-allocated object that holds its VM, heap, handler, and threaded state, and
it is handed to a worker only while there is a message to process. A worker owns
the actor's run state exclusively for that time, so the heap is still touched by
one thread at a time and needs no locks; between messages the isolate sits idle,
owned by no thread. The loop a worker runs on an actor is: pop a message, rebuild
it in the actor's heap, run `handle(state, message)`, take its return as the next
state, and repeat until the mailbox empties, a stop, or an error.

## Scheduler for v0.0.7

The scheduler is a fixed pool of worker threads, one per core, over a
work-stealing set of deques (crossbeam-deque: a global injector and a per-worker
deque with stealers). It lives once per process, started the first time an actor
is spawned.

An actor's mailbox is a queue plus a `scheduled` flag. The flag is only changed
while holding the mailbox lock, so it serializes cleanly. A `send` locks the
mailbox, pushes the message, and swaps `scheduled` to true; if it was false, the
actor was idle and the sender injects it into the pool and wakes a worker. A
worker that picks up an actor drains its mailbox message by message; when the
mailbox is empty it clears `scheduled` under the lock and stops touching the
actor. Because the flag flips to scheduled exactly once per idle-to-busy edge,
an actor is in the pool at most once and runs on one worker at a time, which is
what makes exclusive access to its VM sound.

An actor stays alive while a subject to it exists or while the pool holds it. A
subject is a reference-counted handle; the actor tracks how many subjects exist,
and when the last one drops it is scheduled one final time so it can drain any
queued messages and then be freed with its isolate. Returning `actor.stop()`, or
a handler that raises, closes the actor: it fails every queued message so waiting
calls do not hang, and rejects later sends.

A `call` still blocks the calling thread on a one-shot reply channel until the
reply or the timeout. When the caller is itself an actor handler, that blocks its
worker for the duration, since the VM cannot suspend mid-handler; a pool sized to
the cores tolerates this, and cooperative suspension is left for later. Bounded
mailbox backpressure, timers for delayed sends, and per-worker fairness tuning
are the open items on top of this first cut.

### Native wiring

`spawn` is a builtin in the `_gecko.actor` submodule, sitting beside
`_gecko.sandbox`; the runtime installs it, since it needs the bytecode reader and
the thread machinery, and `from gecko import actor` binds that submodule. A
subject is a native object in the heap whose `send` and `call` methods push to
and wait on its mailbox. It holds a reference-counted handle that drops when the
subject is collected, so the actor is freed with its isolate once the last
subject to it is gone and its mailbox has drained.

### Errors

A handler that raises stops the actor, and the message that triggered it decides
what the caller sees. A call carries its reply channel alongside the message, so
the failure text travels back through that channel and `call` re-raises it at the
caller as a RuntimeError carrying the handler's error text. A cast has no one
waiting, so the failure just stops the actor. Preserving the original exception
type across the boundary, and supervision with restart and failure trees, are
deferred.

### Portable API

The `gecko` package exposes `actor.spawn` and bridges to the native
`_gecko.actor` when it is present. On CPython, where the native module is absent,
`spawn` raises today; a fallback over processes or threads follows once its
behavior is pinned down, so a program written against the API will run on both.

## Open

- What a message copy costs on a large object graph, and where the immutable
  fast path starts to pay off.
- Backpressure and bounded mailboxes.
- Supervision and failure propagation between actors.
- Transferring closures and the module globals a function reads, so spawn takes
  any function and not only a self-contained one.
