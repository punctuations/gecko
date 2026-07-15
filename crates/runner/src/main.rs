use std::process::exit;

fn main() {
    let code = std::env::current_exe()
        .ok()
        .and_then(|p| bytecode::read_frozen(&p));
    let Some(code) = code else {
        eprintln!("gecko-runner: no embedded program, freeze one with gecko build");
        exit(2);
    };
    let mut vm = runtime::Vm::new();
    let run = vm.run(&code);
    print!("{}", run.output);
    if run.error {
        if run.message.is_empty() {
            eprintln!("RuntimeError");
        } else {
            eprintln!("{}", run.message);
        }
        exit(1);
    }
}
