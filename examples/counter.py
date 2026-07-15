def make_counter(start):
    count = start

    def inc():
        nonlocal count
        count += 1
        return count

    def peek():
        return count

    return [inc, peek]


pair = make_counter(10)
inc = pair[0]
peek = pair[1]

inc()
inc()
print(peek())
print(inc())
print(peek())
