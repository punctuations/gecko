use super::super::run_source;
use super::check;

#[test]
fn del_attribute() {
    let src = r#"
class P:
    def __init__(self):
        self.v = 1
p = P()
del p.v
try:
    print(p.v)
except AttributeError:
    print("gone")
"#;
    let want = r#"
gone
"#;
    check(src, want);
}

#[test]
fn list_literals_and_methods() {
    let src = r#"
l = [1, 2]
l.append(3)
print(l, len(l), l[0], l[-1])
print(l.pop(), l)
"#;
    let want = r#"
[1, 2, 3] 3 1 3
3 [1, 2]
"#;
    check(src, want);
}

#[test]
fn dict_literals_and_methods() {
    let src = r#"
d = {"a": 1}
d["b"] = 2
print(d, len(d), d["b"], d.get("z", 9))
print(d.keys(), d.values())
"#;
    let want = r#"
{'a': 1, 'b': 2} 2 2 9
['a', 'b'] [1, 2]
"#;
    check(src, want);
}

#[test]
fn polymorphic_method_dispatch_picks_the_right_override() {
    let src = r#"
class Animal:
    def speak(self):
        return "..."
class Dog(Animal):
    def speak(self):
        return "woof"
class Cat(Animal):
    def speak(self):
        return "meow"
def go(a):
    return a.speak()
d = Dog()
c = Cat()
print(go(d), go(c), go(d), go(c))
"#;
    let want = r#"
woof meow woof meow
"#;
    check(src, want);
}

#[test]
fn reassigning_a_method_busts_the_cache() {
    let src = r#"
class C:
    def f(self):
        return 1
def g(self):
    return 2
c = C()
print(c.f())
C.f = g
print(c.f())
"#;
    let want = r#"
1
2
"#;
    check(src, want);
}

#[test]
fn a_data_attribute_shadows_a_method_at_a_call_site() {
    let src = r#"
class C:
    def f(self):
        return 1
def plain(v):
    return v + 100
c = C()
print(c.f())
c.f = plain
print(c.f(5))
"#;
    let want = r#"
1
105
"#;
    check(src, want);
}

#[test]
fn inherited_methods_resolve_through_the_cache() {
    let src = r#"
class Base:
    def m(self):
        return 42
class Mid(Base):
    pass
class Leaf(Mid):
    pass
print(Leaf().m(), Leaf().m())
"#;
    let want = r#"
42 42
"#;
    check(src, want);
}

#[test]
fn polymorphic_attribute_site_reads_the_right_slot() {
    let src = r#"
class A:
    def __init__(self):
        self.x = 1
        self.y = 2
class B:
    def __init__(self):
        self.y = 10
        self.x = 20
def getx(o):
    return o.x
a = A()
b = B()
print(getx(a), getx(b), getx(a), getx(b))
"#;
    let want = r#"
1 20 1 20
"#;
    check(src, want);
}

#[test]
fn adding_an_attribute_busts_a_cached_site() {
    let src = r#"
class G:
    pass
def read(o):
    return o.a
g = G()
g.a = 1
print(read(g))
g.b = 2
print(read(g), g.b)
"#;
    let want = r#"
1
1 2
"#;
    check(src, want);
}

#[test]
fn instance_attributes_get_and_set() {
    let src = r#"
class P:
    def __init__(self, x, y):
        self.x = x
        self.y = y
p = P(3, 4)
print(p.x, p.y)
p.x = 100
print(p.x, p.y)
p.z = 9
print(p.z)
"#;
    let want = r#"
3 4
100 4
9
"#;
    check(src, want);
}

#[test]
fn instances_with_different_attribute_orders_stay_correct() {
    let src = r#"
class Bag:
    pass
a = Bag()
a.p = 1
a.q = 2
b = Bag()
b.q = 20
b.p = 10
print(a.p, a.q, b.p, b.q)
"#;
    let want = r#"
1 2 10 20
"#;
    check(src, want);
}

