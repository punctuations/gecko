class N:
    def __init__(self, v):
        self.v = v
        self.next = None
head = None
for i in range(20000):
    n = N(i)
    n.next = head
    head = n
s = 0
cur = head
while cur is not None:
    s = s + cur.v
    cur = cur.next
print(s)
