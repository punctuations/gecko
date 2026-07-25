def norm(a, b):
    return a * a + b * b

def run():
    t = 0
    for i in range(600000):
        j = i % 1000
        t = (t + norm(j, j + 1)) % 1000003
    return t
print(run())
