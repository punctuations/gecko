class C:
    def __enter__(self):
        return 9
    def __exit__(self, t, v, tb):
        print("exit")
def f():
    with C() as v:
        return v
print(f())
