use super::super::run_source;

#[test]
fn a_missing_import_raises_at_runtime() {
    let f = run_source("import no_such_module_zzz\n").unwrap_err();
    assert!(f.message.contains("ImportError"));
    assert!(f.message.contains("no_such_module_zzz"));
}
