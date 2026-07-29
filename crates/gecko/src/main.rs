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
mod tests;
