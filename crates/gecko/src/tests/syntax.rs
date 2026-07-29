use super::super::run_source;
use super::check;

#[test]
fn hello_world() {
    check("print(\"hello world\")\n", "hello world\n");
}

#[test]
fn assignment_then_use() {
    let src = r#"
x = 41
print(x + 1)
"#;
    let want = r#"
42
"#;
    check(src, want);
}

#[test]
fn if_else_branches() {
    let src = r#"
if 1 < 2:
    print("yes")
else:
    print("no")
"#;
    let want = r#"
yes
"#;
    check(src, want);
}

#[test]
fn while_loop_sums() {
    let src =
        "i = 1\ntotal = 0\nwhile i <= 5:\n    total = total + i\n    i = i + 1\nprint(total)\n";
    assert_eq!(run_source(src).unwrap(), "15\n");
}

#[test]
fn short_circuit_and_or() {
    let src = r#"
print(1 and 2)
print(0 or 5)
"#;
    let want = r#"
2
5
"#;
    check(src, want);
}

#[test]
fn ternary_expression() {
    let src = r#"
x = 5
print("big" if x > 3 else "small")
"#;
    let want = r#"
big
"#;
    check(src, want);
}

#[test]
fn global_statement() {
    let src = r#"
c = 0
def bump():
    global c
    c = c + 1
bump()
bump()
print(c)
"#;
    let want = r#"
2
"#;
    check(src, want);
}

#[test]
fn assert_statement() {
    let src = r#"
try:
    assert 1 > 2, "nope"
except AssertionError as e:
    print(e)
"#;
    let want = r#"
nope
"#;
    check(src, want);
}

#[test]
fn chained_assignment_and_comparison() {
    let src = r#"
a = b = 3
print(a, b)
print(0 < a < 10)
print(1 < 2 < 3 < 4)
print(1 < 2 < 9 < 4)
"#;
    let want = r#"
3 3
True
True
False
"#;
    check(src, want);
}

#[test]
fn del_statement() {
    let src = r#"
d = {"x": 1, "y": 2}
del d["x"]
print(d)
"#;
    let want = r#"
{'y': 2}
"#;
    check(src, want);
}

#[test]
fn with_statement() {
    let src = r#"
class C:
    def __enter__(self):
        return 7
    def __exit__(self, a, b, c):
        print("exit")
with C() as v:
    print(v)
"#;
    let want = r#"
7
exit
"#;
    check(src, want);
}

#[test]
fn extended_arg_on_a_jump_target() {
    let mut src = String::from("x = 0\nfor j in range(3):\n");
    for i in 0..200 {
        src.push_str(&format!("    x = x + {i} - {i}\n"));
    }
    src.push_str("    x = x + 1\nprint(x)\n");
    assert_eq!(run_source(&src).unwrap(), "3\n");
}

#[test]
fn a_cached_global_reads_its_live_value() {
    let src = r#"
def get():
    return x
x = 1
print(get())
x = 2
print(get())
"#;
    let want = r#"
1
2
"#;
    check(src, want);
}

#[test]
fn many_globals_resolve_after_indexing() {
    let mut src = String::new();
    for i in 0..20 {
        src.push_str(&format!("g{i} = {i}\n"));
    }
    src.push_str("print(g0, g11, g19)\ng11 = 100\nprint(g11)\n");
    assert_eq!(run_source(&src).unwrap(), "0 11 19\n100\n");
}

#[test]
fn constant_folding_matches_runtime_evaluation() {
    let cases = [
        ("1 + 2 * 3", "7"),
        ("7 // 2", "3"),
        ("-7 // 2", "-4"),
        ("7 % 3", "1"),
        ("-7 % 3", "2"),
        ("6 / 2", "3.0"),
        ("2000000000 + 2000000000", "4000000000"),
        ("1000000 * 1000000", "1000000000000"),
        ("2.5 * 4", "10.0"),
        ("- -5", "5"),
        ("not 0", "True"),
        ("not \"x\"", "False"),
    ];
    for (expr, want) in cases {
        let got = run_source(&format!("print({expr})\n")).unwrap();
        assert_eq!(got, format!("{want}\n"), "folding {expr}");
    }
}

#[test]
fn is_and_is_not_compare_identity() {
    let src = r#"
x = None
print(x is None)
print(x is not None)
y = 5
print(y is not None)
"#;
    let want = r#"
True
False
True
"#;
    check(src, want);
}

