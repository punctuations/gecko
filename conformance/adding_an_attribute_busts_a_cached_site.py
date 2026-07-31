class G:
    pass
def read(o):
    return o.a
g = G()
g.a = 1
print(read(g))
g.b = 2
print(read(g), g.b)
