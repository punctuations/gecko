FACTOR = 100
class W:
    FACTOR = 3
    def get(self):
        return FACTOR
print(W().get(), W.FACTOR)
