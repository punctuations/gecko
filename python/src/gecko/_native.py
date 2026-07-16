try:
    import _gecko as native
except ImportError:
    native = None


def available():
    return native is not None
