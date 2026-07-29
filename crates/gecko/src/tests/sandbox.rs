use super::check;

#[test]
fn sandbox_runs_code_and_returns_output() {
    let src = r#"
from gecko import sandbox
out = sandbox.run("print(2 + 3)\nprint(\"hi\")")
print(out)
"#;
    let want = r#"
5
hi

"#;
    check(src, want);
}

#[test]
fn sandbox_step_limit_stops_a_loop() {
    let src = r#"
from gecko import sandbox
try:
    sandbox.run("while True:\n    pass", 5000)
except SandboxError as e:
    print("stopped")
"#;
    let want = r#"
stopped
"#;
    check(src, want);
}

#[test]
fn sandbox_time_limit_stops_a_loop() {
    let src = r#"
from gecko import sandbox
try:
    sandbox.run("while True:\n    pass", 0, 0, 20)
except SandboxError as e:
    print("stopped")
"#;
    let want = r#"
stopped
"#;
    check(src, want);
}

#[test]
fn sandbox_error_is_catchable_and_isolated() {
    let src = r#"
from gecko import sandbox
x = 1
try:
    sandbox.run("1 / 0")
except SandboxError:
    print("caught")
print(x)
"#;
    let want = r#"
caught
1
"#;
    check(src, want);
}

#[test]
fn sandboxed_code_cannot_import_files() {
    let src = r#"
from gecko import sandbox
try:
    sandbox.run("import os")
except SandboxError:
    print("blocked")
"#;
    let want = r#"
blocked
"#;
    check(src, want);
}

#[test]
fn gecko_module_also_reaches_sandbox() {
    let src = r#"
import gecko
print(gecko.sandbox.run("print(1 + 1)"))
"#;
    let want = r#"
2

"#;
    check(src, want);
}
