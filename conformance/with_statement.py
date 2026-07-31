class C:
    def __enter__(self):
        return 7
    def __exit__(self, a, b, c):
        print("exit")
with C() as v:
    print(v)
