hits = []
for a in range(3):
    for b in range(9):
        if b > a:
            break
        hits.append((a, b))
print(hits)
