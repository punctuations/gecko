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
