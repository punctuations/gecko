def risky(n):
    if n > 2:
        raise ValueError("too big")
    return n
try:
    print(risky(1))
except ValueError:
    print("unseen")
else:
    print("else")
try:
    risky(9)
except ValueError as e:
    print(e)
else:
    print("unseen")
