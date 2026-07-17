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
fn relative_imports_resolve_within_a_package() {
    let dir = std::env::temp_dir().join(format!("gecko-relimport-{}", std::process::id()));
    let pkg = dir.join("pkg");
    let sub = pkg.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(
        pkg.join("__init__.py"),
        "from . import helpers\nfrom .helpers import shout\nfrom .sub import deep\n",
    )
    .unwrap();
    std::fs::write(
        pkg.join("helpers.py"),
        "def shout(s):\n    return s + \"!\"\n",
    )
    .unwrap();
    std::fs::write(
        sub.join("__init__.py"),
        "from ..helpers import shout\ndef deep():\n    return shout(\"deep\")\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.py"),
        "import pkg\nprint(pkg.helpers.shout(\"hi\"))\nprint(pkg.shout(\"hey\"))\nprint(pkg.deep())\n",
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
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hi!\nhey!\ndeep!\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_broken_submodule_reports_its_syntax_error() {
    let dir = std::env::temp_dir().join(format!("gecko-broken-sub-{}", std::process::id()));
    let pkg = dir.join("pkg");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(pkg.join("__init__.py"), "from . import broken\n").unwrap();
    std::fs::write(pkg.join("broken.py"), "def (:\n").unwrap();
    std::fs::write(dir.join("main.py"), "import pkg\n").unwrap();
    let gecko = env!("CARGO_BIN_EXE_gecko");
    let out = Command::new(gecko)
        .arg(dir.join("main.py"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("SyntaxError"), "{err}");
    assert!(err.contains("broken"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn dotted_packages_and_subpackages_resolve() {
    let dir = std::env::temp_dir().join(format!("gecko-dotted-{}", std::process::id()));
    let pkg = dir.join("pkg");
    let sub = pkg.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(pkg.join("__init__.py"), "name = \"pkg\"\n").unwrap();
    std::fs::write(pkg.join("mod.py"), "def f():\n    return 1\nV = 9\n").unwrap();
    std::fs::write(sub.join("__init__.py"), "def deep():\n    return 2\n").unwrap();
    std::fs::write(sub.join("leaf.py"), "def g():\n    return 3\n").unwrap();
    std::fs::write(
        dir.join("main.py"),
        "import pkg.mod\nimport pkg.sub.leaf as L\nfrom pkg.mod import f, V\nfrom pkg.sub import deep\nprint(pkg.name, pkg.mod.f(), pkg.mod.V)\nprint(pkg.sub.leaf.g(), L.g())\nprint(f(), V, deep())\n",
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
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "pkg 1 9\n3 3\n1 9 2\n"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn gecko_path_and_packages_resolve() {
    let dir = std::env::temp_dir().join(format!("gecko-path-{}", std::process::id()));
    let lib = dir.join("lib");
    let app = dir.join("app");
    let pkg = app.join("mypkg");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(lib.join("util.py"), "def double(n):\n    return n * 2\n").unwrap();
    std::fs::write(
        pkg.join("__init__.py"),
        "name = \"mypkg\"\ndef greet():\n    return \"hi \" + name\n",
    )
    .unwrap();
    std::fs::write(
        app.join("main.py"),
        "import util\nimport mypkg\nfrom mypkg import greet\nprint(util.double(21), mypkg.name, greet())\n",
    )
    .unwrap();
    let gecko = env!("CARGO_BIN_EXE_gecko");
    let out = Command::new(gecko)
        .arg(app.join("main.py"))
        .env("GECKO_PATH", &lib)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42 mypkg hi mypkg\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn install_a_wheel_then_import_it() {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    let dir = std::env::temp_dir().join(format!("gecko-wheel-{}", std::process::id()));
    let home = dir.join("home");
    std::fs::create_dir_all(&dir).unwrap();

    let wheel_path = dir.join("mathlib-1.0-py3-none-any.whl");
    let wheel = std::fs::File::create(&wheel_path).unwrap();
    let mut zw = zip::ZipWriter::new(wheel);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zw.start_file("mathlib/__init__.py", opts).unwrap();
    zw.write_all(b"def add(a, b):\n    return a + b\nNAME = \"mathlib\"\n")
        .unwrap();
    zw.start_file("mathlib/extra.py", opts).unwrap();
    zw.write_all(b"def triple(n):\n    return n * 3\n").unwrap();
    zw.start_file("mathlib-1.0.dist-info/METADATA", opts)
        .unwrap();
    zw.write_all(b"Name: mathlib\nVersion: 1.0\n").unwrap();
    zw.finish().unwrap();

    let gecko = env!("CARGO_BIN_EXE_gecko");
    let installed = Command::new(gecko)
        .args(["install", wheel_path.to_str().unwrap()])
        .env("GECKO_HOME", &home)
        .output()
        .unwrap();
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    assert!(home.join("site-packages/mathlib/__init__.py").is_file());

    let app = dir.join("app.py");
    std::fs::write(
        &app,
        "import mathlib\nfrom mathlib.extra import triple\nprint(mathlib.NAME, mathlib.add(2, 3), triple(4))\n",
    )
    .unwrap();
    let run = Command::new(gecko)
        .arg(&app)
        .env("GECKO_HOME", &home)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "mathlib 5 12\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn site_packages_resolve_via_gecko_home() {
    let dir = std::env::temp_dir().join(format!("gecko-site-{}", std::process::id()));
    let site = dir.join("home").join("site-packages").join("widget");
    let app = dir.join("app");
    std::fs::create_dir_all(&site).unwrap();
    std::fs::create_dir_all(&app).unwrap();
    std::fs::write(
        site.join("__init__.py"),
        "def make():\n    return \"widget\"\n",
    )
    .unwrap();
    std::fs::write(app.join("main.py"), "import widget\nprint(widget.make())\n").unwrap();
    let gecko = env!("CARGO_BIN_EXE_gecko");
    let out = Command::new(gecko)
        .arg(app.join("main.py"))
        .env("GECKO_HOME", dir.join("home"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "widget\n");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_filesystem_gecko_package_shadows_the_builtin() {
    let dir = std::env::temp_dir().join(format!("gecko-shadow-{}", std::process::id()));
    let pkg = dir.join("gecko");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(
        pkg.join("__init__.py"),
        "def hello():\n    return \"from package\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.py"),
        "from gecko import hello\nimport _gecko\nprint(hello())\nprint(_gecko.sandbox.run(\"print(1)\"))\n",
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
    assert_eq!(String::from_utf8_lossy(&out.stdout), "from package\n1\n\n");
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
