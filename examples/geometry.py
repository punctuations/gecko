PI = 3


def circle_area(r):
    return PI * r * r


def rect_area(w, h):
    return w * h


class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y

    def manhattan(self, other):
        dx = self.x - other.x
        dy = self.y - other.y
        if dx < 0:
            dx = -dx
        if dy < 0:
            dy = -dy
        return dx + dy
