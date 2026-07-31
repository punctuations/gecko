class Node:
    def __init__(self, v):
        self.v = v
keep = Node(7)
i = 0
while i < 20000:
    tmp = Node(i)
    i += 1
print(keep.v)
