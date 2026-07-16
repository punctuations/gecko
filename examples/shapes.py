class Shape:
    def __init__(self, name):
        self.name = name

    def area(self):
        return 0


class Rectangle(Shape):
    def __init__(self, w, h):
        self.name = "rectangle"
        self.w = w
        self.h = h

    def area(self):
        return self.w * self.h


class Square(Rectangle):
    def __init__(self, side):
        self.name = "square"
        self.w = side
        self.h = side


shapes = [Rectangle(3, 4), Square(5), Shape("nothing")]
for s in shapes:
    print(s.name, "area", s.area())

total = 0
for s in shapes:
    total += s.area()
print("total area:", total)
