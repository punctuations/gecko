class Base:
    def m(self):
        return 42
class Mid(Base):
    pass
class Leaf(Mid):
    pass
print(Leaf().m(), Leaf().m())
