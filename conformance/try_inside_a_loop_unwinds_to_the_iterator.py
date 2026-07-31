kept = []
for i in range(5):
    try:
        if i % 2 == 0:
            raise ValueError("skip")
        kept.append(i)
    except ValueError:
        pass
print(kept)
