def pair():
    v = 0
    def set5():
        nonlocal v
        v = 5
    def get():
        return v
    set5()
    return get()
print(pair())
