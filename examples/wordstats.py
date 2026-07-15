words = ["gecko", "setae", "toe", "pad", "grip", "wall"]

lengths = {w: len(w) for w in words}
short = [w for w in words if len(w) <= 4]
pairs = [(w, n) for w, n in lengths.items() if n > 3]

longest = ""
for w in words:
    if w == "wall":
        break
    if len(w) <= len(longest):
        continue
    longest = w

print(lengths)
print(short)
print(pairs)
print(longest)
