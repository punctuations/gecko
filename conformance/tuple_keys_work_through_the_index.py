d = {}
for i in range(10):
    d[(i, i + 1)] = i
print(d[(3, 4)], (7, 8) in d, (7, 9) in d)
