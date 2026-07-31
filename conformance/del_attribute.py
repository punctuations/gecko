class P:
    def __init__(self):
        self.v = 1
p = P()
del p.v
try:
    print(p.v)
except AttributeError:
    print("gone")
