use std::process::exit;

mod freeze;
mod install;
mod sandbox;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "usage: gecko [option] [file]
       gecko build file [-o out] [--debug]
run gecko --help for details";

const ART: &str = include_str!("art.txt");

fn print_help() {
    let p = freeze::Paint::auto();
    let art: Vec<&str> = ART.trim_end().lines().collect();
    let width = art.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 2;
    let text = vec![
        format!("{} {}", p.wrap("1;32", "gecko"), p.wrap("1", VERSION)),
        "a fast, embeddable Python runtime".to_string(),
        String::new(),
        p.wrap("1;32", "usage: gecko [option] [file]"),
        help_row(&p, "gecko file.py", "run a program"),
        help_row(&p, "gecko -c source", "run from a string"),
        help_row(&p, "gecko -", "run from stdin"),
        help_row(&p, "gecko build file [-o out] [--debug]", ""),
        "      freeze into a standalone executable".to_string(),
        help_row(&p, "gecko install wheel.whl [--to dir]", ""),
        "      unpack a wheel into site-packages".to_string(),
        String::new(),
        p.wrap("1;32", "options"),
        help_row(&p, "-h, --help", "print this help and exit"),
        help_row(&p, "-V, --version", "print the version and exit"),
    ];
    let offset = art.len().saturating_sub(text.len()) / 2;
    for i in 0..art.len().max(text.len() + offset) {
        let a = art.get(i).copied().unwrap_or("");
        let pad = " ".repeat(width.saturating_sub(a.chars().count()));
        let t = if i >= offset {
            text.get(i - offset).cloned().unwrap_or_default()
        } else {
            String::new()
        };
        let line = format!("{}{pad}{t}", p.wrap("32", a));
        println!("{}", line.trim_end());
    }
}

fn help_row(p: &freeze::Paint, left: &str, right: &str) -> String {
    if right.is_empty() {
        format!("  {}", p.wrap("1", left))
    } else {
        format!("  {}{right}", p.wrap_pad("1", left, 18))
    }
}

fn main() {
    if let Some(code) = embedded() {
        finish(run_code(&code));
        return;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("--help" | "-h") => print_help(),
        Some("--version" | "-V") => println!("gecko {VERSION}"),
        Some("build") => freeze::build(&args[1..]),
        Some("install") => install::install(&args[1..]),
        Some("-c") => match args.get(1) {
            Some(src) => execute(src),
            None => {
                eprintln!("gecko: -c needs an argument");
                exit(2);
            }
        },
        Some("-") => {
            use std::io::Read;
            let mut src = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut src) {
                eprintln!("gecko: cannot read stdin: {e}");
                exit(1);
            }
            execute(&src);
        }
        Some(path) if !path.starts_with('-') => match std::fs::read_to_string(path) {
            Ok(src) => execute_file(path, &src),
            Err(e) => {
                eprintln!("gecko: cannot read {path}: {e}");
                exit(1);
            }
        },
        Some(other) => {
            eprintln!("gecko: unknown argument '{other}'");
            eprintln!("{USAGE}");
            exit(2);
        }
    }
}

fn embedded() -> Option<bytecode::Code> {
    let path = std::env::current_exe().ok()?;
    bytecode::read_frozen(&path)
}

#[derive(Debug)]
struct Failure {
    output: String,
    message: String,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure {
            output: String::new(),
            message,
        }
    }
}

fn finish(result: Result<String, Failure>) {
    match result {
        Ok(output) => print!("{output}"),
        Err(f) => {
            print!("{}", f.output);
            eprintln!("{}", f.message);
            exit(1);
        }
    }
}

fn execute(src: &str) {
    finish(run_source(src));
}

fn execute_file(path: &str, src: &str) {
    let base = std::path::Path::new(path).parent().map(|p| {
        if p.as_os_str().is_empty() {
            std::path::PathBuf::from(".")
        } else {
            p.to_path_buf()
        }
    });
    finish(run_source_base(src, base));
}

fn run_source(src: &str) -> Result<String, Failure> {
    run_source_base(src, None)
}

fn run_source_base(src: &str, base: Option<std::path::PathBuf>) -> Result<String, Failure> {
    let module = parser::parse(src).map_err(|e| format!("SyntaxError: {}", e.message))?;
    let code = compiler::compile_with_base(&module, base)
        .map_err(|e| format!("CompileError: {}", e.message))?;
    run_code(&code)
}

fn run_code(code: &bytecode::Code) -> Result<String, Failure> {
    let mut vm = runtime::Vm::new();
    vm.set_sandbox_hook(sandbox::hook);
    vm.enable_actors();
    let run = vm.run(code);
    if run.error {
        let message = if run.message.is_empty() {
            "RuntimeError".into()
        } else {
            run.message
        };
        return Err(Failure {
            output: run.output,
            message,
        });
    }
    Ok(run.output)
}

#[cfg(test)]
mod tests {
    use super::run_source;

    #[test]
    fn hello_world() {
        assert_eq!(
            run_source("print(\"hello world\")\n").unwrap(),
            "hello world\n"
        );
    }

    #[test]
    fn arithmetic() {
        assert_eq!(run_source("print(1 + 2 * 3)\n").unwrap(), "7\n");
    }

    #[test]
    fn assignment_then_use() {
        assert_eq!(run_source("x = 41\nprint(x + 1)\n").unwrap(), "42\n");
    }

    #[test]
    fn syntax_error_reports() {
        assert!(run_source("print(\n").is_err());
    }

    #[test]
    fn if_else_branches() {
        let src = "if 1 < 2:\n    print(\"yes\")\nelse:\n    print(\"no\")\n";
        assert_eq!(run_source(src).unwrap(), "yes\n");
    }

