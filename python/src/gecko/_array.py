from ._native import native


def array(data, dtype="f64"):
    if native is None:
        raise NotImplementedError(
            "typed arrays need the gecko runtime; the CPython fallback is not built yet"
        )
    return native.array(data, dtype=dtype)
