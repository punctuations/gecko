def sieve(n):
    flags = [True for _ in range(n)]
    flags[0] = False
    flags[1] = False
    i = 2
    while i * i < n:
        if flags[i]:
            j = i * i
            while j < n:
                flags[j] = False
                j += i
        i += 1
    c = 0
    for f in flags:
        if f:
            c += 1
    return c
print(sieve(1000000))
