def tag(label):
    def deco(f):
        def w(x):
            return label + ":" + f(x)
        return w
    return deco
@tag("r")
def shout(s):
    return s
print(shout("hi"))
