use super::run_source;

mod actors;
mod arrays;
mod builtins;
mod classes;
mod errors;
mod functions;
mod generators;
mod imports;
mod sandbox;
mod syntax;
mod types;

fn check(src: &str, want: &str) {
    let src = src.strip_prefix('\n').unwrap_or(src);
    let want = want.strip_prefix('\n').unwrap_or(want);
    assert_eq!(run_source(src).unwrap(), want);
}
