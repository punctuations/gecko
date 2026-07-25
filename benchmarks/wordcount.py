BASE = "the quick brown fox jumps over the lazy dog"

def run(reps):
    parts = []
    for _ in range(reps):
        parts.append(BASE)
    words = " ".join(parts).split()
    counts = {}
    for w in words:
        counts[w] = counts.get(w, 0) + 1
    total = 0
    for k in sorted(counts):
        total += counts[k] * len(k)
    return total

t = 0
for _ in range(60):
    t = run(1500)
print(t)
