def a(f):
    def w(x):
        return "a(" + f(x) + ")"
    return w
def b(f):
    def w(x):
        return "b(" + f(x) + ")"
    return w
@a
@b
def base(x):
    return x
print(base("X"))
