def counter():
    n = 0
    def inc():
        nonlocal n
        n += 1
        return n
    return inc
c = counter()
d = counter()
print(c(), c(), d(), c())
