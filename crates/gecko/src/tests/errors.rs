use super::super::run_source;
use super::check;

#[test]
fn syntax_error_reports() {
    assert!(run_source("print(\n").is_err());
}

#[test]
fn with_suppresses_exception() {
    let src = r#"
class Q:
    def __enter__(self):
        return self
    def __exit__(self, t, v, tb):
        return True
with Q():
    raise ValueError("x")
print("survived")
"#;
    let want = r#"
survived
"#;
    check(src, want);
}

#[test]
fn name_error_reports_the_name() {
    let f = run_source("print(missing)\n").unwrap_err();
    assert_eq!(f.message, "NameError: name 'missing' is not defined");
}

#[test]
fn output_before_an_error_is_kept() {
    let f = run_source("print(\"before\")\n1 / 0\n").unwrap_err();
    assert_eq!(f.output, "before\n");
    assert_eq!(f.message, "ZeroDivisionError: division by zero");
}

#[test]
fn wrong_arity_is_a_type_error() {
    let f = run_source("def f(a):\n    return a\nf(1, 2)\n").unwrap_err();
    assert!(f.message.contains("takes 1 positional argument"));
}

#[test]
fn recursion_limit_is_enforced() {
    let f = run_source("def f(n):\n    return f(n)\nf(1)\n").unwrap_err();
    assert!(f.message.contains("RecursionError"));
}

#[test]
fn an_operand_stack_overflow_is_a_clean_error() {
    let mut src = String::from("x = [");
    for i in 0..1100 {
        if i > 0 {
            src.push(',');
        }
        src.push_str(&i.to_string());
    }
    src.push_str("]\nprint(len(x))\n");
    let f = run_source(&src).unwrap_err();
    assert!(f.message.contains("value stack overflow"), "{}", f.message);
}

#[test]
fn too_many_positionals_without_varargs_errors() {
    let f = run_source("def f(a, b):\n    return a\nf(1, 2, 3)\n").unwrap_err();
    assert!(
        f.message.contains("positional arguments but 3"),
        "{}",
        f.message
    );
}

#[test]
fn unexpected_keyword_argument_errors() {
    let f = run_source("def f(a):\n    return a\nf(x=1)\n").unwrap_err();
    assert!(
        f.message.contains("unexpected keyword argument"),
        "{}",
        f.message
    );
}

#[test]
fn duplicate_argument_value_errors() {
    let f = run_source("def f(a, b):\n    return a\nf(1, a=2)\n").unwrap_err();
    assert!(
        f.message.contains("multiple values for argument"),
        "{}",
        f.message
    );
}

#[test]
fn too_few_arguments_still_errors_with_defaults() {
    let f = run_source("def f(a, b=1):\n    return a\nf()\n").unwrap_err();
    assert!(f.message.contains("positional argument"));
}

#[test]
fn constant_division_by_zero_stays_a_runtime_error() {
    let f = run_source("print(1 // 0)\n").unwrap_err();
    assert!(f.message.contains("ZeroDivisionError"), "{}", f.message);
}

#[test]
fn exceptions_catch_by_type() {
    let src = r#"
try:
    1 / 0
except ZeroDivisionError as e:
    print("caught:", e)
"#;
    let want = r#"
caught: division by zero
"#;
    check(src, want);
}

#[test]
fn exceptions_pick_the_first_matching_handler() {
    let src = r#"
try:
    {}["k"]
except ValueError:
    print("wrong")
except KeyError:
    print("right")
except Exception:
    print("late")
"#;
    let want = r#"
right
"#;
    check(src, want);
}

#[test]
fn raise_and_catch_with_else() {
    let src = r#"
def risky(n):
    if n > 2:
        raise ValueError("too big")
    return n
try:
    print(risky(1))
except ValueError:
    print("unseen")
else:
    print("else")
try:
    risky(9)
except ValueError as e:
    print(e)
else:
    print("unseen")
"#;
    let want = r#"
1
else
too big
"#;
    check(src, want);
}

#[test]
fn finally_runs_on_both_paths() {
    let src = r#"
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
"#;
    let want = r#"
ok
cleanup
inner cleanup
outer
"#;
    check(src, want);
}

#[test]
fn exceptions_propagate_through_calls() {
    let src = r#"
def f():
    raise IndexError("deep")
def g():
    return f()
try:
    g()
except IndexError as e:
    print(e)
"#;
    let want = r#"
deep
"#;
    check(src, want);
}

#[test]
fn uncaught_exceptions_keep_their_message() {
    let f = run_source("raise ValueError(\"unhandled\")\n").unwrap_err();
    assert_eq!(f.message, "ValueError: unhandled");
    let f = run_source("try:\n    1 / 0\nexcept KeyError:\n    pass\n").unwrap_err();
    assert_eq!(f.message, "ZeroDivisionError: division by zero");
}

#[test]
fn raising_a_non_exception_is_a_type_error() {
    let f = run_source("raise 42\n").unwrap_err();
    assert!(f.message.contains("must derive from BaseException"));
}

#[test]
fn exception_reprs_follow_python() {
    let src = r#"
e = ValueError("kept")
print(e, [e], ValueError)
print(TypeError())
print([TypeError()])
"#;
    let want = r#"
kept [ValueError('kept')] <class 'ValueError'>

[TypeError()]
"#;
    check(src, want);
}

#[test]
fn return_through_finally_runs_it() {
    let src = r#"
def f():
    try:
        return 1
    finally:
        print("x")
print(f())
"#;
    let want = r#"
x
1
"#;
    check(src, want);
}

#[test]
fn percent_formatting_errors() {
    let src = r#"
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
"#;
    let want = r#"
TypeError
TypeError
TypeError
ValueError
KeyError
"#;
    check(src, want);
}

#[test]
fn bare_raise_reraises() {
    let src = r#"
def f(x):
    try:
        if x < 0:
            raise ValueError("neg")
        return x
    except ValueError:
        print("log")
        raise
for v in [2, -1]:
    try:
        print(f(v))
    except ValueError as e:
        print("caught", e)
try:
    try:
        raise KeyError("a")
    except KeyError:
        raise
except KeyError:
    print("reraised")
"#;
    let want = r#"
2
log
caught neg
reraised
"#;
    check(src, want);
}

#[test]
fn with_exit_gets_exception_type() {
    let src = r#"
class C:
    def __enter__(self):
        return self
    def __exit__(self, t, v, tb):
        print(t is ValueError)
        return True
with C():
    raise ValueError("x")
print("ok")
"#;
    let want = r#"
True
ok
"#;
    check(src, want);
}

#[test]
fn unpack_arity_mismatch_is_a_value_error() {
    let f = run_source("a, b = [1, 2, 3]\n").unwrap_err();
    assert!(f.message.contains("too many values to unpack"));
    let f = run_source("a, b, c = (1, 2)\n").unwrap_err();
    assert!(f.message.contains("not enough values"));
}
