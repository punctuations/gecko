class C:
    def f(self):
        return 1
def g(self):
    return 2
c = C()
print(c.f())
C.f = g
print(c.f())
