use crate::freeze::Paint;
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::process::exit;

pub fn install(args: &[String]) {
    let mut wheel = None;
    let mut target = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--to" => {
                i += 1;
                match args.get(i) {
                    Some(t) => target = Some(PathBuf::from(t)),
                    None => usage(),
                }
            }
            a if !a.starts_with('-') && wheel.is_none() => wheel = Some(a.to_string()),
            _ => usage(),
        }
        i += 1;
    }
    let Some(wheel) = wheel else { usage() };
    let target = target.or_else(compiler::site_packages).unwrap_or_else(|| {
        eprintln!("gecko: cannot find site-packages, set GECKO_HOME or pass --to");
        exit(1);
    });
    match unpack(&wheel, &target) {
        Ok(report) => report.print(&wheel, &target),
        Err(e) => {
            eprintln!("gecko: {e}");
            exit(1);
        }
    }
}

fn usage() -> ! {
    eprintln!("gecko: usage: gecko install wheel.whl [--to dir]");
    exit(2);
}

struct Report {
    files: usize,
    packages: BTreeSet<String>,
}

fn unpack(wheel: &str, target: &Path) -> Result<Report, String> {
    let file = std::fs::File::open(wheel).map_err(|e| format!("cannot read {wheel}: {e}"))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|_| format!("{wheel} is not a valid wheel"))?;
    std::fs::create_dir_all(target)
        .map_err(|e| format!("cannot create {}: {e}", target.display()))?;
    let mut files = 0;
    let mut packages = BTreeSet::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("corrupt wheel: {e}"))?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(format!("unsafe path in wheel: {}", entry.name()));
        };
        if let Some(std::path::Component::Normal(top)) = rel.components().next() {
            let top = top.to_string_lossy();
            if !top.ends_with(".dist-info") && !top.ends_with(".data") {
                packages.insert(top.into_owned());
            }
        }
        let out = target.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| format!("cannot create dir: {e}"))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("cannot create dir: {e}"))?;
        }
        let mut w = std::fs::File::create(&out)
            .map_err(|e| format!("cannot write {}: {e}", out.display()))?;
        io::copy(&mut entry, &mut w).map_err(|e| format!("cannot extract: {e}"))?;
        files += 1;
    }
    Ok(Report { files, packages })
}

impl Report {
    fn print(&self, wheel: &str, target: &Path) {
        let name = Path::new(wheel)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| wheel.to_string());
        let p = Paint::auto();
        println!("{} {}", p.wrap("1;32", "install"), p.wrap("1", &name));
        let pkgs: Vec<&str> = self.packages.iter().map(String::as_str).collect();
        row(&p, "packages", &pkgs.join(", "), false);
        row(&p, "files", &self.files.to_string(), false);
        row(&p, "into", &target.display().to_string(), true);
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