#[test]
fn instance_slots_survive_gc() {
    let src = r#"
class N:
    def __init__(self, v):
        self.v = v
        self.next = None
head = None
for i in range(20000):
    n = N(i)
    n.next = head
    head = n
s = 0
cur = head
while cur is not None:
    s = s + cur.v
    cur = cur.next
print(s)
"#;
    let want = r#"
199990000
"#;
    check(src, want);
}

#[test]
fn subclass_init_sets_attributes() {
    let src = r#"
class A:
    def __init__(self, name):
        self.name = name
class B(A):
    def greet(self):
        return self.name
b = B("x")
print(b.name, b.greet())
"#;
    let want = r#"
x x
"#;
    check(src, want);
}

#[test]
fn class_with_many_methods_resolves() {
    let mut src = String::from("class C:\n");
    for i in 0..12 {
        src.push_str(&format!("    def m{i}(self):\n        return {i}\n"));
    }
    src.push_str("c = C()\nprint(c.m0(), c.m6(), c.m11())\n");
    assert_eq!(run_source(&src).unwrap(), "0 6 11\n");
}

#[test]
fn a_subclassed_exception_is_catchable() {
    let src = r#"
class MyError(Exception):
    pass
try:
    raise MyError("boom")
except MyError:
    print("caught")
"#;
    let want = r#"
caught
"#;
    check(src, want);
}

#[test]
fn decorator_wraps_a_function() {
    let src = r#"
def twice(f):
    def w(x):
        return f(f(x))
    return w
@twice
def inc(n):
    return n + 1
print(inc(10))
"#;
    let want = r#"
12
"#;
    check(src, want);
}

#[test]
fn decorator_with_arguments() {
    let src = r#"
def tag(label):
    def deco(f):
        def w(x):
            return label + ":" + f(x)
        return w
    return deco
@tag("r")
def shout(s):
    return s
print(shout("hi"))
"#;
    let want = r#"
r:hi
"#;
    check(src, want);
}

#[test]
fn stacked_decorators_apply_bottom_up() {
    let src = r#"
def a(f):
    def w(x):
        return "a(" + f(x) + ")"
    return w
def b(f):
    def w(x):
        return "b(" + f(x) + ")"
    return w
@a
@b
def base(x):
    return x
print(base("X"))
"#;
    let want = r#"
a(b(X))
"#;
    check(src, want);
}

#[test]
fn class_decorator_runs() {
    let src = r#"
seen = []
def register(cls):
    seen.append(cls)
    return cls
@register
class W:
    def __init__(self):
        self.name = "w"
print(W().name, len(seen))
"#;
    let want = r#"
w 1
"#;
    check(src, want);
}

#[test]
fn classes_init_attributes_and_methods() {
    let src = r#"
class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y
    def norm2(self):
        return self.x * self.x + self.y * self.y
p = Point(3, 4)
print(p.x, p.y, p.norm2())
"#;
    let want = r#"
3 4 25
"#;
    check(src, want);
}

#[test]
fn classes_inherit_and_override() {
    let src = r#"
class Animal:
    def __init__(self, name):
        self.name = name
    def speak(self):
        return "..."
    def describe(self):
        return self.name + ": " + self.speak()
class Dog(Animal):
    def speak(self):
        return "woof"
print(Dog("Rex").describe())
print(Animal("Thing").describe())
"#;
    let want = r#"
Rex: woof
Thing: ...
"#;
    check(src, want);
}

#[test]
fn class_attributes_and_instance_shadowing() {
    let src = r#"
class Box:
    kind = "box"
    def set(self, v):
        self.kind = v
b = Box()
print(b.kind, Box.kind)
b.set("crate")
print(b.kind, Box.kind)
"#;
    let want = r#"
box box
crate box
"#;
    check(src, want);
}

#[test]
fn class_body_names_are_invisible_to_methods() {
    let src = r#"
FACTOR = 100
class W:
    FACTOR = 3
    def get(self):
        return FACTOR
print(W().get(), W.FACTOR)
"#;
    let want = r#"
100 3
"#;
    check(src, want);
}

#[test]
fn bound_methods_are_values() {
    let src = r#"
class Counter:
    def __init__(self):
        self.n = 0
    def inc(self):
        self.n += 1
        return self.n
c = Counter()
m = c.inc
print(m(), m(), c.n)
"#;
    let want = r#"
1 2 2
"#;
    check(src, want);
}

