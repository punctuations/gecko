use std::process::Command;

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
