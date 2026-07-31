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
