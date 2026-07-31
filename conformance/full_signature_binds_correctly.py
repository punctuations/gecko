def f(a, b=10, *args, **kw):
    return (a, b, args, kw)
print(f(1))
print(f(1, 2, 3, 4, p=5))
