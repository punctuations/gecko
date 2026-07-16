from ._native import native


class SandboxError(Exception):
    pass


def run(source, steps=0, memory=0, millis=0):
    if native is not None:
        return native.sandbox.run(source, steps, memory, millis)
    return _run_pure(source)


def _run_pure(source):
    import io
    import sys

    buf = io.StringIO()
    saved = sys.stdout
    sys.stdout = buf
    try:
        exec(compile(source, "<sandbox>", "exec"), {})
    except BaseException as e:
        sys.stdout = saved
        raise SandboxError(str(e))
    sys.stdout = saved
    return buf.getvalue()
