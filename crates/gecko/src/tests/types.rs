use super::super::run_source;
use super::check;

#[test]
fn arithmetic() {
    check("print(1 + 2 * 3)\n", "7\n");
}

#[test]
fn unary_negation() {
    check("print(-3 + 1)\n", "-2\n");
}

#[test]
fn list_subscript_assignment() {
    let src = r#"
l = [1, 2, 3]
l[1] = 20
print(l)
"#;
    let want = r#"
[1, 20, 3]
"#;
    check(src, want);
}

#[test]
fn for_over_range_list_str_dict() {
    let src = r#"
total = 0
for i in range(5):
    total += i
for x in [10, 20]:
    total += x
for c in "ab":
    print(c)
d = {"k": 1}
for k in d:
    print(k)
print(total)
"#;
    let want = r#"
a
b
k
40
"#;
    check(src, want);
}

#[test]
fn membership_tests() {
    check(
        "print(2 in [1, 2], 5 in [1, 2], \"a\" in {\"a\": 1}, \"ell\" in \"hello\", 4 in range(0, 10, 2))\n",
        "True False True True True\n",
    );
}

#[test]
fn mod_and_floordiv_follow_python() {
    check(
        "print(17 % 5, -17 % 5, 17 // 5, -17 // 5, 7.5 % 2, -7.5 // 2)\n",
        "2 3 3 -4 1.5 -4.0\n",
    );
}

#[test]
fn str_concat_and_ordering() {
    check(
        "print(\"con\" + \"cat\", \"a\" < \"b\", \"abc\" <= \"ab\")\n",
        "concat True False\n",
    );
}

#[test]
fn deep_equality() {
    check(
        "print([1, {\"a\": 2}] == [1, {\"a\": 2}], [1] == [2])\n",
        "True False\n",
    );
}

#[test]
fn large_dict_lookups_stay_correct() {
    let src = r#"
d = {}
for i in range(50):
    d[i] = i * i
print(d[0], d[25], d[49], len(d))
print(25 in d, 999 in d)
"#;
    let want = r#"
0 625 2401 50
True False
"#;
    check(src, want);
}

#[test]
fn dict_int_and_float_keys_collide_like_python() {
    let src = r#"
d = {}
for i in range(12):
    d[i] = i
print(d[5.0])
d[5.0] = 100
print(d[5], len(d))
"#;
    let want = r#"
5
100 12
"#;
    check(src, want);
}

#[test]
fn large_dict_preserves_insertion_order() {
    let src = r#"
d = {}
for i in range(15):
    d[i * 3] = i
out = []
for k in d:
    out.append(k)
print(out[0], out[7], out[14])
"#;
    let want = r#"
0 21 42
"#;
    check(src, want);
}

#[test]
fn tuple_keys_work_through_the_index() {
    let src = r#"
d = {}
for i in range(10):
    d[(i, i + 1)] = i
print(d[(3, 4)], (7, 8) in d, (7, 9) in d)
"#;
    let want = r#"
3 True False
"#;
    check(src, want);
}

#[test]
fn string_constants_fold() {
    check("print(\"a\" + \"b\" + \"c\")\n", "abc\n");
}

#[test]
fn tuple_of_types_matches_any() {
    let src = r#"
try:
    raise RuntimeError("boom")
except (ValueError, RuntimeError) as e:
    print(e)
"#;
    let want = r#"
boom
"#;
    check(src, want);
}

#[test]
fn dict_variants() {
    let src = r#"
print(dict([("a", 1), ("b", 2)]))
print(dict(x=1, y=2))
d = {"a": 1}
print(dict(d, b=2))
"#;
    let want = r#"
{'a': 1, 'b': 2}
{'x': 1, 'y': 2}
{'a': 1, 'b': 2}
"#;
    check(src, want);
}

#[test]
fn sets() {
    let src = r#"
print(sorted({1, 2, 2, 3}))
print(len({1, 2, 3}))
print(2 in {1, 2, 3})
print(sorted({x * 2 for x in range(4)}))
s = {1, 2}
s.add(3)
s.add(1)
s.discard(2)
print(sorted(s))
print(set())
print(sorted(set([1, 1, 2, 3, 3])))
"#;
    let want = r#"
[1, 2, 3]
3
True
[0, 2, 4, 6]
[1, 3]
set()
[1, 2, 3]
"#;
    check(src, want);
}

