class C:
    def __enter__(self):
        return self
    def __exit__(self, t, v, tb):
        print(t is ValueError)
        return True
with C():
    raise ValueError("x")
print("ok")
