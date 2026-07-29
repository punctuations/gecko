use super::super::run_source;
use super::check;

#[test]
fn functions_call_and_return() {
    let src = r#"
def add(a, b):
    return a + b
print(add(2, 40))
"#;
    let want = r#"
42
"#;
    check(src, want);
}

#[test]
fn functions_recurse() {
    let src = r#"
def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)
print(fib(10))
"#;
    let want = r#"
55
"#;
    check(src, want);
}

#[test]
fn function_without_return_gives_none() {
    let src = r#"
def f():
    pass
print(f())
"#;
    let want = r#"
None
"#;
    check(src, want);
}

#[test]
fn function_locals_shadow_globals() {
    let src = r#"
x = 1
def f():
    x = 2
    return x
print(f(), x)
"#;
    let want = r#"
2 1
"#;
    check(src, want);
}

#[test]
fn wide_arguments_run_through_extended_arg() {
    let mut src = String::new();
    for i in 0..300 {
        src.push_str(&format!("v{i} = {}\n", i + 1000));
    }
    src.push_str("print(v0 + v299)\n");
    assert_eq!(run_source(&src).unwrap(), "2299\n");
}

#[test]
fn parameter_defaults_fill_missing_arguments() {
    let src = r#"
def greet(name, greeting="hi"):
    return greeting + " " + name
print(greet("a"))
print(greet("b", "yo"))
"#;
    let want = r#"
hi a
yo b
"#;
    check(src, want);
}

#[test]
fn parameter_defaults_are_evaluated_once_at_definition() {
    let src = r#"
n = 10
def f(x=n):
    return x
n = 20
print(f())
print(f(1))
"#;
    let want = r#"
10
1
"#;
    check(src, want);
}

#[test]
fn varargs_collect_extra_positionals() {
    let src =
        "def g(a, b, *rest):\n    return (a, b, rest)\nprint(g(1, 2, 3, 4))\nprint(g(1, 2))\n";
    assert_eq!(run_source(src).unwrap(), "(1, 2, (3, 4))\n(1, 2, ())\n");
}

#[test]
fn kwargs_collect_extra_keywords() {
    let src = r#"
def h(a, **k):
    return (a, k)
print(h(1, x=2, y=3))
"#;
    let want = r#"
(1, {'x': 2, 'y': 3})
"#;
    check(src, want);
}

#[test]
fn keyword_arguments_bind_by_name() {
    let src = r#"
def f(a, b):
    return a - b
print(f(b=1, a=10))
print(f(10, b=3))
"#;
    let want = r#"
9
7
"#;
    check(src, want);
}

#[test]
fn full_signature_binds_correctly() {
    let src = r#"
def f(a, b=10, *args, **kw):
    return (a, b, args, kw)
print(f(1))
print(f(1, 2, 3, 4, p=5))
"#;
    let want = r#"
(1, 10, (), {})
(1, 2, (3, 4), {'p': 5})
"#;
    check(src, want);
}

#[test]
fn call_site_spreads_expand() {
    let src = r#"
def g(a, b, *rest):
    return (a, b, rest)
xs = [2, 3, 4]
print(g(1, *xs))
d = {'y': 9}
def h(**k):
    return k
print(h(x=1, **d))
"#;
    let want = r#"
(1, 2, (3, 4))
{'x': 1, 'y': 9}
"#;
    check(src, want);
}

#[test]
fn non_default_after_default_is_rejected() {
    let f = run_source("def f(a=1, b):\n    return a\n").unwrap_err();
    assert!(
        f.message
            .contains("non-default argument follows default argument")
    );
}

#[test]
fn deep_and_mutual_recursion_reuse_frames_correctly() {
    let src = r#"
def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)
def is_even(n):
    if n == 0:
        return True
    return is_odd(n - 1)
def is_odd(n):
    if n == 0:
        return False
    return is_even(n - 1)
print(fib(20), is_even(200), is_odd(101))
"#;
    let want = r#"
6765 True True
"#;
    check(src, want);
}
