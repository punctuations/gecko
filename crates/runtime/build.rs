use std::env;
use std::path::PathBuf;
use std::process::Command;

// Meson is often installed without ending up on PATH, so fall back to running
// it as a module. clang-cl is forced so the objects match the MSVC ABI that the
// Rust msvc target links against.
fn meson(args: &[&str]) -> Result<(), String> {
    let python = env::var("GECKO_PYTHON").unwrap_or_else(|_| "python".into());
    let launchers: [(String, Vec<String>); 2] = [
        ("meson".into(), args.iter().map(|s| s.to_string()).collect()),
        (python, {
            let mut v = vec!["-m".into(), "mesonbuild.mesonmain".into()];
            v.extend(args.iter().map(|s| s.to_string()));
            v
        }),
    ];
    let msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let mut err = "no meson launcher found".to_string();
    for (prog, a) in launchers {
        let mut cmd = Command::new(&prog);
        cmd.args(&a);
        if msvc {
            cmd.env("CC", "clang-cl");
        }
        match cmd.status() {
            Ok(s) if s.success() => return Ok(()),
            Ok(s) => return Err(format!("{prog} {a:?} failed: {s}")),
            Err(e) => err = format!("{prog}: {e}"),
        }
    }
    Err(err)
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let native = manifest.join("../../native");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let build = out.join("meson");

    let native_s = native.to_str().unwrap();
    let build_s = build.to_str().unwrap();

    if build.join("build.ninja").exists() {
        meson(&["setup", "--reconfigure", build_s, native_s]).unwrap();
    } else {
        meson(&["setup", build_s, native_s, "--buildtype=release"]).unwrap();
    }
    meson(&["compile", "-C", build_s]).unwrap();

    println!("cargo:rustc-link-search=native={build_s}");
    println!("cargo:rustc-link-lib=static=gecko_runtime");
    for f in ["src", "include", "meson.build"] {
        println!("cargo:rerun-if-changed={}", native.join(f).display());
    }
}
