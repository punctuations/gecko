def uselen(a):
    return len(a)
print(uselen([1, 2, 3]))
len = 99
try:
    uselen([1, 2, 3])
except TypeError:
    print("shadowed")
