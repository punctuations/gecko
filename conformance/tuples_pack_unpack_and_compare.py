t = (1, "two")
a, b = t
b, a = a, b
x, (y, z) = 1, (2, 3)
print(t, a, b, x, y, z)
print(t == (1, "two"), (1,) + (2, 3), len(()), 2 in (1, 2))
