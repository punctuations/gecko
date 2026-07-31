print(2 ** 100)
def fact(n):
    r = 1
    for i in range(1, n + 1):
        r *= i
    return r
print(fact(25))
print(10 ** 30 + 1)
print(2 ** 100 // 7, 2 ** 100 % 7)
print(-(2 ** 70))
print(2 ** 100 == 2 ** 100, 2 ** 100 > 2 ** 99)
x = 123456789012345678901234567890
print(x + x)
print(x * 1000000)
print(divmod(x, 7))
print(abs(-x), x > 0)
print(1000000 * 1000000)
print(type(2 ** 100) is int, isinstance(2 ** 100, int))
