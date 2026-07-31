for i in range(9):
    if i == 2:
        break
else:
    print("unseen")
print(i)
out = []
for j in range(5):
    if j % 2 == 0:
        continue
    out.append(j)
print(out)
k = 0
while True:
    k += 1
    if k == 3:
        break
print(k)
