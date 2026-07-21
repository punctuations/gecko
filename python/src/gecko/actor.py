from ._native import native


def spawn(state, handle, args=None):
    if native is None:
        raise NotImplementedError(
            "actors need the gecko runtime; the CPython fallback is not built yet"
        )
    if args is None:
        return native.actor.spawn(state, handle)
    return native.actor.spawn(state, handle, args)
