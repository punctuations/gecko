class Box:
    kind = "box"
    def set(self, v):
        self.kind = v
b = Box()
print(b.kind, Box.kind)
b.set("crate")
print(b.kind, Box.kind)
