use super::super::run_source;
use super::check;

#[test]
fn lambda_with_closure() {
    let src =
        "def outer():\n    n = 100\n    f = lambda k: k + n\n    return f(5)\nprint(outer())\n";
    assert_eq!(run_source(src).unwrap(), "105\n");
}

#[test]
fn actor_call_replies() {
    let src = r#"
from gecko import actor

def handle(state, message):
    reply = message[1]
    reply.send(state + message[0])
    return state + message[0]

def build(reply):
    return [7, reply]

calc = actor.spawn(0, handle)
print(calc.call(build, 1000))
"#;
    let want = r#"
7
"#;
    check(src, want);
}

#[test]
fn actor_counter_casts_then_calls() {
    let src = r#"
from gecko import actor

def handle(state, message):
    if message[0] == "add":
        return state + message[1]
    message[1].send(state)
    return state

def get(reply):
    return ["get", reply]

counter = actor.spawn(0, handle)
counter.send(["add", 5])
counter.send(["add", 3])
print(counter.call(get, 1000))
"#;
    let want = r#"
8
"#;
    check(src, want);
}

#[test]
fn actor_call_reraises_a_handler_failure() {
    let src = r#"
from gecko import actor

def handle(state, message):
    message[2].send(message[0] / message[1])
    return state

def divide(a, b):
    def build(reply):
        return [a, b, reply]
    return build

calc = actor.spawn(0, handle)
try:
    print(calc.call(divide(10, 0), 1000))
except RuntimeError as e:
    print("caught")
"#;
    let want = r#"
caught
"#;
    check(src, want);
}

#[test]
fn actor_stop_ends_the_actor() {
    let src = r#"
from gecko import actor

def handle(state, message):
    if message[0] == "stop":
        return actor.stop()
    message[1].send(state + 1)
    return state + 1

a = actor.spawn(0, handle)

def ping(reply):
    return ["ping", reply]

print(a.call(ping, 1000))
a.send(["stop", 0])
try:
    a.call(ping, 200)
    print("alive")
except Exception as e:
    print("stopped")
"#;
    let want = r#"
1
stopped
"#;
    check(src, want);
}

#[test]
fn handler_reaches_module_globals() {
    let src = r#"
from gecko import actor

SCALE = 10

def double(x):
    return x * 2

def handle(state, message):
    message[1].send(double(message[0]) + SCALE)
    return state

a = actor.spawn(0, handle)
print(a.call(lambda r: [5, r], 2000))
"#;
    let want = r#"
20
"#;
    check(src, want);
}

#[test]
fn handler_can_spawn_children() {
    let src = r#"
from gecko import actor

def child_h(state, message):
    message[1].send(state + message[0])
    return state

def parent_h(state, message):
    message[1].send(actor.spawn(100, child_h))
    return state

p = actor.spawn(None, parent_h)
c = p.call(lambda r: ["make", r], 2000)
print(c.call(lambda r: [5, r], 2000))
"#;
    let want = r#"
105
"#;
    check(src, want);
}

#[test]
fn monitor_notifies_on_actor_death() {
    let src = r#"
from gecko import actor

def worker(state, message):
    if message[0] == "boom":
        raise ValueError("crash")
    if message[0] == "stop":
        return actor.stop()
    return state

def collector(state, message):
    if message[0] == "down":
        return state + 1
    if message[0] == "get":
        message[1].send(state)
        return state
    return state

col = actor.spawn(0, collector)
w1 = actor.spawn(0, worker)
w1.monitor(col, ["down"])
w1.send(["boom"])
w2 = actor.spawn(0, worker)
w2.monitor(col, ["down"])
w2.send(["stop"])

def fence(reply):
    col.send_after(150, ["get", reply])
    return ["noop"]

print(col.call(fence, 3000))
"#;
    let want = r#"
2
"#;
    check(src, want);
}

#[test]
fn bounded_mailbox_keeps_order_without_loss() {
    let src = r#"
from gecko import actor

def handle(state, message):
    if message[0] == "log":
        return state + message[1]
    message[1].send(state)
    return state

a = actor.spawn("", handle, [], 1)
for ch in ["a", "b", "c", "d", "e"]:
    a.send(["log", ch])
print(a.call(lambda r: ["get", r], 3000))
"#;
    let want = r#"
abcde
"#;
    check(src, want);
}

#[test]
fn send_after_delivers_delayed_messages() {
    let src = r#"
from gecko import actor

def handle(state, message):
    if message[0] == "tick":
        return state + 1
    if message[0] == "report":
        message[1].send(state)
        return state
    return state

a = actor.spawn(0, handle)
for i in range(3):
    a.send_after(5, ["tick"])
print(a.call(lambda r: ["report", r], 2000))

def fence(reply):
    a.send_after(120, ["report", reply])
    return ["noop"]

print(a.call(fence, 3000))
"#;
    let want = r#"
0
3
"#;
    check(src, want);
}

#[test]
fn many_actors_share_the_pool() {
    let src = r#"
from gecko import actor

def handle(state, message):
    message[1].send(state + message[0])
    return state

def ask(n):
    def build(reply):
        return [n, reply]
    return build

actors = [actor.spawn(i, handle) for i in range(64)]
total = 0
for a in actors:
    total += a.call(ask(1), 2000)
print(total)
"#;
    let want = r#"
2080
"#;
    check(src, want);
}

