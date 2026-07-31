def count(n):
    i = 0
    while i < n:
        yield i
        i = i + 1
print([x for x in count(4)])
g = count(2)
print(next(g))
print(next(g))
try:
    next(g)
except StopIteration:
    print("done")
