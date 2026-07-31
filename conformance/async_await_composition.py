async def add(a, b):
    return a + b
async def compute():
    x = await add(2, 3)
    return await add(x, 10)
def drive(coro):
    try:
        while True:
            coro.send(None)
    except StopIteration:
        pass
drive(compute())
print("ran")
