def counter():
    n = 0
    def inc():
        nonlocal n
        n += 1
        return n
    return inc
c = counter()
j = 0
while j < 20000:
    g = ["x" + "y", {"k": j}]
    j += 1
print(c(), c())
