// Rebuild cache keys whenever compiler or bundled runtime inputs change.
use std::{
    collections::hash_map::DefaultHasher,
    env,
    fmt::Write,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
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
    // The selfhost embedder reads the same list of shipped modules.
    let mut table = String::from("&[\n");
    for name in include_str!("stdlib/modules.txt")
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        writeln!(table, r#"    ("{name}", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/stdlib/{name}.ddr"))),"#).unwrap();
    }
    table.push_str("]\n");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(output.join("standard_modules.rs"), table).unwrap();

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