#[test]
fn try_inside_a_loop_unwinds_to_the_iterator() {
    let src = r#"
kept = []
for i in range(5):
    try:
        if i % 2 == 0:
            raise ValueError("skip")
        kept.append(i)
    except ValueError:
        pass
print(kept)
"#;
    let want = r#"
[1, 3]
"#;
    check(src, want);
}

#[test]
fn match_statement() {
    let src = r#"
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
"#;
    let want = r#"
zero small huge other
hi
"#;
    check(src, want);
}

#[test]
fn walrus() {
    let src = r#"
if (n := len([1, 2, 3])) > 2:
    print(n)
def f(x):
    if (d := x * 2) > 5:
        return d
    return 0
print(f(4))
"#;
    let want = r#"
3
8
"#;
    check(src, want);
}

#[test]
fn with_return_runs_exit() {
    let src = r#"
class C:
    def __enter__(self):
        return 9
    def __exit__(self, t, v, tb):
        print("exit")
def f():
    with C() as v:
        return v
print(f())
"#;
    let want = r#"
exit
9
"#;
    check(src, want);
}

#[test]
fn break_skips_else_and_continue_skips_body() {
    let src = r#"
for i in range(9):
    if i == 2:
        break
else:
    print("unseen")
print(i)
out = []
for j in range(5):
    if j % 2 == 0:
        continue
    out.append(j)
print(out)
k = 0
while True:
    k += 1
    if k == 3:
        break
print(k)
"#;
    let want = r#"
2
[1, 3]
3
"#;
    check(src, want);
}

#[test]
fn nested_break_binds_the_inner_loop() {
    let src = r#"
hits = []
for a in range(3):
    for b in range(9):
        if b > a:
            break
        hits.append((a, b))
print(hits)
"#;
    let want = r#"
[(0, 0), (1, 0), (1, 1), (2, 0), (2, 1), (2, 2)]
"#;
    check(src, want);
}

#[test]
fn comprehensions_close_over_enclosing_scopes() {
    let src = r#"
def scaled(factor):
    return [n * factor for n in range(4)]
print(scaled(3))
print([[y + 1 for y in range(x)] for x in range(3)])
n = 9
print([n for _ in range(2)])
"#;
    let want = r#"
[0, 3, 6, 9]
[[], [1], [1, 2]]
[9, 9]
"#;
    check(src, want);
}

#[test]
fn comprehension_variables_stay_local() {
    let src = r#"
x = "kept"
l = [x for x in range(3)]
print(x, l)
"#;
    let want = r#"
kept [0, 1, 2]
"#;
    check(src, want);
}

#[test]
fn cells_survive_collection() {
    let src = r#"
def counter():
    n = 0
    def inc():
        nonlocal n
        n += 1
        return n
    return inc
c = counter()
j = 0
while j < 20000:
    g = ["x" + "y", {"k": j}]
    j += 1
print(c(), c())
"#;
    let want = r#"
1 2
"#;
    check(src, want);
}

#[test]
fn garbage_stays_bounded() {
    let src =
        "i = 0\nwhile i < 20000:\n    s = \"a\" + \"b\"\n    l = [s, {\"k\": s}]\n    i += 1\n";
    let code = compiler::compile(&parser::parse(src).unwrap()).unwrap();
    let mut vm = runtime::Vm::new();
    let run = vm.run(&code);
    assert!(!run.error, "{}", run.message);
    assert!(
        vm.heap_live() < 5000,
        "heap has {} live objects",
        vm.heap_live()
    );
}

#[test]
fn survivors_keep_their_contents() {
    let src = r#"
keep = []
for i in range(100):
    keep.append("v" + "x")
d = {"total": 0}
i = 0
while i < 20000:
    junk = ["g", {"k": "v"}, i]
    i += 1
d["total"] = len(keep)
print(d["total"], keep[0], keep[99], d)
"#;
    let want = r#"
100 vx vx {'total': 100}
"#;
    check(src, want);
}

#[test]
fn collect_reclaims_unreachable_values() {
    let src = "l = [\"a\" + \"b\"]\nl = 0\n";
    let code = compiler::compile(&parser::parse(src).unwrap()).unwrap();
    let mut vm = runtime::Vm::new();
    let run = vm.run(&code);
    assert!(!run.error);
    let before = vm.heap_live();
    vm.collect();
    assert!(vm.heap_live() < before);
}