    #[test]
    fn while_loop_sums() {
        let src =
            "i = 1\ntotal = 0\nwhile i <= 5:\n    total = total + i\n    i = i + 1\nprint(total)\n";
        assert_eq!(run_source(src).unwrap(), "15\n");
    }

    #[test]
    fn short_circuit_and_or() {
        assert_eq!(
            run_source("print(1 and 2)\nprint(0 or 5)\n").unwrap(),
            "2\n5\n"
        );
    }

    #[test]
    fn ternary_expression() {
        assert_eq!(
            run_source("x = 5\nprint(\"big\" if x > 3 else \"small\")\n").unwrap(),
            "big\n"
        );
    }

    #[test]
    fn lambda_with_closure() {
        let src =
            "def outer():\n    n = 100\n    f = lambda k: k + n\n    return f(5)\nprint(outer())\n";
        assert_eq!(run_source(src).unwrap(), "105\n");
    }

    #[test]
    fn global_statement() {
        let src = "c = 0\ndef bump():\n    global c\n    c = c + 1\nbump()\nbump()\nprint(c)\n";
        assert_eq!(run_source(src).unwrap(), "2\n");
    }

    #[test]
    fn assert_statement() {
        let src = "try:\n    assert 1 > 2, \"nope\"\nexcept AssertionError as e:\n    print(e)\n";
        assert_eq!(run_source(src).unwrap(), "nope\n");
    }

    #[test]
    fn chained_assignment_and_comparison() {
        let src = "a = b = 3\nprint(a, b)\nprint(0 < a < 10)\nprint(1 < 2 < 3 < 4)\nprint(1 < 2 < 9 < 4)\n";
        assert_eq!(run_source(src).unwrap(), "3 3\nTrue\nTrue\nFalse\n");
    }

    #[test]
    fn del_statement() {
        let src = "d = {\"x\": 1, \"y\": 2}\ndel d[\"x\"]\nprint(d)\n";
        assert_eq!(run_source(src).unwrap(), "{'y': 2}\n");
    }

    #[test]
    fn del_attribute() {
        let src = "class P:\n    def __init__(self):\n        self.v = 1\np = P()\ndel p.v\ntry:\n    print(p.v)\nexcept AttributeError:\n    print(\"gone\")\n";
        assert_eq!(run_source(src).unwrap(), "gone\n");
    }

    #[test]
    fn with_statement() {
        let src = "class C:\n    def __enter__(self):\n        return 7\n    def __exit__(self, a, b, c):\n        print(\"exit\")\nwith C() as v:\n    print(v)\n";
        assert_eq!(run_source(src).unwrap(), "7\nexit\n");
    }

    #[test]
    fn with_suppresses_exception() {
        let src = "class Q:\n    def __enter__(self):\n        return self\n    def __exit__(self, t, v, tb):\n        return True\nwith Q():\n    raise ValueError(\"x\")\nprint(\"survived\")\n";
        assert_eq!(run_source(src).unwrap(), "survived\n");
    }

    #[test]
    fn actor_call_replies() {
        let src = "from gecko import actor\n\ndef handle(state, message):\n    reply = message[1]\n    reply.send(state + message[0])\n    return state + message[0]\n\ndef build(reply):\n    return [7, reply]\n\ncalc = actor.spawn(0, handle)\nprint(calc.call(build, 1000))\n";
        assert_eq!(run_source(src).unwrap(), "7\n");
    }

    #[test]
    fn actor_counter_casts_then_calls() {
        let src = "from gecko import actor\n\ndef handle(state, message):\n    if message[0] == \"add\":\n        return state + message[1]\n    message[1].send(state)\n    return state\n\ndef get(reply):\n    return [\"get\", reply]\n\ncounter = actor.spawn(0, handle)\ncounter.send([\"add\", 5])\ncounter.send([\"add\", 3])\nprint(counter.call(get, 1000))\n";
        assert_eq!(run_source(src).unwrap(), "8\n");
    }

    #[test]
    fn actor_call_reraises_a_handler_failure() {
        let src = "from gecko import actor\n\ndef handle(state, message):\n    message[2].send(message[0] / message[1])\n    return state\n\ndef divide(a, b):\n    def build(reply):\n        return [a, b, reply]\n    return build\n\ncalc = actor.spawn(0, handle)\ntry:\n    print(calc.call(divide(10, 0), 1000))\nexcept RuntimeError as e:\n    print(\"caught\")\n";
        assert_eq!(run_source(src).unwrap(), "caught\n");
    }

    #[test]
    fn actor_stop_ends_the_actor() {
        let src = "from gecko import actor\n\ndef handle(state, message):\n    if message[0] == \"stop\":\n        return actor.stop()\n    message[1].send(state + 1)\n    return state + 1\n\na = actor.spawn(0, handle)\n\ndef ping(reply):\n    return [\"ping\", reply]\n\nprint(a.call(ping, 1000))\na.send([\"stop\", 0])\ntry:\n    a.call(ping, 200)\n    print(\"alive\")\nexcept Exception as e:\n    print(\"stopped\")\n";
        assert_eq!(run_source(src).unwrap(), "1\nstopped\n");
    }

    #[test]
    fn floats_print_shortest_repr() {
        let src = "print(100.0)\nprint(2.5)\nprint(340.0 / 9.0)\nprint(0.0001)\nprint(1e16)\n";
        assert_eq!(
            run_source(src).unwrap(),
            "100.0\n2.5\n37.77777777777778\n0.0001\n1e+16\n"
        );
    }

    #[test]
    fn unary_negation() {
        assert_eq!(run_source("print(-3 + 1)\n").unwrap(), "-2\n");
    }

