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
