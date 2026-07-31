def twice(f):
    def w(x):
        return f(f(x))
    return w
@twice
def inc(n):
    return n + 1
print(inc(10))
