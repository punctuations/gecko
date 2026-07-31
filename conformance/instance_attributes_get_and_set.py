class P:
    def __init__(self, x, y):
        self.x = x
        self.y = y
p = P(3, 4)
print(p.x, p.y)
p.x = 100
print(p.x, p.y)
p.z = 9
print(p.z)
