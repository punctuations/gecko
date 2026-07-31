keep = []
for i in range(100):
    keep.append("v" + "x")
d = {"total": 0}
i = 0
while i < 20000:
    junk = ["g", {"k": "v"}, i]
    i += 1
d["total"] = len(keep)
print(d["total"], keep[0], keep[99], d)
