def run():
    t = 0
    for i in range(3000000):
        t = (t + i * 2) % 1000003
    return t
print(run())
