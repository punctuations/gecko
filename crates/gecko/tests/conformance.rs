use std::path::{Path, PathBuf};
use std::process::Command;

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("conformance")
}

fn interpreter() -> Option<String> {
    if let Ok(p) = std::env::var("GECKO_CONFORMANCE_PYTHON") {
        assert!(
            Command::new(&p).arg("--version").output().is_ok(),
            "GECKO_CONFORMANCE_PYTHON is set to {p:?}, which cannot be run"
        );
        return Some(p);
    }
    for c in ["python3.14", "python3.13", "python3.12", "python3"] {
        if Command::new(c).arg("--version").output().is_ok() {
            return Some(c.to_string());
        }
    }
    None
}

fn run(cmd: &str, file: &Path) -> (String, String) {
    let out = Command::new(cmd)
        .arg(file)
        .output()
        .unwrap_or_else(|e| panic!("run {cmd} {}: {e}", file.display()));
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn gecko_matches_cpython() {
    let python = match interpreter() {
        Some(p) => p,
        None => return,
    };
    let dir = corpus();
    let gecko = env!("CARGO_BIN_EXE_gecko");

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no conformance programs found");

    let mut diverged = Vec::new();
    for f in &files {
        let (want, want_err) = run(&python, f);
        if !want_err.is_empty() {
            continue;
        }
        let (got, _) = run(gecko, f);
        if got != want {
            diverged.push(format!(
                "{}\n  cpython: {:?}\n  gecko:   {:?}",
                f.file_stem().unwrap().to_string_lossy(),
                want,
                got
            ));
        }
    }

    assert!(
        diverged.is_empty(),
        "{} of {} programs diverge from {python}:\n{}",
        diverged.len(),
        files.len(),
        diverged.join("\n")
    );
}
