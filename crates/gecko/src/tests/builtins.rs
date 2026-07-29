use super::check;

#[test]
fn floats_print_shortest_repr() {
    let src = r#"
print(100.0)
print(2.5)
print(340.0 / 9.0)
print(0.0001)
print(1e16)
"#;
    let want = r#"
100.0
2.5
37.77777777777778
0.0001
1e+16
"#;
    check(src, want);
}

#[test]
fn nested_containers_print_reprs() {
    check(
        "print([1, [\"s\", {\"k\": None}]])\n",
        "[1, ['s', {'k': None}]]\n",
    );
}

#[test]
fn a_cached_builtin_busts_when_a_global_shadows_it() {
    let src = r#"
def uselen(a):
    return len(a)
print(uselen([1, 2, 3]))
len = 99
try:
    uselen([1, 2, 3])
except TypeError:
    print("shadowed")
"#;
    let want = r#"
3
shadowed
"#;
    check(src, want);
}

#[test]
fn a_global_shadows_a_builtin() {
    let src = r#"
print(len([1, 2]))
len = 42
print(len)
"#;
    let want = r#"
2
42
"#;
    check(src, want);
}

#[test]
fn fstrings() {
    let src = r#"
name = "gecko"
n = 3
print(f"{name} has {n + 1} legs")
print(f"{name!r}")
print(f"{{esc}} {name}")
"#;
    let want = r#"
gecko has 4 legs
'gecko'
{esc} gecko
"#;
    check(src, want);
}

#[test]
fn core_builtins() {
    let src = r#"
print(str(42), int("17"), float("2.5"), bool([]))
print(list(range(3)), tuple([1, 2]))
print(sum([1, 2, 3]), min(4, 1, 7), max([4, 1, 7]), abs(-9))
print(sorted([3, 1, 2]))
print(list(map(lambda n: n * 2, [1, 2, 3])))
print(list(filter(lambda n: n > 1, [0, 1, 2, 3])))
print(any([0, 1]), all([1, 1]))
"#;
    let want = r#"
42 17 2.5 False
[0, 1, 2] (1, 2)
6 1 7 9
[1, 2, 3]
[2, 4, 6]
[2, 3]
True True
"#;
    check(src, want);
}

#[test]
fn lazy_iterators() {
    let src = r#"
m = map(lambda x: x * 2, [1, 2, 3])
print(next(m))
print(list(m))
print(list(filter(lambda x: x % 2 == 0, range(6))))
print(list(zip([1, 2], "ab")))
print(list(enumerate("xy", start=1)))
print(list(reversed([1, 2, 3])))
print(sum(map(lambda x: x + 1, range(4))))
"#;
    let want = r#"
2
[4, 6]
[0, 2, 4]
[(1, 'a'), (2, 'b')]
[(1, 'x'), (2, 'y')]
[3, 2, 1]
10
"#;
    check(src, want);
}

#[test]
fn sort_min_max_kwargs() {
    let src = r#"
print(sorted([3, 1, 2], reverse=True))
print(sorted(["bb", "a", "ccc"], key=len))
print(min([], default=9))
print(max(["a", "bbb", "cc"], key=len))
print(min([3, 1, 2], key=lambda x: -x))
"#;
    let want = r#"
[3, 2, 1]
['a', 'bb', 'ccc']
9
bbb
3
"#;
    check(src, want);
}

#[test]
fn percent_string_formatting() {
    let src = r#"
print('%d|%s|%r' % (5, 'a', 'a'))
print('%5d|%-5d|%05d' % (42, 42, 42))
print('%+d|% d' % (7, 7))
print('%.2f|%10.3f|%-10.2f|' % (3.14159, 3.14159, 3.14159))
print('%x|%X|%#x|%o|%#o' % (255, 255, 255, 8, 8))
print('%e|%E|%g|%G' % (12345.6789, 12345.6789, 0.00001234, 123456789.0))
print('%c|%c' % (65, 'z'))
print('%%|%s' % 'end')
print('%(a)s-%(b)d' % {'a': 'x', 'b': 3})
print('%.3s|%*d|%.*f' % ('abcdefg', 6, 42, 2, 3.14159))
print('%s|%s|%d|%s' % ([1, 2], None, True, {'k': 1}))
print('%d' % 2 ** 80)
print('%+05d|%+.3d|%.3d|%#.3x|%#.3o' % (-7, 7, -7, 255, 8))
"#;
    let want = r#"
5|a|'a'
   42|42   |00042
+7| 7
3.14|     3.142|3.14      |
ff|FF|0xff|10|0o10
1.234568e+04|1.234568E+04|1.234e-05|1.23457E+08
A|z
%|end
x-3
abc|    42|3.14
[1, 2]|None|1|{'k': 1}
1208925819614629174706176
-0007|+007|-007|0x0ff|0o010
"#;
    check(src, want);
}

#[test]
fn fstring_format_specs() {
    let src = r#"
print(f"{3.14159:.2f}")
print(f"{42:>6}|{42:<6}|{42:^6}|")
print(f"{42:06}", f"{-42:06}")
print(f"{255:x}", f"{255:#x}", f"{10:b}", f"{64:o}")
print(f"{1234567:,}", f"{1234.5:,.2f}")
print(f"{5:+}", f"{0.1234:.1%}")
print(f"{'hi':>5}|", f"{'x':*^7}")
print(f"{len('abc'):03}")
"#;
    let want = r#"
3.14
    42|42    |  42  |
000042 -00042
ff 0xff 1010 100
1,234,567 1,234.50
+5 12.3%
   hi| ***x***
003
"#;
    check(src, want);
}

#[test]
fn more_builtins() {
    let src = r#"
print(round(2.5), round(3.5), round(3.14159, 2), round(1234, -2))
print(divmod(17, 5), divmod(-17, 5))
print(ord("A"), chr(97))
print(hex(255), oct(64), bin(10), hex(-255))
print(repr("hi"), repr([1, 2]))
print(isinstance(5, int), isinstance(True, bool), isinstance(5, bool), isinstance(5, (str, int)))
print(callable(print), callable(5))
"#;
    let want = r#"
2 4 3.14 1200
(3, 2) (-4, 3)
65 a
0xff 0o100 0b1010 -0xff
'hi' [1, 2]
True True False True
True False
"#;
    check(src, want);
}

#[test]
fn type_builtin() {
    let src = r#"
print(type(5) is int)
print(type("a") is str)
print(type([]) is list)
"#;
    let want = r#"
True
True
True
"#;
    check(src, want);
}
