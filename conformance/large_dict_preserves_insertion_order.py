d = {}
for i in range(15):
    d[i * 3] = i
out = []
for k in d:
    out.append(k)
print(out[0], out[7], out[14])
