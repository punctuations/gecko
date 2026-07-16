use std::path::{Path, PathBuf};
use std::process::exit;

pub fn build(args: &[String]) {
    let mut input = None;
    let mut out = None;
    let mut debug = false;
    let mut i = 0;
    let usage = || {
        eprintln!("gecko: usage: gecko build file [-o out] [--debug]");
        exit(2);
    };
    while i < args.len() {
        match args[i].as_str() {
            "--debug" => debug = true,
            "-o" => {
                i += 1;
                match args.get(i) {
                    Some(o) => out = Some(o.clone()),
                    None => usage(),
                }
            }
            a if !a.starts_with('-') && input.is_none() => input = Some(a.to_string()),
            _ => usage(),
        }
        i += 1;
    }
    let Some(input) = input else {
        usage();
        return;
    };
    let started = std::time::Instant::now();
    let src = match std::fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("gecko: cannot read {input}: {e}");
            exit(1);
        }
    };
    let module = match parser::parse(&src) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("SyntaxError: {}", e.message);
            exit(1);
        }
    };
    let base = Path::new(&input).parent().map(|p| {
        if p.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            p.to_path_buf()
        }
    });
    let code = match compiler::compile_with_base(&module, base) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("CompileError: {}", e.message);
            exit(1);
        }
    };
    let compiled = started.elapsed();
    let out = out.unwrap_or_else(|| {
        let stem = Path::new(&input)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "a.out".into());
        if stem == input {
            format!("{stem}.bin")
        } else {
            stem
        }
    });
    if out == input {
        eprintln!("gecko: output would overwrite the input, pass -o");
        exit(2);
    }
    let payload = bytecode::to_bytes(&code);
    let base = runtime_base(debug);
    let base_len = base.len();
    let mut bytes = base;
    bytes.extend_from_slice(&payload);
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(bytecode::FROZEN_TRAILER);
    let total_len = bytes.len();
    if let Err(e) = std::fs::write(&out, bytes) {
        eprintln!("gecko: cannot write {out}: {e}");
        exit(1);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755));
    }
    let (ncodes, nops) = code_stats(&code);
    let lines = src.lines().count();
    let runtime_kind = if debug {
        "this gecko"
    } else {
        "release runner"
    };
    let p = Paint::auto();
    println!("{} {}", p.wrap("1;32", "build"), p.wrap("1", &input));
    row(
        &p,
        "compile",
        &format!(
            "{}, {}, {}, {}",
            plural(lines, "line"),
            plural(ncodes, "code object"),
            plural(nops, "instruction"),
            human_time(compiled)
        ),
        false,
    );
    row(&p, "bytecode", &human_bytes(payload.len()), false);
    row(
        &p,
        "runtime",
        &format!("{} ({runtime_kind})", human_bytes(base_len)),
        false,
    );
    row(
        &p,
        &out,
        &format!(
            "{} in {}",
            human_bytes(total_len),
            human_time(started.elapsed())
        ),
        true,
    );
}

fn runtime_base(debug: bool) -> Vec<u8> {
    let runner = if cfg!(windows) {
        "gecko-runner.exe"
    } else {
        "gecko-runner"
    };
    let exe = std::env::current_exe().ok();
    let dir = exe.as_ref().and_then(|p| p.parent());
    let mut candidates = Vec::new();
    if let Some(dir) = dir {
        if debug == cfg!(debug_assertions) {
            candidates.push(dir.join(runner));
        }
        if let Some(root) = dir.parent() {
            let profile = if debug { "debug" } else { "release" };
            candidates.push(root.join(profile).join(runner));
        }
    }
    for c in &candidates {
        if let Ok(bytes) = std::fs::read(c) {
            return bytes;
        }
    }
    if debug || !cfg!(debug_assertions) {
        match exe.and_then(|p| std::fs::read(p).ok()) {
            Some(bytes) => return bytes,
            None => {
                eprintln!("gecko: cannot read the runtime executable");
                exit(1);
            }
        }
    }
    eprintln!(
        "gecko: no release gecko-runner found, run `cargo build --release -p runner` or pass --debug"
    );
    exit(1);
}

pub struct Paint(pub bool);

impl Paint {
    pub fn auto() -> Paint {
        use std::io::IsTerminal;
        Paint(std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none())
    }

    pub fn wrap(&self, code: &str, s: &str) -> String {
        if self.0 {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }

    pub fn wrap_pad(&self, code: &str, s: &str, width: usize) -> String {
        let mut out = self.wrap(code, s);
        for _ in s.len()..width {
            out.push(' ');
        }
        out
    }
}

fn row(p: &Paint, label: &str, value: &str, strong: bool) {
    let dots = ".".repeat(18usize.saturating_sub(label.len()).max(3));
    let value = if strong {
        p.wrap("1", value)
    } else {
        value.to_string()
    };
    println!("  {label} {} {value}", p.wrap("2", &dots));
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

fn code_stats(code: &bytecode::Code) -> (usize, usize) {
    let mut ncodes = 1;
    let mut nops = code.ops.len();
    for child in &code.codes {
        let (c, o) = code_stats(child);
        ncodes += c;
        nops += o;
    }
    (ncodes, nops)
}

fn human_bytes(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    }
}

fn human_time(d: std::time::Duration) -> String {
    let us = d.as_micros();
    if us < 1000 {
        format!("{us} us")
    } else if us < 1_000_000 {
        format!("{:.1} ms", us as f64 / 1000.0)
    } else {
        format!("{:.2} s", us as f64 / 1_000_000.0)
    }
}