#[test]
fn slicing() {
    let src = r#"
s = "hello world"
print(s[1:3], s[:5], s[6:], s[::-1], s[::2])
l = [0, 1, 2, 3, 4, 5]
print(l[2:5], l[::-1], l[1:5:2], l[-3:])
t = (1, 2, 3, 4)
print(t[1:3], t[::-1])
print(l[100:], l[3:3])
"#;
    let want = r#"
el hello world dlrow olleh hlowrd
[2, 3, 4] [5, 4, 3, 2, 1, 0] [1, 3] [3, 4, 5]
(2, 3) (4, 3, 2, 1)
[] []
"#;
    check(src, want);
}

#[test]
fn dict_keys_use_hash_and_eq() {
    let src = r#"
class V:
    def __init__(self, k): self.k = k
    def __eq__(self, o): return self.k == o.k
    def __hash__(self): return self.k % 7

d = {}
for i in range(200):
    d[V(i % 5)] = 1
    d['s' + str(i % 3)] = 2
print(len(d), sorted(d.values()))
d2 = {}
d2[V(1)] = 'a'
d2[V(1)] = 'b'
print(len(d2), list(d2.values()))
print(V(1) in d2, V(9) in d2)
print(len({V(1), V(1), V(3)}))

class E:
    def __init__(self, v): self.v = v
    def __eq__(self, o): return True
try:
    {E(1): 1}
    print('hashable')
except TypeError:
    print('unhashable')
"#;
    let want = r#"
8 [1, 1, 1, 1, 1, 2, 2, 2]
1 ['b']
True False
2
unhashable
"#;
    check(src, want);
}

#[test]
fn sequence_repetition() {
    let src = r#"
print('ab' * 3, 3 * 'ab', 'x' * 0, 'x' * -1)
print([1, 2] * 2, 2 * [1], [] * 5, [0] * 0)
print((0,) * 3, 3 * (1, 2), () * 4)
print(len([0] * 1000), ('ab' * 100)[:6])
print(True * 3, 3 * True, False * 2)
print([[0] * 2] * 2)
"#;
    let want = r#"
ababab ababab  
[1, 2, 1, 2] [1, 1] [] []
(0, 0, 0) (1, 2, 1, 2, 1, 2) ()
1000 ababab
3 3 0
[[0, 0], [0, 0]]
"#;
    check(src, want);
}

