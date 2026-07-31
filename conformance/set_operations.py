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
