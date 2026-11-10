use std::{fs, path::Path};

/// Install the example library beside the consumer's root module so imports
/// exercise the same filesystem loader as a copied application project.
pub fn install(project: &Path) {
    fn copy_tree(source: &Path, target: &Path) {
        fs::create_dir(target).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let path = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &path);
            } else {
                fs::copy(entry.path(), path).unwrap();
            }
        }
    }

    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("dogfood/graphics");
    copy_tree(&library, &project.join("graphics"));
}