#[test]
fn big_integers() {
    let src = r#"
print(2 ** 100)
def fact(n):
    r = 1
    for i in range(1, n + 1):
        r *= i
    return r
print(fact(25))
print(10 ** 30 + 1)
print(2 ** 100 // 7, 2 ** 100 % 7)
print(-(2 ** 70))
print(2 ** 100 == 2 ** 100, 2 ** 100 > 2 ** 99)
x = 123456789012345678901234567890
print(x + x)
print(x * 1000000)
print(divmod(x, 7))
print(abs(-x), x > 0)
print(1000000 * 1000000)
print(type(2 ** 100) is int, isinstance(2 ** 100, int))
"#;
    let want = r#"
1267650600228229401496703205376
15511210043330985984000000
1000000000000000000000000000001
181092942889747057356671886482 2
-1180591620717411303424
True True
246913578024691357802469135780
123456789012345678901234567890000000
(17636684144620811271604938270, 0)
123456789012345678901234567890 True
1000000000000
True True
"#;
    check(src, want);
}

#[test]
fn unary_invert() {
    let src = r#"
print(~5, ~0, ~-1)
x = 12
print(~x & 255)
"#;
    let want = r#"
-6 -1 0
243
"#;
    check(src, want);
}

#[test]
fn numeric_operators() {
    let src = r#"
print(2 ** 10, 2 ** -1, 2.0 ** 3)
print(6 & 3, 6 | 1, 6 ^ 3, 1 << 4, 255 >> 2)
print(True | False, True & True, True ^ True)
x = 5
x **= 2
x |= 2
print(x)
"#;
    let want = r#"
1024 0.5 8.0
2 7 5 16 63
True True False
27
"#;
    check(src, want);
}

#[test]
fn set_operations() {
    let src = r#"
a = {1, 2, 3, 4}
b = {3, 4, 5, 6}
print(sorted(a | b))
print(sorted(a & b))
print(sorted(a - b))
print(sorted(a ^ b))
print({1, 2} <= a, a <= {1, 2}, a >= {1, 2}, {1, 2} < a)
print({1, 2}.issubset(a), a.isdisjoint({9, 10}))
print(sorted(a.union([7]).intersection({2, 7})))
c = {1, 2}
c.update([3], {4})
print(sorted(c))
"#;
    let want = r#"
[1, 2, 3, 4, 5, 6]
[3, 4]
[1, 2]
[1, 2, 5, 6]
True False True True
True True
[2, 7]
[1, 2, 3, 4]
"#;
    check(src, want);
}

#[test]
fn frozensets() {
    let src = r#"
f = frozenset([1, 2, 3, 2])
print(sorted(f), len(f), 2 in f)
print(f == {1, 2, 3})
d = {f: "y", frozenset([9]): "n"}
print(d[frozenset([1, 2, 3])])
print(len({frozenset([1]), frozenset([1]), frozenset([2])}))
print(type(f | {4}) is frozenset, type({4} | f) is set)
try:
    f.add(9)
except AttributeError:
    print("immutable")
"#;
    let want = r#"
[1, 2, 3] 3 True
True
y
2
True True
immutable
"#;
    check(src, want);
}

#[test]
fn dict_fromkeys() {
    let src = r#"
print(dict.fromkeys([1, 2, 3]))
print(dict.fromkeys("ab", 0))
print(dict.fromkeys(range(2), []))
"#;
    let want = r#"
{1: None, 2: None, 3: None}
{'a': 0, 'b': 0}
{0: [], 1: []}
"#;
    check(src, want);
}

#[test]
fn set_order_matches_cpython() {
    let src = r#"
print({7, 15, 23, 31, 39})
print({100, 1, 50, 8, 200, 7})
s = set()
for x in [7, 15, 23, 31, 39]:
    s.add(x)
print(s)
print(set([100, 1, 50, 8, 200, 7]))
print({3, 4, 5, 6})
"#;
    let want = r#"
{7, 23, 39, 31, 15}
{1, 50, 100, 7, 8, 200}
{39, 7, 15, 23, 31}
{1, 100, 7, 8, 200, 50}
{3, 4, 5, 6}
"#;
    check(src, want);
}

#[test]
fn tuples_pack_unpack_and_compare() {
    let src = r#"
t = (1, "two")
a, b = t
b, a = a, b
x, (y, z) = 1, (2, 3)
print(t, a, b, x, y, z)
print(t == (1, "two"), (1,) + (2, 3), len(()), 2 in (1, 2))
"#;
    let want = r#"
(1, 'two') two 1 1 2 3
True (1, 2, 3) 0 True
"#;
    check(src, want);
}

#[test]
fn comprehensions_build_lists_and_dicts() {
    let src = r#"
print([x * x for x in range(5)])
print([x for x in range(10) if x % 2 == 0 if x > 3])
print([(a, b) for a in range(2) for b in "xy"])
print({w: len(w) for w in ["hi", "there"]})
"#;
    let want = r#"
[0, 1, 4, 9, 16]
[4, 6, 8]
[(0, 'x'), (0, 'y'), (1, 'x'), (1, 'y')]
{'hi': 2, 'there': 5}
"#;
    check(src, want);
}

#[test]
fn unicode_strings_index_by_code_point() {
    let src = r#"
s = "héllo"
print(len(s), s[1], s[-1])
for c in "éü":
    print(c)
"#;
    let want = r#"
5 é o
é
ü
"#;
    check(src, want);
}

#[test]
fn container_edge_cases() {
    let src = r#"
l = [1, 2, 3]
print(l.pop(0), l)
d = {}
print(d.get("x"))
r = range(10, 0, -2)
print(len(r), r[0], r[4], 8 in r, 7 in r)
"#;
    let want = r#"
1 [2, 3]
None
5 10 2 True False
"#;
    check(src, want);
}

#[test]
fn reading_an_unset_cell_fails() {
    let src = "def outer():\n    def get():\n        return v\n    r = get()\n    v = 1\n    return r\nouter()\n";
    let f = run_source(src).unwrap_err();
    assert!(f.message.contains("UnboundLocalError"));
}

#[test]
fn dict_views_are_live() {
    let src = r#"
d = {"a": 1, "b": 2}
k = d.keys()
print(k, d.values(), d.items())
print(list(k), len(k), "a" in k, "z" in k)
d["c"] = 3
print(list(k), len(k))
print(1 in d.values(), 9 in d.values())
print(("a", 1) in d.items())
print(d.keys() == d.keys(), d.keys() == {"a", "b", "c"})
print(d.values() == d.values())
print(bool(d.keys()), bool({}.keys()))
try:
    k[0]
except TypeError:
    print("not subscriptable")
try:
    hash(k)
except TypeError:
    print("unhashable")
"#;
    let want = r#"
dict_keys(['a', 'b']) dict_values([1, 2]) dict_items([('a', 1), ('b', 2)])
['a', 'b'] 2 True False
['a', 'b', 'c'] 3
True False
True
True True
False
True False
not subscriptable
unhashable
"#;
    check(src, want);
}
