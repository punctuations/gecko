def late():
    fs = []
    for i in range(3):
        def f():
            return i
        fs.append(f)
    return fs
fs = late()
print(fs[0](), fs[1](), fs[2]())
