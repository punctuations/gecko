class Temp:
    def __init__(self, c):
        self._c = c

    @property
    def celsius(self):
        return self._c

    @celsius.setter
    def celsius(self, v):
        if v < -273:
            raise ValueError("too cold")
        self._c = v

    @property
    def fahrenheit(self):
        return self._c * 9 / 5 + 32

    @staticmethod
    def freezing():
        return 0

    @classmethod
    def boiling(cls):
        return cls(100)

t = Temp(25)
print(t.celsius, t.fahrenheit)
t.celsius = 30
print(t.celsius, t.fahrenheit)
try:
    t.celsius = -300
except ValueError:
    print("rejected")
print(t.celsius)
try:
    t.fahrenheit = 5
except AttributeError:
    print("read-only")
print(Temp.freezing(), t.freezing())
b = Temp.boiling()
print(b.celsius, type(b) is Temp)
b2 = t.boiling()
print(b2.celsius)
