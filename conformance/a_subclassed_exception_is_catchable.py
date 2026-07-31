class MyError(Exception):
    pass
try:
    raise MyError("boom")
except MyError:
    print("caught")
