def memoize(f):
    cache = {}

    def wrapper(n):
        if n in cache:
            return cache[n]
        result = f(n)
        cache[n] = result
        return result

    return wrapper


calls = []


def trace(f):
    def wrapper(n):
        calls.append(n)
        return f(n)

    return wrapper


@memoize
@trace
def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)


print(fib(10))
print("traced calls:", len(calls))
