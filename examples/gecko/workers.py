from gecko import actor


def worker(state, message):
    message[1].send(message[0] * message[0])
    return state


def square(n):
    def build(reply):
        return [n, reply]

    return build


a = actor.spawn(None, worker)
b = actor.spawn(None, worker)
print(a.call(square(6), 1000))
print(b.call(square(9), 1000))
