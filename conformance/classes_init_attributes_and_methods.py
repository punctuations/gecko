class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y
    def norm2(self):
        return self.x * self.x + self.y * self.y
p = Point(3, 4)
print(p.x, p.y, p.norm2())
