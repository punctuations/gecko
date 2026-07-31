def g(a, b, *rest):
    return (a, b, rest)
xs = [2, 3, 4]
print(g(1, *xs))
d = {'y': 9}
def h(**k):
    return k
print(h(x=1, **d))
