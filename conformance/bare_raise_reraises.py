def f(x):
    try:
        if x < 0:
            raise ValueError("neg")
        return x
    except ValueError:
        print("log")
        raise
for v in [2, -1]:
    try:
        print(f(v))
    except ValueError as e:
        print("caught", e)
try:
    try:
        raise KeyError("a")
    except KeyError:
        raise
except KeyError:
    print("reraised")
