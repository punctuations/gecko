use super::check;

#[test]
fn typed_arrays() {
    let src = r#"
from gecko import array
xs = array([1.0, 2.0, 3.0, 4.0], dtype="f64")
print(xs)
print(len(xs), xs[1], xs[-1])
ys = array([10, 20, 30, 40, 50], dtype="i64")
print(ys[1:3], ys[::2])
print(array([1, 2, 3], dtype="i32"))
print(array(range(4), dtype="f32"))
print(bool(array([], dtype="f64")), bool(xs))
print(array([10 ** 18], dtype="i64")[0])
"#;
    let want = r#"
array([1.0, 2.0, 3.0, 4.0], dtype='f64')
4 2.0 4.0
array([20, 30], dtype='i64') array([10, 30, 50], dtype='i64')
array([1, 2, 3], dtype='i32')
array([0.0, 1.0, 2.0, 3.0], dtype='f32')
False True
1000000000000000000
"#;
    check(src, want);
}

#[test]
fn typed_array_view_survives_collection() {
    let src = r#"
from gecko import array

def make_view():
    parent = array([1.0, 2.0, 3.0, 4.0, 5.0], dtype="f64")
    return parent[1:4]

v = make_view()
for _ in range(50000):
    array([0.0, 0.0, 0.0], dtype="f64")
print(v, v[0], v[2])
try:
    array([1.5], dtype="i64")
except TypeError:
    print("typeerror")
try:
    array([1], dtype="q8")
except ValueError:
    print("valueerror")
"#;
    let want = r#"
array([2.0, 3.0, 4.0], dtype='f64') 2.0 4.0
typeerror
valueerror
"#;
    check(src, want);
}

#[test]
fn typed_array_arithmetic() {
    let src = r#"
from gecko import array
xs = array([1.0, 2.0, 3.0, 4.0], dtype="f64")
print(xs * 2.0 + 1.0)
print(xs.sum(), xs.prod(), xs.min(), xs.max())
a = array([1, 2, 3, 4], dtype="i64")
b = array([10, 20, 30, 40], dtype="i64")
print(a + b, a * b, b - a)
print(a.sum(), a.prod(), a.min(), a.max())
print(a / b)
print(a.tolist())
try:
    a + array([1, 2], dtype="i64")
except ValueError:
    print("shape")
"#;
    let want = r#"
array([3.0, 5.0, 7.0, 9.0], dtype='f64')
10.0 24.0 1.0 4.0
array([11, 22, 33, 44], dtype='i64') array([10, 40, 90, 160], dtype='i64') array([9, 18, 27, 36], dtype='i64')
10 24 1 4
array([0.1, 0.1, 0.1, 0.1], dtype='f64')
[1, 2, 3, 4]
shape
"#;
    check(src, want);
}

#[test]
fn typed_arrays_transfer_by_handle() {
    let src = r#"
from gecko import actor, array

def handle(state, message):
    message[1].send(message[0].sum())
    return state

def echo(state, message):
    message[1].send(message[0])
    return state

a = actor.spawn(0, handle)
data = array([1.0, 2.0, 3.0, 4.0], dtype="f64")
print(a.call(lambda r: [data, r], 2000))
print(data)
e = actor.spawn(0, echo)
back = e.call(lambda r: [data[1:3], r], 2000)
print(back, back.sum())
big = e.call(lambda r: [array(range(100000), dtype="f64"), r], 5000)
print(len(big), big[99999])
"#;
    let want = r#"
10.0
array([1.0, 2.0, 3.0, 4.0], dtype='f64')
array([2.0, 3.0], dtype='f64') 5.0
100000 99999.0
"#;
    check(src, want);
}

#[test]
fn typed_array_map_filter_reduce() {
    let src = r#"
from gecko import array
xs = array([1.0, 2.0, 3.0, 4.0, 5.0], dtype="f64")
print(xs.map(lambda x: x * x))
print(xs.filter(lambda x: x > 2.0))
print(xs.reduce(lambda a, b: a + b), xs.reduce(lambda a, b: a + b, 100.0))
a = array([1, 2, 3, 4, 5, 6], dtype="i64")
print(a.map(lambda x: x * 2))
print(a.filter(lambda x: x % 2 == 0))
print(a.reduce(lambda p, q: p * q))
print(a.map(lambda x: x / 2))
v = a[1:4]
print(v.map(lambda x: x + 10), v.filter(lambda x: x > 2))
big = array([10 ** 18, 2 * 10 ** 18, 3], dtype="i64")
print(big.reduce(lambda p, q: p + q))
print(array([], dtype="f64").filter(lambda x: True))
try:
    array([], dtype="i64").reduce(lambda p, q: p + q)
except TypeError:
    print("empty")
"#;
    let want = r#"
array([1.0, 4.0, 9.0, 16.0, 25.0], dtype='f64')
array([3.0, 4.0, 5.0], dtype='f64')
15.0 115.0
array([2, 4, 6, 8, 10, 12], dtype='i64')
array([2, 4, 6], dtype='i64')
720
array([0.5, 1.0, 1.5, 2.0, 2.5, 3.0], dtype='f64')
array([12, 13, 14], dtype='i64') array([3, 4], dtype='i64')
3000000000000000003
array([], dtype='f64')
empty
"#;
    check(src, want);
}

#[test]
fn typed_array_kernels_run_in_parallel() {
    let src = r#"
from gecko import array
n = 300000
xs = array(range(n), dtype="i64")
z = xs + xs
print(len(z), z[0], z[1], z[n - 1])
print(z.sum(), z.min(), z.max())
f = array(range(n), dtype="f64")
g = f * 2.0 + 1.0
print(g[0], g[n - 1], g.min(), g.max(), g.sum())
"#;
    let want = r#"
300000 0 2 599998
89999700000 0 599998
1.0 599999.0 1.0 599999.0 90000000000.0
"#;
    check(src, want);
}
