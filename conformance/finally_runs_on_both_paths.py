try:
    print("ok")
finally:
    print("cleanup")
try:
    try:
        raise TypeError("x")
    finally:
        print("inner cleanup")
except TypeError:
    print("outer")
