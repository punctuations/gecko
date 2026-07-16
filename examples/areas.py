import geometry
from geometry import Point, rect_area

print("circle:", geometry.circle_area(2))
print("rect:", rect_area(3, 4))

shapes = [rect_area(w, h) for w, h in [(1, 2), (3, 3), (4, 5)]]
total = 0
for area in shapes:
    total += area
print("shapes:", shapes, "total:", total)

a = Point(0, 0)
b = Point(3, 4)
print("distance:", a.manhattan(b))
