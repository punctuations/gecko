use std::process::Command;

#[test]
fn imports_run_sibling_modules() {
    let dir = std::env::temp_dir().join(format!("gecko-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("lib.py"),
        "greeting = \"hi\"\ndef twice(n):\n    return n * 2\nclass Box:\n    def __init__(self, v):\n        self.v = v\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.py"),
        "import lib\nfrom lib import twice, Box as B\nprint(lib.greeting, lib.twice(3), twice(4))\nb = B(7)\nprint(b.v)\n",
    )
    .unwrap();
    let gecko = env!("CARGO_BIN_EXE_gecko");
    let out = Command::new(gecko)
        .arg(dir.join("app.py"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hi 6 8\n7\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn modules_keep_separate_namespaces() {
    let dir = std::env::temp_dir().join(format!("gecko-import-iso-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.py"), "x = \"a\"\ndef who():\n    return x\n").unwrap();
    std::fs::write(dir.join("b.py"), "x = \"b\"\ndef who():\n    return x\n").unwrap();
    std::fs::write(
        dir.join("main.py"),
        "import a\nimport b\nx = \"main\"\nprint(a.who(), b.who(), x)\n",
    )
    .unwrap();
    let gecko = env!("CARGO_BIN_EXE_gecko");
    let out = Command::new(gecko)
        .arg(dir.join("main.py"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "a b main\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cyclic_imports_resolve() {
    let dir = std::env::temp_dir().join(format!("gecko-import-cyc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("ping.py"),
        "import pong\ndef name():\n    return \"ping\"\ndef ask():\n    return pong.name()\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("pong.py"),
        "import ping\ndef name():\n    return \"pong\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.py"),
        "import ping\nprint(ping.name(), ping.ask())\n",
    )
    .unwrap();
    let gecko = env!("CARGO_BIN_EXE_gecko");
    let out = Command::new(gecko)
        .arg(dir.join("main.py"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "ping pong\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_frozen_binary_bundles_its_imports() {
    let dir = std::env::temp_dir().join(format!("gecko-import-freeze-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("helper.py"), "def val():\n    return 99\n").unwrap();
    std::fs::write(dir.join("prog.py"), "import helper\nprint(helper.val())\n").unwrap();
    let out = dir.join("prog");
    let gecko = env!("CARGO_BIN_EXE_gecko");
    let built = Command::new(gecko)
        .args([
            "build",
            dir.join("prog.py").to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--debug",
        ])
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    std::fs::remove_file(dir.join("helper.py")).unwrap();
    let run = Command::new(&out).output().unwrap();
    assert!(run.status.success());
    assert_eq!(String::from_utf8_lossy(&run.stdout), "99\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn build_freezes_a_runnable_binary() {
    let dir = std::env::temp_dir().join(format!("gecko-freeze-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("frozen.py");
    std::fs
        ::write(
            &script,
            "def greet(n):\n    return \"hi \" + n\nprint([greet(w) for w in [\"a\", \"b\"]])\ntry:\n    1 / 0\nexcept ZeroDivisionError:\n    print(\"safe\")\n"
        )
        .unwrap();
    let out = dir.join("frozen");
    let gecko = env!("CARGO_BIN_EXE_gecko");
    let built = Command::new(gecko)
        .args([
            "build",
            script.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--debug",
        ])
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = Command::new(&out).output().unwrap();
    assert!(run.status.success());
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "['hi a', 'hi b']\nsafe\n"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn frozen_binaries_report_errors() {
    let dir = std::env::temp_dir().join(format!("gecko-freeze-err-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("boom.py");
    std::fs::write(
        &script,
        "print(\"before\")\nraise ValueError(\"frozen boom\")\n",
    )
    .unwrap();
    let out = dir.join("boom");
    let gecko = env!("CARGO_BIN_EXE_gecko");
    let built = Command::new(gecko)
        .args([
            "build",
            script.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--debug",
        ])
        .output()
        .unwrap();
    assert!(built.status.success());
    let run = Command::new(&out).output().unwrap();
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&run.stdout), "before\n");
    assert!(String::from_utf8_lossy(&run.stderr).contains("ValueError: frozen boom"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn dash_reads_the_program_from_stdin() {
    use std::io::Write;
    let gecko = env!("CARGO_BIN_EXE_gecko");
    let mut child = Command::new(gecko)
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"t = 0\nfor x in range(4):\n    t += x * x\nprint(t)\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "14\n");
}

#[test]
fn help_and_unknown_flags() {
    let gecko = env!("CARGO_BIN_EXE_gecko");
    let help = Command::new(gecko).arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("usage: gecko"));
    let bad = Command::new(gecko).arg("--bogus").output().unwrap();
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("usage: gecko"));
    let version = Command::new(gecko).arg("-V").output().unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).starts_with("gecko "));
}