#[test]
fn missing_attribute_raises() {
    let src = "class E:\n    pass\nE().nope\n";
    let f = run_source(src).unwrap_err();
    assert!(f.message.contains("AttributeError"), "{}", f.message);
    assert!(f.message.contains("nope"), "{}", f.message);
}

#[test]
fn instances_survive_collection() {
    let src = r#"
class Node:
    def __init__(self, v):
        self.v = v
keep = Node(7)
i = 0
while i < 20000:
    tmp = Node(i)
    i += 1
print(keep.v)
"#;
    let want = r#"
7
"#;
    check(src, want);
}

#[test]
fn type_repr_is_class() {
    let src = r#"
print(type(5))
print(type("x"))
print(int)
print(list)
print(set)
"#;
    let want = r#"
<class 'int'>
<class 'str'>
<class 'int'>
<class 'list'>
<class 'set'>
"#;
    check(src, want);
}

#[test]
fn string_methods() {
    let src = r#"
print("a,b,c".split(","))
print("a b  c".split())
print("-".join(["a", "b"]))
print("  hi  ".strip(), "xxhi".strip("x"))
print("Hi".upper(), "Hi".lower(), "hi there".title())
print("hello".replace("l", "L"))
print("hello".startswith("he"), "hello".endswith("lo"))
print("hello".find("l"), "hello".count("l"))
print("5".zfill(3), "hi".center(6) + "|")
print("abc".isalpha(), "123".isdigit())
"#;
    let want = r#"
['a', 'b', 'c']
['a', 'b', 'c']
a-b
hi hi
HI hi Hi There
heLLo
True True
2 2
005   hi  |
True True
"#;
    check(src, want);
}

#[test]
fn class_descriptors() {
    let src = r#"
class Temp:
    def __init__(self, c):
        self._c = c

    @property
    def celsius(self):
        return self._c

    @celsius.setter
    def celsius(self, v):
        if v < -273:
            raise ValueError("too cold")
        self._c = v

    @property
    def fahrenheit(self):
        return self._c * 9 / 5 + 32

    @staticmethod
    def freezing():
        return 0

    @classmethod
    def boiling(cls):
        return cls(100)

t = Temp(25)
print(t.celsius, t.fahrenheit)
t.celsius = 30
print(t.celsius, t.fahrenheit)
try:
    t.celsius = -300
except ValueError:
    print("rejected")
print(t.celsius)
try:
    t.fahrenheit = 5
except AttributeError:
    print("read-only")
print(Temp.freezing(), t.freezing())
b = Temp.boiling()
print(b.celsius, type(b) is Temp)
b2 = t.boiling()
print(b2.celsius)
"#;
    let want = r#"
25 77.0
30 86.0
rejected
30
read-only
0 0
100 True
100
"#;
    check(src, want);
}

#[test]
fn super_calls() {
    let src = r#"
class Animal:
    def __init__(self, name):
        self.name = name
    def speak(self):
        return "..."
    def describe(self):
        return self.name + " says " + self.speak()

class Dog(Animal):
    def __init__(self, name, breed):
        super().__init__(name)
        self.breed = breed
    def speak(self):
        return "woof"
    def describe(self):
        return super().describe() + " (" + self.breed + ")"

class Puppy(Dog):
    def speak(self):
        return super().speak() + "!"

d = Dog("Rex", "lab")
print(d.name, d.breed, d.speak())
print(d.describe())
p = Puppy("Bit", "corgi")
print(p.speak(), p.describe())
print(super(Dog, d).speak())
print(isinstance(d, Dog), isinstance(d, Animal))
"#;
    let want = r#"
Rex lab woof
Rex says woof (lab)
woof! Bit says woof! (corgi)
...
True True
"#;
    check(src, want);
}

