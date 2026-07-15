queue = [(10, 2), (5, 0), (7, "x"), (9, 3)]

results = []
for a, b in queue:
    try:
        results.append(a // b)
    except ZeroDivisionError:
        results.append("div0")
    except TypeError:
        results.append("badtype")

print(results)

try:
    total = 0
    for r in results:
        total += r
except TypeError as e:
    print("cannot sum:", e)
finally:
    print("done")
