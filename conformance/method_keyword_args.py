l = [3, 1, 2]
l.sort(key=lambda x: -x)
print(l)
w = ["bb", "a", "ccc"]
w.sort(key=len, reverse=True)
print(w)
class G:
    def hi(self, name, punct="!"):
        return name + punct
print(G().hi("a"), G().hi("b", punct="?"))
