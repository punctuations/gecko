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
