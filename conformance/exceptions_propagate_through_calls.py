def f():
    raise IndexError("deep")
def g():
    return f()
try:
    g()
except IndexError as e:
    print(e)
