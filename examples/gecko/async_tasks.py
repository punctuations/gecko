class _Suspend:
    def __await__(self):
        yield


async def fetch(url, size):
    await _Suspend()
    return f"data from {url} ({size})"


async def worker(name, urls):
    for url in urls:
        result = await fetch(url, len(url))
        print(name, "->", result)


def run_all(coros):
    active = coros
    while active:
        still = []
        for c in active:
            try:
                c.send(None)
                still.append(c)
            except StopIteration:
                pass
        active = still


run_all(
    [
        worker("A", ["one", "two"]),
        worker("B", ["three", "four"]),
    ]
)
