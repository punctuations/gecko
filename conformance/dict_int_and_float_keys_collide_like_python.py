d = {}
for i in range(12):
    d[i] = i
print(d[5.0])
d[5.0] = 100
print(d[5], len(d))
