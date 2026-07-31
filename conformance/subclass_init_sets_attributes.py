class A:
    def __init__(self, name):
        self.name = name
class B(A):
    def greet(self):
        return self.name
b = B("x")
print(b.name, b.greet())
