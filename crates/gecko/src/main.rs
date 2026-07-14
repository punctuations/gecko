use std::process::exit;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("--version" | "-V") => println!("gecko {VERSION}"),
        Some("-c") => match args.get(1) {
            Some(src) => execute(src),
            None => {
                eprintln!("gecko: -c needs an argument");
                exit(2);
            }
        },
        Some(path) if !path.starts_with('-') => match std::fs::read_to_string(path) {
            Ok(src) => execute(&src),
            Err(e) => {
                eprintln!("gecko: cannot read {path}: {e}");
                exit(1);
            }
        },
        Some(other) => {
            eprintln!("gecko: unknown argument '{other}'");
            exit(2);
        }
    }
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

fn execute(src: &str) {
    match run_source(src) {
        Ok(output) => print!("{output}"),
        Err(f) => {
            print!("{}", f.output);
            eprintln!("{}", f.message);
            exit(1);
        }
    }
}

fn run_source(src: &str) -> Result<String, Failure> {
    let module = parser::parse(src).map_err(|e| format!("SyntaxError: {}", e.message))?;
    let code = compiler::compile(&module).map_err(|e| format!("CompileError: {}", e.message))?;
    let mut vm = runtime::Vm::new();
    let run = vm.run(&code);
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
}
