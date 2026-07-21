from gecko import actor


def handle(count, message):
    if message[0] == "add":
        return count + message[1]
    message[1].send(count)
    return count


def get(reply):
    return ["get", reply]


counter = actor.spawn(0, handle)
counter.send(["add", 5])
counter.send(["add", 3])
print(counter.call(get, 1000))
