// build.rs v1.3.48
use std::{env, fs, path::Path};

fn collect(path: &Path, versions: &mut Vec<[u64; 3]>) {
    println!("cargo:rerun-if-changed={}", path.display());
    if path.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(path).unwrap().map(|entry| entry.unwrap().path()).collect();
        entries.sort();
        for entry in entries { collect(&entry, versions); }
        return;
    }
    let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("");
    if !matches!(extension, "rs" | "py" | "js" | "cjs" | "html" | "css" | "sh" | "ps1") { return; }
    let source = fs::read_to_string(path).unwrap();
    let marker = format!("{} v", path.file_name().unwrap().to_str().unwrap());
    for line in source.lines().take(3) {
        if let Some((_, suffix)) = line.split_once(&marker) {
            let digits: String = suffix.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            let values: Vec<u64> = digits.split('.').map(|value| value.parse().unwrap()).collect();
            assert_eq!(values.len(), 3, "Invalid version: {}", path.display());
            versions.push([values[0], values[1], values[2]]);
            return;
        }
    }
}

fn mean(values: impl Iterator<Item = u64>) -> u64 {
    let values: Vec<_> = values.collect();
    if values.is_empty() { return 0; }
    let count = values.len() as u64;
    (values.iter().sum::<u64>() * 2 + count) / (count * 2)
}

fn main() {
    assert_eq!(mean([1, 2].into_iter()), 2);
    assert_eq!(mean([0, 0].into_iter()), 0);
    assert_eq!(mean([1, 1, 2].into_iter()), 1);
    let root = env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&root);
    let mut versions = Vec::new();
    for name in ["src", "ui", "tools", "scripts", "tests", "run.sh", "run.ps1", "build.rs"] {
        collect(&root.join(name), &mut versions);
    }
    assert!(!versions.is_empty(), "No application source versions found");
    let x = mean(versions.iter().map(|value| value[0]));
    let y = mean(versions.iter().map(|value| value[1]).filter(|value| *value > 0));
    let z = mean(versions.iter().map(|value| value[2]));
    println!("cargo:rustc-env=OACLIS_APP_VERSION=v{x}.{y}.{z}");
}
