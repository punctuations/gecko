class Suspend:
    def __await__(self):
        yield
async def worker(name, n):
    i = 0
    while i < n:
        await Suspend()
        print(name, i)
        i = i + 1
def run_all(active):
    while active:
        still = []
        for c in active:
            try:
                c.send(None)
                still.append(c)
            except StopIteration:
                pass
        active = still
run_all([worker("A", 3), worker("B", 2)])