    #[test]
    fn functions_call_and_return() {
        let src = "def add(a, b):\n    return a + b\nprint(add(2, 40))\n";
        assert_eq!(run_source(src).unwrap(), "42\n");
    }

    #[test]
    fn functions_recurse() {
        let src = "def fib(n):\n    if n < 2:\n        return n\n    return fib(n - 1) + fib(n - 2)\nprint(fib(10))\n";
        assert_eq!(run_source(src).unwrap(), "55\n");
    }

    #[test]
    fn function_without_return_gives_none() {
        let src = "def f():\n    pass\nprint(f())\n";
        assert_eq!(run_source(src).unwrap(), "None\n");
    }

    #[test]
    fn function_locals_shadow_globals() {
        let src = "x = 1\ndef f():\n    x = 2\n    return x\nprint(f(), x)\n";
        assert_eq!(run_source(src).unwrap(), "2 1\n");
    }

    #[test]
    fn list_literals_and_methods() {
        let src = "l = [1, 2]\nl.append(3)\nprint(l, len(l), l[0], l[-1])\nprint(l.pop(), l)\n";
        assert_eq!(run_source(src).unwrap(), "[1, 2, 3] 3 1 3\n3 [1, 2]\n");
    }

    #[test]
    fn list_subscript_assignment() {
        let src = "l = [1, 2, 3]\nl[1] = 20\nprint(l)\n";
        assert_eq!(run_source(src).unwrap(), "[1, 20, 3]\n");
    }

    #[test]
    fn dict_literals_and_methods() {
        let src = "d = {\"a\": 1}\nd[\"b\"] = 2\nprint(d, len(d), d[\"b\"], d.get(\"z\", 9))\nprint(d.keys(), d.values())\n";
        assert_eq!(
            run_source(src).unwrap(),
            "{'a': 1, 'b': 2} 2 2 9\n['a', 'b'] [1, 2]\n"
        );
    }

    #[test]
    fn for_over_range_list_str_dict() {
        let src = "total = 0\nfor i in range(5):\n    total += i\nfor x in [10, 20]:\n    total += x\nfor c in \"ab\":\n    print(c)\nd = {\"k\": 1}\nfor k in d:\n    print(k)\nprint(total)\n";
        assert_eq!(run_source(src).unwrap(), "a\nb\nk\n40\n");
    }

    #[test]
    fn membership_tests() {
        let src = "print(2 in [1, 2], 5 in [1, 2], \"a\" in {\"a\": 1}, \"ell\" in \"hello\", 4 in range(0, 10, 2))\n";
        assert_eq!(run_source(src).unwrap(), "True False True True True\n");
    }

    #[test]
    fn mod_and_floordiv_follow_python() {
        let src = "print(17 % 5, -17 % 5, 17 // 5, -17 // 5, 7.5 % 2, -7.5 // 2)\n";
        assert_eq!(run_source(src).unwrap(), "2 3 3 -4 1.5 -4.0\n");
    }

    #[test]
    fn str_concat_and_ordering() {
        let src = "print(\"con\" + \"cat\", \"a\" < \"b\", \"abc\" <= \"ab\")\n";
        assert_eq!(run_source(src).unwrap(), "concat True False\n");
    }

    #[test]
    fn nested_containers_print_reprs() {
        let src = "print([1, [\"s\", {\"k\": None}]])\n";
        assert_eq!(run_source(src).unwrap(), "[1, ['s', {'k': None}]]\n");
    }

