try:
    {}["k"]
except ValueError:
    print("wrong")
except KeyError:
    print("right")
except Exception:
    print("late")