#[test]
fn methods_can_return_closures_over_self() {
    let src = r#"
class Adder:
    def __init__(self, base):
        self.base = base
    def make(self):
        b = self.base
        def add(x):
            return b + x
        return add
f = Adder(10).make()
print(f(5), f(7))
"#;
    let want = r#"
15 17
"#;
    check(src, want);
}

#[test]
fn actors_run_parallel_kernels_concurrently() {
    let src = r#"
from gecko import array, actor

N = 200000

def handle(state, message):
    a = array(range(N), dtype="i64")
    b = a + a
    message[1].send(b.sum())
    return state

workers = []
for i in range(8):
    workers.append(actor.spawn(0, handle))

expect = 0
for i in range(N):
    expect += i
expect = expect * 2
bad = 0
for w in workers:
    if w.call(lambda r: [0, r], 30000) != expect:
        bad += 1
print(expect, bad)
"#;
    let want = r#"
39999800000 0
"#;
    check(src, want);
}

#[test]
fn big_integers_cross_actors() {
    let src = r#"
from gecko import actor

def handle(state, message):
    message[1].send(message[0] * 3)
    return state

a = actor.spawn(0, handle)
print(a.call(lambda r: [10 ** 25, r], 2000))
print(a.call(lambda r: [-(2 ** 70), r], 2000))
print(a.call(lambda r: [7, r], 2000))
"#;
    let want = r#"
30000000000000000000000000
-3541774862152233910272
21
"#;
    check(src, want);
}

#[test]
fn supervise_restarts_a_failing_child() {
    let src = r#"
from gecko import actor

def handle(state, message):
    if message[0] == 'boom':
        raise ValueError('crash')
    message[1].send(state + message[0])
    return state + message[0]

sup = actor.supervise(0, handle, None, 2, 60000)
print(sup.call(lambda r: [5, r], 2000))
print(sup.call(lambda r: [7, r], 2000))
try:
    sup.call(lambda r: ['boom', r], 2000)
except RuntimeError:
    print('crashed')
print(sup.call(lambda r: [1, r], 2000))
try:
    sup.call(lambda r: ['boom', r], 2000)
except RuntimeError:
    print('crashed')
print(sup.call(lambda r: [1, r], 2000))
try:
    sup.call(lambda r: ['boom', r], 2000)
except RuntimeError:
    print('crashed')
try:
    sup.call(lambda r: [1, r], 2000)
    print('alive')
except RuntimeError:
    print('exhausted')
"#;
    let want = r#"
5
12
crashed
1
crashed
1
crashed
exhausted
"#;
    check(src, want);
}

#[test]
fn supervise_keeps_closure_captures_across_restarts() {
    let src = r#"
from gecko import actor

def make(base):
    def handle(state, message):
        if message[0] < 0:
            raise ValueError('bad')
        message[1].send(base + message[0])
        return state
    return handle

sup = actor.supervise(0, make(100))
print(sup.call(lambda r: [5, r], 2000))
try:
    sup.call(lambda r: [-1, r], 2000)
except RuntimeError:
    print('crashed')
print(sup.call(lambda r: [7, r], 2000))
"#;
    let want = r#"
105
crashed
107
"#;
    check(src, want);
}

#[test]
fn spawn_transfers_closures() {
    let src = r#"
from gecko import actor

def make_adder(base, label):
    def handle(state, message):
        message[1].send(label + str(base + message[0]))
        return state
    return handle

a = actor.spawn(0, make_adder(100, "sum="))
b = actor.spawn(0, make_adder(1000, "big="))
print(a.call(lambda r: [5, r], 2000))
print(b.call(lambda r: [1, r], 2000))
print(a.call(lambda r: [2, r], 2000))

def uses(d):
    def handle(state, message):
        message[0].send(len(d))
        return state
    return handle

data = [1, 2]
h = actor.spawn(0, uses(data))
print(h.call(lambda r: [r], 2000))
data.append(3)
print(h.call(lambda r: [r], 2000), len(data))

def bad():
    helper = lambda x: x
    def handle(state, message):
        message[0].send(helper(1))
        return state
    return handle

try:
    actor.spawn(0, bad())
    print('spawned')
except TypeError:
    print('TypeError')
"#;
    let want = r#"
sum=105
big=1001
sum=102
2
2 3
TypeError
"#;
    check(src, want);
}

#[test]
fn closures_capture_and_update() {
    let src = r#"
def counter():
    n = 0
    def inc():
        nonlocal n
        n += 1
        return n
    return inc
c = counter()
d = counter()
print(c(), c(), d(), c())
"#;
    let want = r#"
1 2 1 3
"#;
    check(src, want);
}

#[test]
fn closures_share_one_cell() {
    let src = r#"
def pair():
    v = 0
    def set5():
        nonlocal v
        v = 5
    def get():
        return v
    set5()
    return get()
print(pair())
"#;
    let want = r#"
5
"#;
    check(src, want);
}

#[test]
fn closures_capture_transitively() {
    let src = r#"
def a():
    x = 7
    def b():
        def inner():
            return x
        return inner()
    return b()
print(a())
"#;
    let want = r#"
7
"#;
    check(src, want);
}

#[test]
fn loop_closures_share_the_variable() {
    let src = r#"
def late():
    fs = []
    for i in range(3):
        def f():
            return i
        fs.append(f)
    return fs
fs = late()
print(fs[0](), fs[1](), fs[2]())
"#;
    let want = r#"
2 2 2
"#;
    check(src, want);
}
