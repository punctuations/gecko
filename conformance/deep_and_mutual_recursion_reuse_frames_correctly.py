def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)
def is_even(n):
    if n == 0:
        return True
    return is_odd(n - 1)
def is_odd(n):
    if n == 0:
        return False
    return is_even(n - 1)
print(fib(20), is_even(200), is_odd(101))