#[test]
fn special_methods() {
    let src = r#"
class V:
    def __init__(self, n): self.n = n
    def __repr__(self): return "V(" + str(self.n) + ")"
    def __str__(self): return "v" + str(self.n)
    def __add__(self, o): return V(self.n + o.n)
    def __radd__(self, o): return V(o + self.n)
    def __mul__(self, o): return V(self.n * o)
    def __neg__(self): return V(-self.n)
    def __abs__(self): return V(abs(self.n))
    def __len__(self): return self.n
    def __bool__(self): return self.n != 0
    def __eq__(self, o): return self.n == o.n
    def __lt__(self, o): return self.n < o.n
    def __le__(self, o): return self.n <= o.n
    def __gt__(self, o): return self.n > o.n
    def __ge__(self, o): return self.n >= o.n
    def __hash__(self): return self.n * 7
    def __call__(self, k): return self.n + k
    def __contains__(self, x): return x == self.n

class Box:
    def __init__(self): self.d = {}
    def __getitem__(self, k): return self.d[k]
    def __setitem__(self, k, v): self.d[k] = v
    def __len__(self): return len(self.d)

class Count:
    def __init__(self, n): self.n = n
    def __iter__(self): return iter([i * i for i in range(self.n)])

a, b = V(3), V(4)
print(a, repr(a), [a], str(a))
print(a + b, 10 + a, a * 5, -a, abs(V(-9)))
print(len(a), bool(a), bool(V(0)))
print(a == V(3), a != V(3), a < b, a > b, a <= V(3), a >= b)
print(hash(a), a(10), 3 in a, 8 in a)
box = Box()
box["k"] = 1
box["j"] = 2
print(box["k"], box["j"], len(box))
print([x for x in Count(5)])
for x in Count(3):
    print(x)
print(sorted([V(5), V(1), V(3)]))
"#;
    let want = r#"
v3 V(3) [V(3)] v3
v7 v13 v15 v-3 v9
3 True False
True False True False True False
21 13 True False
1 2 2
[0, 1, 4, 9, 16]
0
1
4
[V(1), V(3), V(5)]
"#;
    check(src, want);
}

#[test]
fn dict_methods_and_attr_builtins() {
    let src = r#"
d = {"a": 1, "b": 2}
print(d.setdefault("a", 9), d.setdefault("c", 3))
print(d.pop("b"), d.pop("z", -1))
print(d.popitem())
d.update([("x", 10)])
print(d)
print(pow(2, 10), pow(2, 10, 1000), issubclass(bool, int))
class A: pass
class B(A): pass
print(issubclass(B, A), issubclass(A, B))
o = B()
setattr(o, "n", 5)
print(getattr(o, "n"), getattr(o, "z", 0), hasattr(o, "n"), hasattr(o, "z"))
"#;
    let want = r#"
1 3
2 -1
('c', 3)
{'a': 1, 'x': 10}
1024 24 True
True False
5 0 True False
"#;
    check(src, want);
}

#[test]
fn method_keyword_args() {
    let src = r#"
l = [3, 1, 2]
l.sort(key=lambda x: -x)
print(l)
w = ["bb", "a", "ccc"]
w.sort(key=len, reverse=True)
print(w)
class G:
    def hi(self, name, punct="!"):
        return name + punct
print(G().hi("a"), G().hi("b", punct="?"))
"#;
    let want = r#"
[3, 2, 1]
['ccc', 'bb', 'a']
a! b?
"#;
    check(src, want);
}

#[test]
fn list_methods_and_ordering() {
    let src = r#"
l = [3, 1, 2]
l.append(4)
l.extend([5, 6])
l.insert(0, 0)
print(l)
l.remove(3)
print(l, l.index(4), l.count(2))
l.sort()
print(l)
l.reverse()
print(l)
print([3, 1, 2] < [3, 1, 3], (1, 2) < (1, 2, 0))
print(sorted([(2, "b"), (1, "z"), (1, "a")]))
"#;
    let want = r#"
[0, 3, 1, 2, 4, 5, 6]
[0, 1, 2, 4, 5, 6] 3 1
[0, 1, 2, 4, 5, 6]
[6, 5, 4, 2, 1, 0]
True True
[(1, 'a'), (1, 'z'), (2, 'b')]
"#;
    check(src, want);
}
