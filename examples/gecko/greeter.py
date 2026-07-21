from gecko import actor


def handle(state, message, greeting):
    reply = message[1]
    reply.send(greeting + ", " + message[0])
    return state


def ask(name):
    def build(reply):
        return [name, reply]

    return build


greeter = actor.spawn(None, handle, ["hello"])
print(greeter.call(ask("world"), 1000))
print(greeter.call(ask("gecko"), 1000))
