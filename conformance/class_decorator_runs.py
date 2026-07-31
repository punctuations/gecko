seen = []
def register(cls):
    seen.append(cls)
    return cls
@register
class W:
    def __init__(self):
        self.name = "w"
print(W().name, len(seen))