    #[test]
    fn deep_equality() {
        let src = "print([1, {\"a\": 2}] == [1, {\"a\": 2}], [1] == [2])\n";
        assert_eq!(run_source(src).unwrap(), "True False\n");
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
    fn extended_arg_on_a_jump_target() {
        let mut src = String::from("x = 0\nfor j in range(3):\n");
        for i in 0..200 {
            src.push_str(&format!("    x = x + {i} - {i}\n"));
        }
        src.push_str("    x = x + 1\nprint(x)\n");
        assert_eq!(run_source(&src).unwrap(), "3\n");
    }

    #[test]
    fn parameter_defaults_fill_missing_arguments() {
        let src = "def greet(name, greeting=\"hi\"):\n    return greeting + \" \" + name\nprint(greet(\"a\"))\nprint(greet(\"b\", \"yo\"))\n";
        assert_eq!(run_source(src).unwrap(), "hi a\nyo b\n");
    }

    #[test]
    fn parameter_defaults_are_evaluated_once_at_definition() {
        let src = "n = 10\ndef f(x=n):\n    return x\nn = 20\nprint(f())\nprint(f(1))\n";
        assert_eq!(run_source(src).unwrap(), "10\n1\n");
    }

    #[test]
    fn varargs_collect_extra_positionals() {
        let src =
            "def g(a, b, *rest):\n    return (a, b, rest)\nprint(g(1, 2, 3, 4))\nprint(g(1, 2))\n";
        assert_eq!(run_source(src).unwrap(), "(1, 2, (3, 4))\n(1, 2, ())\n");
    }

    #[test]
    fn kwargs_collect_extra_keywords() {
        let src = "def h(a, **k):\n    return (a, k)\nprint(h(1, x=2, y=3))\n";
        assert_eq!(run_source(src).unwrap(), "(1, {'x': 2, 'y': 3})\n");
    }

    #[test]
    fn keyword_arguments_bind_by_name() {
        let src = "def f(a, b):\n    return a - b\nprint(f(b=1, a=10))\nprint(f(10, b=3))\n";
        assert_eq!(run_source(src).unwrap(), "9\n7\n");
    }

    #[test]
    fn full_signature_binds_correctly() {
        let src = "def f(a, b=10, *args, **kw):\n    return (a, b, args, kw)\nprint(f(1))\nprint(f(1, 2, 3, 4, p=5))\n";
        assert_eq!(
            run_source(src).unwrap(),
            "(1, 10, (), {})\n(1, 2, (3, 4), {'p': 5})\n"
        );
    }

    #[test]
    fn call_site_spreads_expand() {
        let src = "def g(a, b, *rest):\n    return (a, b, rest)\nxs = [2, 3, 4]\nprint(g(1, *xs))\nd = {'y': 9}\ndef h(**k):\n    return k\nprint(h(x=1, **d))\n";
        assert_eq!(
            run_source(src).unwrap(),
            "(1, 2, (3, 4))\n{'x': 1, 'y': 9}\n"
        );
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
    fn non_default_after_default_is_rejected() {
        let f = run_source("def f(a=1, b):\n    return a\n").unwrap_err();
        assert!(
            f.message
                .contains("non-default argument follows default argument")
        );
    }

    #[test]
    fn deep_and_mutual_recursion_reuse_frames_correctly() {
        let src = "def fib(n):\n    if n < 2:\n        return n\n    return fib(n - 1) + fib(n - 2)\ndef is_even(n):\n    if n == 0:\n        return True\n    return is_odd(n - 1)\ndef is_odd(n):\n    if n == 0:\n        return False\n    return is_even(n - 1)\nprint(fib(20), is_even(200), is_odd(101))\n";
        assert_eq!(run_source(src).unwrap(), "6765 True True\n");
    }

    #[test]
    fn polymorphic_method_dispatch_picks_the_right_override() {
        let src = "class Animal:\n    def speak(self):\n        return \"...\"\nclass Dog(Animal):\n    def speak(self):\n        return \"woof\"\nclass Cat(Animal):\n    def speak(self):\n        return \"meow\"\ndef go(a):\n    return a.speak()\nd = Dog()\nc = Cat()\nprint(go(d), go(c), go(d), go(c))\n";
        assert_eq!(run_source(src).unwrap(), "woof meow woof meow\n");
    }

    #[test]
    fn reassigning_a_method_busts_the_cache() {
        let src = "class C:\n    def f(self):\n        return 1\ndef g(self):\n    return 2\nc = C()\nprint(c.f())\nC.f = g\nprint(c.f())\n";
        assert_eq!(run_source(src).unwrap(), "1\n2\n");
    }

    #[test]
    fn a_data_attribute_shadows_a_method_at_a_call_site() {
        let src = "class C:\n    def f(self):\n        return 1\ndef plain(v):\n    return v + 100\nc = C()\nprint(c.f())\nc.f = plain\nprint(c.f(5))\n";
        assert_eq!(run_source(src).unwrap(), "1\n105\n");
    }

    #[test]
    fn inherited_methods_resolve_through_the_cache() {
        let src = "class Base:\n    def m(self):\n        return 42\nclass Mid(Base):\n    pass\nclass Leaf(Mid):\n    pass\nprint(Leaf().m(), Leaf().m())\n";
        assert_eq!(run_source(src).unwrap(), "42 42\n");
    }

    #[test]
    fn a_cached_global_reads_its_live_value() {
        let src = "def get():\n    return x\nx = 1\nprint(get())\nx = 2\nprint(get())\n";
        assert_eq!(run_source(src).unwrap(), "1\n2\n");
    }

    #[test]
    fn a_cached_builtin_busts_when_a_global_shadows_it() {
        let src = "def uselen(a):\n    return len(a)\nprint(uselen([1, 2, 3]))\nlen = 99\ntry:\n    uselen([1, 2, 3])\nexcept TypeError:\n    print(\"shadowed\")\n";
        assert_eq!(run_source(src).unwrap(), "3\nshadowed\n");
    }

    #[test]
    fn polymorphic_attribute_site_reads_the_right_slot() {
        let src = "class A:\n    def __init__(self):\n        self.x = 1\n        self.y = 2\nclass B:\n    def __init__(self):\n        self.y = 10\n        self.x = 20\ndef getx(o):\n    return o.x\na = A()\nb = B()\nprint(getx(a), getx(b), getx(a), getx(b))\n";
        assert_eq!(run_source(src).unwrap(), "1 20 1 20\n");
    }

    #[test]
    fn adding_an_attribute_busts_a_cached_site() {
        let src = "class G:\n    pass\ndef read(o):\n    return o.a\ng = G()\ng.a = 1\nprint(read(g))\ng.b = 2\nprint(read(g), g.b)\n";
        assert_eq!(run_source(src).unwrap(), "1\n1 2\n");
    }

    #[test]
    fn instance_attributes_get_and_set() {
        let src = "class P:\n    def __init__(self, x, y):\n        self.x = x\n        self.y = y\np = P(3, 4)\nprint(p.x, p.y)\np.x = 100\nprint(p.x, p.y)\np.z = 9\nprint(p.z)\n";
        assert_eq!(run_source(src).unwrap(), "3 4\n100 4\n9\n");
    }

    #[test]
    fn instances_with_different_attribute_orders_stay_correct() {
        let src = "class Bag:\n    pass\na = Bag()\na.p = 1\na.q = 2\nb = Bag()\nb.q = 20\nb.p = 10\nprint(a.p, a.q, b.p, b.q)\n";
        assert_eq!(run_source(src).unwrap(), "1 2 10 20\n");
    }

    #[test]
    fn instance_slots_survive_gc() {
        let src = "class N:\n    def __init__(self, v):\n        self.v = v\n        self.next = None\nhead = None\nfor i in range(20000):\n    n = N(i)\n    n.next = head\n    head = n\ns = 0\ncur = head\nwhile cur is not None:\n    s = s + cur.v\n    cur = cur.next\nprint(s)\n";
        assert_eq!(run_source(src).unwrap(), "199990000\n");
    }

    #[test]
    fn subclass_init_sets_attributes() {
        let src = "class A:\n    def __init__(self, name):\n        self.name = name\nclass B(A):\n    def greet(self):\n        return self.name\nb = B(\"x\")\nprint(b.name, b.greet())\n";
        assert_eq!(run_source(src).unwrap(), "x x\n");
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
    fn a_global_shadows_a_builtin() {
        let src = "print(len([1, 2]))\nlen = 42\nprint(len)\n";
        assert_eq!(run_source(src).unwrap(), "2\n42\n");
    }

    #[test]
    fn large_dict_lookups_stay_correct() {
        let src = "d = {}\nfor i in range(50):\n    d[i] = i * i\nprint(d[0], d[25], d[49], len(d))\nprint(25 in d, 999 in d)\n";
        assert_eq!(run_source(src).unwrap(), "0 625 2401 50\nTrue False\n");
    }

    #[test]
    fn dict_int_and_float_keys_collide_like_python() {
        let src = "d = {}\nfor i in range(12):\n    d[i] = i\nprint(d[5.0])\nd[5.0] = 100\nprint(d[5], len(d))\n";
        assert_eq!(run_source(src).unwrap(), "5\n100 12\n");
    }

    #[test]
    fn large_dict_preserves_insertion_order() {
        let src = "d = {}\nfor i in range(15):\n    d[i * 3] = i\nout = []\nfor k in d:\n    out.append(k)\nprint(out[0], out[7], out[14])\n";
        assert_eq!(run_source(src).unwrap(), "0 21 42\n");
    }

    #[test]
    fn tuple_keys_work_through_the_index() {
        let src = "d = {}\nfor i in range(10):\n    d[(i, i + 1)] = i\nprint(d[(3, 4)], (7, 8) in d, (7, 9) in d)\n";
        assert_eq!(run_source(src).unwrap(), "3 True False\n");
    }

    #[test]
    fn class_with_many_methods_resolves() {
        let mut src = String::from("class C:\n");
        for i in 0..12 {
            src.push_str(&format!("    def m{i}(self):\n        return {i}\n"));
        }
        src.push_str("c = C()\nprint(c.m0(), c.m6(), c.m11())\n");
        assert_eq!(run_source(&src).unwrap(), "0 6 11\n");
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
            ("2000000000 + 2000000000", "4000000000.0"),
            ("1000000 * 1000000", "1000000000000.0"),
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
    fn string_constants_fold() {
        assert_eq!(
            run_source("print(\"a\" + \"b\" + \"c\")\n").unwrap(),
            "abc\n"
        );
    }

    #[test]
    fn constant_division_by_zero_stays_a_runtime_error() {
        let f = run_source("print(1 // 0)\n").unwrap_err();
        assert!(f.message.contains("ZeroDivisionError"), "{}", f.message);
    }

    #[test]
    fn is_and_is_not_compare_identity() {
        let src = "x = None\nprint(x is None)\nprint(x is not None)\ny = 5\nprint(y is not None)\n";
        assert_eq!(run_source(src).unwrap(), "True\nFalse\nTrue\n");
    }

    #[test]
    fn a_missing_import_raises_at_runtime() {
        let f = run_source("import no_such_module_zzz\n").unwrap_err();
        assert!(f.message.contains("ImportError"));
        assert!(f.message.contains("no_such_module_zzz"));
    }

    #[test]
    fn a_subclassed_exception_is_catchable() {
        let src = "class MyError(Exception):\n    pass\ntry:\n    raise MyError(\"boom\")\nexcept MyError:\n    print(\"caught\")\n";
        assert_eq!(run_source(src).unwrap(), "caught\n");
    }

    #[test]
    fn sandbox_runs_code_and_returns_output() {
        let src = "from gecko import sandbox\nout = sandbox.run(\"print(2 + 3)\\nprint(\\\"hi\\\")\")\nprint(out)\n";
        assert_eq!(run_source(src).unwrap(), "5\nhi\n\n");
    }

    #[test]
    fn sandbox_step_limit_stops_a_loop() {
        let src = "from gecko import sandbox\ntry:\n    sandbox.run(\"while True:\\n    pass\", 5000)\nexcept SandboxError as e:\n    print(\"stopped\")\n";
        assert_eq!(run_source(src).unwrap(), "stopped\n");
    }

    #[test]
    fn sandbox_time_limit_stops_a_loop() {
        let src = "from gecko import sandbox\ntry:\n    sandbox.run(\"while True:\\n    pass\", 0, 0, 20)\nexcept SandboxError as e:\n    print(\"stopped\")\n";
        assert_eq!(run_source(src).unwrap(), "stopped\n");
    }

    #[test]
    fn sandbox_error_is_catchable_and_isolated() {
        let src = "from gecko import sandbox\nx = 1\ntry:\n    sandbox.run(\"1 / 0\")\nexcept SandboxError:\n    print(\"caught\")\nprint(x)\n";
        assert_eq!(run_source(src).unwrap(), "caught\n1\n");
    }

    #[test]
    fn sandboxed_code_cannot_import_files() {
        let src = "from gecko import sandbox\ntry:\n    sandbox.run(\"import os\")\nexcept SandboxError:\n    print(\"blocked\")\n";
        assert_eq!(run_source(src).unwrap(), "blocked\n");
    }

    #[test]
    fn gecko_module_also_reaches_sandbox() {
        let src = "import gecko\nprint(gecko.sandbox.run(\"print(1 + 1)\"))\n";
        assert_eq!(run_source(src).unwrap(), "2\n\n");
    }

    #[test]
    fn decorator_wraps_a_function() {
        let src = "def twice(f):\n    def w(x):\n        return f(f(x))\n    return w\n@twice\ndef inc(n):\n    return n + 1\nprint(inc(10))\n";
        assert_eq!(run_source(src).unwrap(), "12\n");
    }

    #[test]
    fn decorator_with_arguments() {
        let src = "def tag(label):\n    def deco(f):\n        def w(x):\n            return label + \":\" + f(x)\n        return w\n    return deco\n@tag(\"r\")\ndef shout(s):\n    return s\nprint(shout(\"hi\"))\n";
        assert_eq!(run_source(src).unwrap(), "r:hi\n");
    }

    #[test]
    fn stacked_decorators_apply_bottom_up() {
        let src = "def a(f):\n    def w(x):\n        return \"a(\" + f(x) + \")\"\n    return w\ndef b(f):\n    def w(x):\n        return \"b(\" + f(x) + \")\"\n    return w\n@a\n@b\ndef base(x):\n    return x\nprint(base(\"X\"))\n";
        assert_eq!(run_source(src).unwrap(), "a(b(X))\n");
    }

    #[test]
    fn class_decorator_runs() {
        let src = "seen = []\ndef register(cls):\n    seen.append(cls)\n    return cls\n@register\nclass W:\n    def __init__(self):\n        self.name = \"w\"\nprint(W().name, len(seen))\n";
        assert_eq!(run_source(src).unwrap(), "w 1\n");
    }

    #[test]
    fn classes_init_attributes_and_methods() {
        let src = "class Point:\n    def __init__(self, x, y):\n        self.x = x\n        self.y = y\n    def norm2(self):\n        return self.x * self.x + self.y * self.y\np = Point(3, 4)\nprint(p.x, p.y, p.norm2())\n";
        assert_eq!(run_source(src).unwrap(), "3 4 25\n");
    }

    #[test]
    fn classes_inherit_and_override() {
        let src = "class Animal:\n    def __init__(self, name):\n        self.name = name\n    def speak(self):\n        return \"...\"\n    def describe(self):\n        return self.name + \": \" + self.speak()\nclass Dog(Animal):\n    def speak(self):\n        return \"woof\"\nprint(Dog(\"Rex\").describe())\nprint(Animal(\"Thing\").describe())\n";
        assert_eq!(run_source(src).unwrap(), "Rex: woof\nThing: ...\n");
    }

    #[test]
    fn class_attributes_and_instance_shadowing() {
        let src = "class Box:\n    kind = \"box\"\n    def set(self, v):\n        self.kind = v\nb = Box()\nprint(b.kind, Box.kind)\nb.set(\"crate\")\nprint(b.kind, Box.kind)\n";
        assert_eq!(run_source(src).unwrap(), "box box\ncrate box\n");
    }

    #[test]
    fn class_body_names_are_invisible_to_methods() {
        let src = "FACTOR = 100\nclass W:\n    FACTOR = 3\n    def get(self):\n        return FACTOR\nprint(W().get(), W.FACTOR)\n";
        assert_eq!(run_source(src).unwrap(), "100 3\n");
    }

    #[test]
    fn bound_methods_are_values() {
        let src = "class Counter:\n    def __init__(self):\n        self.n = 0\n    def inc(self):\n        self.n += 1\n        return self.n\nc = Counter()\nm = c.inc\nprint(m(), m(), c.n)\n";
        assert_eq!(run_source(src).unwrap(), "1 2 2\n");
    }

    #[test]
    fn methods_can_return_closures_over_self() {
        let src = "class Adder:\n    def __init__(self, base):\n        self.base = base\n    def make(self):\n        b = self.base\n        def add(x):\n            return b + x\n        return add\nf = Adder(10).make()\nprint(f(5), f(7))\n";
        assert_eq!(run_source(src).unwrap(), "15 17\n");
    }

    #[test]
    fn missing_attribute_raises() {
        let src = "class E:\n    pass\nE().nope\n";
        let f = run_source(src).unwrap_err();
        assert!(f.message.contains("AttributeError"), "{}", f.message);
        assert!(f.message.contains("nope"), "{}", f.message);
    }

    #[test]
    fn instances_survive_collection() {
        let src = "class Node:\n    def __init__(self, v):\n        self.v = v\nkeep = Node(7)\ni = 0\nwhile i < 20000:\n    tmp = Node(i)\n    i += 1\nprint(keep.v)\n";
        assert_eq!(run_source(src).unwrap(), "7\n");
    }

    #[test]
    fn exceptions_catch_by_type() {
        let src = "try:\n    1 / 0\nexcept ZeroDivisionError as e:\n    print(\"caught:\", e)\n";
        assert_eq!(run_source(src).unwrap(), "caught: division by zero\n");
    }

    #[test]
    fn exceptions_pick_the_first_matching_handler() {
        let src = "try:\n    {}[\"k\"]\nexcept ValueError:\n    print(\"wrong\")\nexcept KeyError:\n    print(\"right\")\nexcept Exception:\n    print(\"late\")\n";
        assert_eq!(run_source(src).unwrap(), "right\n");
    }

    #[test]
    fn raise_and_catch_with_else() {
        let src = "def risky(n):\n    if n > 2:\n        raise ValueError(\"too big\")\n    return n\ntry:\n    print(risky(1))\nexcept ValueError:\n    print(\"unseen\")\nelse:\n    print(\"else\")\ntry:\n    risky(9)\nexcept ValueError as e:\n    print(e)\nelse:\n    print(\"unseen\")\n";
        assert_eq!(run_source(src).unwrap(), "1\nelse\ntoo big\n");
    }

    #[test]
    fn finally_runs_on_both_paths() {
        let src = "try:\n    print(\"ok\")\nfinally:\n    print(\"cleanup\")\ntry:\n    try:\n        raise TypeError(\"x\")\n    finally:\n        print(\"inner cleanup\")\nexcept TypeError:\n    print(\"outer\")\n";
        assert_eq!(
            run_source(src).unwrap(),
            "ok\ncleanup\ninner cleanup\nouter\n"
        );
    }

    #[test]
    fn exceptions_propagate_through_calls() {
        let src = "def f():\n    raise IndexError(\"deep\")\ndef g():\n    return f()\ntry:\n    g()\nexcept IndexError as e:\n    print(e)\n";
        assert_eq!(run_source(src).unwrap(), "deep\n");
    }

    #[test]
    fn tuple_of_types_matches_any() {
        let src = "try:\n    raise RuntimeError(\"boom\")\nexcept (ValueError, RuntimeError) as e:\n    print(e)\n";
        assert_eq!(run_source(src).unwrap(), "boom\n");
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
    fn try_inside_a_loop_unwinds_to_the_iterator() {
        let src = "kept = []\nfor i in range(5):\n    try:\n        if i % 2 == 0:\n            raise ValueError(\"skip\")\n        kept.append(i)\n    except ValueError:\n        pass\nprint(kept)\n";
        assert_eq!(run_source(src).unwrap(), "[1, 3]\n");
    }

    #[test]
    fn exception_reprs_follow_python() {
        let src = "e = ValueError(\"kept\")\nprint(e, [e], ValueError)\nprint(TypeError())\nprint([TypeError()])\n";
        assert_eq!(
            run_source(src).unwrap(),
            "kept [ValueError('kept')] <class 'ValueError'>\n\n[TypeError()]\n"
        );
    }

    #[test]
    fn return_through_finally_runs_it() {
        let src = "def f():\n    try:\n        return 1\n    finally:\n        print(\"x\")\nprint(f())\n";
        assert_eq!(run_source(src).unwrap(), "x\n1\n");
    }

    #[test]
    fn fstrings() {
        let src = "name = \"gecko\"\nn = 3\nprint(f\"{name} has {n + 1} legs\")\nprint(f\"{name!r}\")\nprint(f\"{{esc}} {name}\")\n";
        assert_eq!(
            run_source(src).unwrap(),
            "gecko has 4 legs\n'gecko'\n{esc} gecko\n"
        );
    }

    #[test]
    fn generators() {
        let src = "def count(n):\n    i = 0\n    while i < n:\n        yield i\n        i = i + 1\nprint([x for x in count(4)])\ng = count(2)\nprint(next(g))\nprint(next(g))\ntry:\n    next(g)\nexcept StopIteration:\n    print(\"done\")\n";
        assert_eq!(run_source(src).unwrap(), "[0, 1, 2, 3]\n0\n1\ndone\n");
    }

    #[test]
    fn match_statement() {
        let src = "def d(x):\n    match x:\n        case 0:\n            return \"zero\"\n        case 1 | 2 | 3:\n            return \"small\"\n        case n if n > 100:\n            return \"huge\"\n        case n:\n            return \"other\"\nprint(d(0), d(2), d(200), d(50))\nmatch \"hi\":\n    case \"hi\" as g:\n        print(g)\n";
        assert_eq!(run_source(src).unwrap(), "zero small huge other\nhi\n");
    }

    #[test]
    fn walrus() {
        let src = "if (n := len([1, 2, 3])) > 2:\n    print(n)\ndef f(x):\n    if (d := x * 2) > 5:\n        return d\n    return 0\nprint(f(4))\n";
        assert_eq!(run_source(src).unwrap(), "3\n8\n");
    }

    #[test]
    fn type_builtin() {
        let src = "print(type(5) is int)\nprint(type(\"a\") is str)\nprint(type([]) is list)\n";
        assert_eq!(run_source(src).unwrap(), "True\nTrue\nTrue\n");
    }

    #[test]
    fn with_return_runs_exit() {
        let src = "class C:\n    def __enter__(self):\n        return 9\n    def __exit__(self, t, v, tb):\n        print(\"exit\")\ndef f():\n    with C() as v:\n        return v\nprint(f())\n";
        assert_eq!(run_source(src).unwrap(), "exit\n9\n");
    }

    #[test]
    fn with_exit_gets_exception_type() {
        let src = "class C:\n    def __enter__(self):\n        return self\n    def __exit__(self, t, v, tb):\n        print(t is ValueError)\n        return True\nwith C():\n    raise ValueError(\"x\")\nprint(\"ok\")\n";
        assert_eq!(run_source(src).unwrap(), "True\nok\n");
    }

    #[test]
    fn tuples_pack_unpack_and_compare() {
        let src = "t = (1, \"two\")\na, b = t\nb, a = a, b\nx, (y, z) = 1, (2, 3)\nprint(t, a, b, x, y, z)\nprint(t == (1, \"two\"), (1,) + (2, 3), len(()), 2 in (1, 2))\n";
        assert_eq!(
            run_source(src).unwrap(),
            "(1, 'two') two 1 1 2 3\nTrue (1, 2, 3) 0 True\n"
        );
    }

    #[test]
    fn unpack_arity_mismatch_is_a_value_error() {
        let f = run_source("a, b = [1, 2, 3]\n").unwrap_err();
        assert!(f.message.contains("too many values to unpack"));
        let f = run_source("a, b, c = (1, 2)\n").unwrap_err();
        assert!(f.message.contains("not enough values"));
    }

    #[test]
    fn dict_items_yields_tuples() {
        let src =
            "d = {\"a\": 1, \"b\": 2}\nfor k, v in d.items():\n    print(k, v)\nprint(d.items())\n";
        assert_eq!(run_source(src).unwrap(), "a 1\nb 2\n[('a', 1), ('b', 2)]\n");
    }

    #[test]
    fn break_skips_else_and_continue_skips_body() {
        let src = "for i in range(9):\n    if i == 2:\n        break\nelse:\n    print(\"unseen\")\nprint(i)\nout = []\nfor j in range(5):\n    if j % 2 == 0:\n        continue\n    out.append(j)\nprint(out)\nk = 0\nwhile True:\n    k += 1\n    if k == 3:\n        break\nprint(k)\n";
        assert_eq!(run_source(src).unwrap(), "2\n[1, 3]\n3\n");
    }

    #[test]
    fn nested_break_binds_the_inner_loop() {
        let src = "hits = []\nfor a in range(3):\n    for b in range(9):\n        if b > a:\n            break\n        hits.append((a, b))\nprint(hits)\n";
        assert_eq!(
            run_source(src).unwrap(),
            "[(0, 0), (1, 0), (1, 1), (2, 0), (2, 1), (2, 2)]\n"
        );
    }

    #[test]
    fn comprehensions_build_lists_and_dicts() {
        let src = "print([x * x for x in range(5)])\nprint([x for x in range(10) if x % 2 == 0 if x > 3])\nprint([(a, b) for a in range(2) for b in \"xy\"])\nprint({w: len(w) for w in [\"hi\", \"there\"]})\n";
        assert_eq!(
            run_source(src).unwrap(),
            "[0, 1, 4, 9, 16]\n[4, 6, 8]\n[(0, 'x'), (0, 'y'), (1, 'x'), (1, 'y')]\n{'hi': 2, 'there': 5}\n"
        );
    }

    #[test]
    fn comprehensions_close_over_enclosing_scopes() {
        let src = "def scaled(factor):\n    return [n * factor for n in range(4)]\nprint(scaled(3))\nprint([[y + 1 for y in range(x)] for x in range(3)])\nn = 9\nprint([n for _ in range(2)])\n";
        assert_eq!(
            run_source(src).unwrap(),
            "[0, 3, 6, 9]\n[[], [1], [1, 2]]\n[9, 9]\n"
        );
    }

    #[test]
    fn comprehension_variables_stay_local() {
        let src = "x = \"kept\"\nl = [x for x in range(3)]\nprint(x, l)\n";
        assert_eq!(run_source(src).unwrap(), "kept [0, 1, 2]\n");
    }

    #[test]
    fn unicode_strings_index_by_code_point() {
        let src = "s = \"h\u{e9}llo\"\nprint(len(s), s[1], s[-1])\nfor c in \"\u{e9}\u{fc}\":\n    print(c)\n";
        assert_eq!(run_source(src).unwrap(), "5 \u{e9} o\n\u{e9}\n\u{fc}\n");
    }

    #[test]
    fn container_edge_cases() {
        let src = "l = [1, 2, 3]\nprint(l.pop(0), l)\nd = {}\nprint(d.get(\"x\"))\nr = range(10, 0, -2)\nprint(len(r), r[0], r[4], 8 in r, 7 in r)\n";
        assert_eq!(
            run_source(src).unwrap(),
            "1 [2, 3]\nNone\n5 10 2 True False\n"
        );
    }

    #[test]
    fn closures_capture_and_update() {
        let src = "def counter():\n    n = 0\n    def inc():\n        nonlocal n\n        n += 1\n        return n\n    return inc\nc = counter()\nd = counter()\nprint(c(), c(), d(), c())\n";
        assert_eq!(run_source(src).unwrap(), "1 2 1 3\n");
    }

    #[test]
    fn closures_share_one_cell() {
        let src = "def pair():\n    v = 0\n    def set5():\n        nonlocal v\n        v = 5\n    def get():\n        return v\n    set5()\n    return get()\nprint(pair())\n";
        assert_eq!(run_source(src).unwrap(), "5\n");
    }

    #[test]
    fn closures_capture_transitively() {
        let src = "def a():\n    x = 7\n    def b():\n        def inner():\n            return x\n        return inner()\n    return b()\nprint(a())\n";
        assert_eq!(run_source(src).unwrap(), "7\n");
    }

    #[test]
    fn loop_closures_share_the_variable() {
        let src = "def late():\n    fs = []\n    for i in range(3):\n        def f():\n            return i\n        fs.append(f)\n    return fs\nfs = late()\nprint(fs[0](), fs[1](), fs[2]())\n";
        assert_eq!(run_source(src).unwrap(), "2 2 2\n");
    }

    #[test]
    fn reading_an_unset_cell_fails() {
        let src = "def outer():\n    def get():\n        return v\n    r = get()\n    v = 1\n    return r\nouter()\n";
        let f = run_source(src).unwrap_err();
        assert!(f.message.contains("UnboundLocalError"));
    }

    #[test]
    fn cells_survive_collection() {
        let src = "def counter():\n    n = 0\n    def inc():\n        nonlocal n\n        n += 1\n        return n\n    return inc\nc = counter()\nj = 0\nwhile j < 20000:\n    g = [\"x\" + \"y\", {\"k\": j}]\n    j += 1\nprint(c(), c())\n";
        assert_eq!(run_source(src).unwrap(), "1 2\n");
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
        let src = "keep = []\nfor i in range(100):\n    keep.append(\"v\" + \"x\")\nd = {\"total\": 0}\ni = 0\nwhile i < 20000:\n    junk = [\"g\", {\"k\": \"v\"}, i]\n    i += 1\nd[\"total\"] = len(keep)\nprint(d[\"total\"], keep[0], keep[99], d)\n";
        assert_eq!(run_source(src).unwrap(), "100 vx vx {'total': 100}\n");
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
}
