class A:
    def __init__(self):
        self.x = 1
        self.y = 2
class B:
    def __init__(self):
        self.y = 10
        self.x = 20
def getx(o):
    return o.x
a = A()
b = B()
print(getx(a), getx(b), getx(a), getx(b))
