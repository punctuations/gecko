from ._native import native


def spawn(state, handle, args=None):
    if native is None:
        raise NotImplementedError(
            "actors need the gecko runtime; the CPython fallback is not built yet"
        )
    if args is None:
        return native.actor.spawn(state, handle)
    return native.actor.spawn(state, handle, args)


def supervise(state, handle, args=None, restarts=3, period=5000):
    if native is None:
        raise NotImplementedError(
            "actors need the gecko runtime; the CPython fallback is not built yet"
        )
    return native.actor.supervise(state, handle, args, restarts, period)


def stop():
    if native is None:
        raise NotImplementedError(
            "actors need the gecko runtime; the CPython fallback is not built yet"
        )
    return native.actor.stop()
