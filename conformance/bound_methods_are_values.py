class Counter:
    def __init__(self):
        self.n = 0
    def inc(self):
        self.n += 1
        return self.n
c = Counter()
m = c.inc
print(m(), m(), c.n)
