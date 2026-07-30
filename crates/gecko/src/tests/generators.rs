use super::check;

#[test]
fn async_await_composition() {
    let src = r#"
async def add(a, b):
    return a + b
async def compute():
    x = await add(2, 3)
    return await add(x, 10)
def drive(coro):
    try:
        while True:
            coro.send(None)
    except StopIteration:
        pass
drive(compute())
print("ran")
"#;
    let want = r#"
ran
"#;
    check(src, want);
}

#[test]
fn async_cooperative_concurrency() {
    let src = r#"
class Suspend:
    def __await__(self):
        yield
async def worker(name, n):
    i = 0
    while i < n:
        await Suspend()
        print(name, i)
        i = i + 1
def run_all(active):
    while active:
        still = []
        for c in active:
            try:
                c.send(None)
                still.append(c)
            except StopIteration:
                pass
        active = still
run_all([worker("A", 3), worker("B", 2)])
"#;
    let want = r#"
A 0
B 0
A 1
B 1
A 2
"#;
    check(src, want);
}

#[test]
fn coroutine_is_not_an_iterator() {
    let src = r#"
async def c():
    return 1
try:
    next(c())
except TypeError as e:
    print(e)
"#;
    let want = r#"
'coroutine' object is not an iterator
"#;
    check(src, want);
}

#[test]
fn generator_expressions() {
    let src = r#"
print(",".join(str(x) for x in range(4)))
print(sum(x * x for x in range(5)))
print(list(x for x in range(3)))
g = (x + 1 for x in [10, 20])
print(next(g), next(g))
print(sum(i for i in range(10) if i % 2 == 0))
"#;
    let want = r#"
0,1,2,3
30
[0, 1, 2]
11 21
20
"#;
    check(src, want);
}

#[test]
fn generators() {
    let src = r#"
def count(n):
    i = 0
    while i < n:
        yield i
        i = i + 1
print([x for x in count(4)])
g = count(2)
print(next(g))
print(next(g))
try:
    next(g)
except StopIteration:
    print("done")
"#;
    let want = r#"
[0, 1, 2, 3]
0
1
done
"#;
    check(src, want);
}

#[test]
fn dict_items_yields_tuples() {
    let src = r#"
d = {"a": 1, "b": 2}
for k, v in d.items():
    print(k, v)
print(d.items())
"#;
    let want = r#"
a 1
b 2
dict_items([('a', 1), ('b', 2)])
"#;
    check(src, want);
}
