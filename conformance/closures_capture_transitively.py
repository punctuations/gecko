def a():
    x = 7
    def b():
        def inner():
            return x
        return inner()
    return b()
print(a())
