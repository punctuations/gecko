class Q:
    def __enter__(self):
        return self
    def __exit__(self, t, v, tb):
        return True
with Q():
    raise ValueError("x")
print("survived")
