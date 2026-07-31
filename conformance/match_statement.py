def d(x):
    match x:
        case 0:
            return "zero"
        case 1 | 2 | 3:
            return "small"
        case n if n > 100:
            return "huge"
        case n:
            return "other"
print(d(0), d(2), d(200), d(50))
match "hi":
    case "hi" as g:
        print(g)
