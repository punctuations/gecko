class C:
    def f(self):
        return 1
def plain(v):
    return v + 100
c = C()
print(c.f())
c.f = plain
print(c.f(5))
