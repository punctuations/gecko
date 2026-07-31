async def c():
    return 1
try:
    next(c())
except TypeError as e:
    print(e)
