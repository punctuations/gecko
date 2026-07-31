try:
    raise RuntimeError("boom")
except (ValueError, RuntimeError) as e:
    print(e)
