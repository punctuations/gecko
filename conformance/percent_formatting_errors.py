for f in [1, 2, 3, 4, 5]:
    try:
        if f == 1:
            print('%d' % 'x')
        elif f == 2:
            print('%d %d' % (1,))
        elif f == 3:
            print('%d' % (1, 2))
        elif f == 4:
            print('%z' % 1)
        else:
            print('%(k)s' % {'j': 1})
    except TypeError:
        print('TypeError')
    except ValueError:
        print('ValueError')
    except KeyError:
        print('KeyError')
