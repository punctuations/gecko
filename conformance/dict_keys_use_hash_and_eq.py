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
