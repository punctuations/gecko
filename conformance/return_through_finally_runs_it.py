def f():
    try:
        return 1
    finally:
        print("x")
print(f())
