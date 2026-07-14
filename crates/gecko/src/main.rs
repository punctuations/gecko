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

fn execute(src: &str) {
    match run_source(src) {
        Ok(output) => print!("{output}"),
        Err(msg) => {
            eprintln!("{msg}");
            exit(1);
        }
    }
}

fn run_source(src: &str) -> Result<String, String> {
    let module = parser::parse(src).map_err(|e| format!("SyntaxError: {}", e.message))?;
    let code = compiler::compile(&module).map_err(|e| format!("CompileError: {}", e.message))?;
    let mut vm = runtime::Vm::new();
    let run = vm.run(&code);
    if run.error {
        return Err("RuntimeError".into());
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
}
