class Adder:
    def __init__(self, base):
        self.base = base
    def make(self):
        b = self.base
        def add(x):
            return b + x
        return add
f = Adder(10).make()
print(f(5), f(7))
