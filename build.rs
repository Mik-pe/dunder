// Rebuild cache keys whenever compiler or bundled runtime inputs change.
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::Path,
};

fn fingerprint(path: &Path, hash: &mut DefaultHasher) {
    path.hash(hash);
    if path.is_dir() {
        let mut files = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        files.sort();
        for file in files {
            fingerprint(&file, hash);
        }
    } else {
        fs::read(path).unwrap().hash(hash);
    }
}

fn main() {
    let mut hash = DefaultHasher::new();
    for input in [
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "src",
        "stdlib",
        "runtime",
    ] {
        println!("cargo:rerun-if-changed={input}");
        fingerprint(Path::new(input), &mut hash);
    }
    println!(
        "cargo:rustc-env=DUNDER_COMPILER_HASH={:016x}",
        hash.finish()
    );
}
