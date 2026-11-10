use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dunder-bundles-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn cli(&self, args: &[&str]) -> Output {
        let mut targeted = vec![args[0], "--target", "arm64-macos"];
        targeted.extend_from_slice(&args[1..]);
        self.cli_exact(&targeted)
    }

    fn cli_exact(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_dunder"))
            .args(args)
            .current_dir(&self.0)
            .env("PATH", "/no-external-tools")
            .output()
            .unwrap()
    }

    fn read(&self, path: &str) -> Vec<u8> {
        fs::read(self.0.join(path)).unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "status {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn failure(output: &Output, message: &str) {
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "macos")]
fn verify(project: &Project, path: &str) -> Output {
    // The build runs with no external tools. This separate read-only validator
    // checks interoperability with macOS rather than trusting our own parser.
    Command::new("/usr/bin/codesign")
        .args(["--verify", "--strict", "--verbose=2"])
        .arg(project.0.join(path))
        .env("PATH", "/no-external-tools")
        .output()
        .unwrap()
}

fn plist_value<'a>(plist: &'a str, key: &str) -> &'a str {
    let start = format!("<key>{key}</key><string>");
    plist
        .split_once(&start)
        .unwrap_or_else(|| panic!("missing {key}: {plist}"))
        .1
        .split_once("</string>")
        .unwrap()
        .0
}

#[test]
fn app_bundle_contains_native_executable_and_escaped_plist_without_external_tools() {
    let project = Project::new();
    project.write("main.ddr", "fn main(){print(42)}");
    let bundle = "out/A&B \"C\".app";
    let executable = format!("{bundle}/Contents/MacOS/A&B \"C\"");
    let info = format!("{bundle}/Contents/Info.plist");
    success(&project.cli(&["build", "main.ddr", "--app", "-o", bundle]));
    assert!(project
        .0
        .join(format!("{bundle}/Contents/_CodeSignature/CodeResources"))
        .is_file());
    #[cfg(target_os = "macos")]
    success(&verify(&project, bundle));
    let image = project.read(&executable);
    assert_eq!(&image[..4], &[0xcf, 0xfa, 0xed, 0xfe]);
    assert_ne!(
        fs::metadata(project.0.join(&executable))
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0
    );
    let plist = String::from_utf8(project.read(&info)).unwrap();
    for key in ["CFBundleExecutable", "CFBundleName", "CFBundleDisplayName"] {
        assert_eq!(plist_value(&plist, key), "A&amp;B &quot;C&quot;");
    }
    assert_eq!(plist_value(&plist, "CFBundlePackageType"), "APPL");
    assert_eq!(
        plist_value(&plist, "CFBundleVersion"),
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(
        plist_value(&plist, "CFBundleShortVersionString"),
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(plist_value(&plist, "LSMinimumSystemVersion"), "11.0");
    assert!(plist.contains("<key>NSHighResolutionCapable</key><true/>"));
    assert!(plist.contains("<key>NSSupportsAutomaticGraphicsSwitching</key><true/>"));
    let identifier = plist_value(&plist, "CFBundleIdentifier");
    assert!(identifier.starts_with("org.dunder.a-b-c-"), "{identifier}");
    assert!(identifier
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-')));

    project.write("out/A&B \"C\".app/Contents/Resources/kept.txt", "kept");
    project.write("main.ddr", "fn main(){print(99)}");
    success(&project.cli(&["build", "main.ddr", "--app", "-o", bundle]));
    #[cfg(target_os = "macos")]
    success(&verify(&project, &executable));
    assert_eq!(project.read(&info), plist.as_bytes());
    assert_eq!(
        project.read("out/A&B \"C\".app/Contents/Resources/kept.txt"),
        b"kept"
    );
    assert_ne!(project.read(&executable), image);

    success(&project.cli(&["build", "main.ddr", "--app", "-o", "A B C.app"]));
    let other = String::from_utf8(project.read("A B C.app/Contents/Info.plist")).unwrap();
    assert_ne!(plist_value(&other, "CFBundleIdentifier"), identifier);
}

#[test]
fn default_app_output_uses_source_or_manifest_name() {
    let project = Project::new();
    project.write("Hello World.ddr", "fn main(){print(42)}");
    success(&project.cli(&["build", "Hello World.ddr", "--app"]));
    assert!(project
        .0
        .join("build/arm64-macos/Hello World.app/Contents/MacOS/Hello World")
        .is_file());
    project.write(
        "dunder.toml",
        "name = \"ManifestApp\"\nroot = \"Hello World.ddr\"\n",
    );
    success(&project.cli(&["build", "--app"]));
    assert!(project
        .0
        .join("build/arm64-macos/ManifestApp.app/Contents/MacOS/ManifestApp")
        .is_file());
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn run_app_passes_program_arguments_and_exit_status() {
    let project = Project::new();
    project.write(
        "main.ddr",
        "import std.sys as sys\nfn main()->i32{assert(sys.arg_count()==4);print(sys.arg(1));print(sys.arg(2));print(sys.arg(3));return 7}",
    );
    let output = project.cli(&[
        "run",
        "main.ddr",
        "--app",
        "-o",
        "Run Me.app",
        "--",
        "å 🌍",
        "",
        "--app",
    ]);
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, "å 🌍\n\n--app\n".as_bytes());
    assert!(String::from_utf8_lossy(&output.stderr).contains("built Run Me.app"));
}

#[test]
fn type_errors_preserve_existing_bundle_and_do_not_create_new_output() {
    let project = Project::new();
    project.write("main.ddr", "fn main(){print(42)}");
    success(&project.cli(&["build", "main.ddr", "--app", "-o", "Kept.app"]));
    let executable = project.read("Kept.app/Contents/MacOS/Kept");
    let info = project.read("Kept.app/Contents/Info.plist");
    let resources = project.read("Kept.app/Contents/_CodeSignature/CodeResources");
    project.write("Kept.app/Contents/Resources/kept.txt", "kept");
    project.write("main.ddr", "fn main(){x:i32=true}");
    failure(
        &project.cli(&["build", "main.ddr", "--app", "-o", "Kept.app"]),
        "E0201",
    );
    assert_eq!(project.read("Kept.app/Contents/MacOS/Kept"), executable);
    assert_eq!(project.read("Kept.app/Contents/Info.plist"), info);
    assert_eq!(
        project.read("Kept.app/Contents/_CodeSignature/CodeResources"),
        resources
    );
    assert_eq!(
        project.read("Kept.app/Contents/Resources/kept.txt"),
        b"kept"
    );
    failure(
        &project.cli(&["build", "main.ddr", "--app", "-o", "New.app"]),
        "E0201",
    );
    assert!(!project.0.join("New.app").exists());
}

#[test]
fn app_destinations_cannot_alias_root_or_imported_sources() {
    for target in [
        "Alias.app/Contents/MacOS/Alias",
        "Alias.app/Contents/Info.plist",
        "Alias.app/Contents/_CodeSignature/CodeResources",
    ] {
        for hard_link in [false, true] {
            let project = Project::new();
            let source = "import helper\nfn main(){print(helper.answer())}";
            let helper = "pub fn answer()->i32{42}";
            project.write("main.ddr", source);
            project.write("helper.ddr", helper);
            project.write("Alias.app/Contents/MacOS/Alias", "old executable");
            project.write("Alias.app/Contents/Info.plist", "old plist");
            project.write(
                "Alias.app/Contents/_CodeSignature/CodeResources",
                "old resources",
            );
            fs::remove_file(project.0.join(target)).unwrap();
            let input = if hard_link { "helper.ddr" } else { "main.ddr" };
            if hard_link {
                fs::hard_link(project.0.join(input), project.0.join(target)).unwrap();
            } else {
                symlink(project.0.join(input), project.0.join(target)).unwrap();
            }
            let executable = project.read("Alias.app/Contents/MacOS/Alias");
            let info = project.read("Alias.app/Contents/Info.plist");
            let resources = project.read("Alias.app/Contents/_CodeSignature/CodeResources");
            failure(
                &project.cli(&["build", "main.ddr", "--app", "-o", "Alias.app"]),
                "output must not overwrite the source file",
            );
            assert_eq!(project.read("main.ddr"), source.as_bytes());
            assert_eq!(project.read("helper.ddr"), helper.as_bytes());
            assert_eq!(project.read("Alias.app/Contents/MacOS/Alias"), executable);
            assert_eq!(project.read("Alias.app/Contents/Info.plist"), info);
            assert_eq!(
                project.read("Alias.app/Contents/_CodeSignature/CodeResources"),
                resources
            );
        }
    }
}

#[test]
#[cfg(target_os = "macos")]
fn bundle_signature_seals_info_and_preserved_resources_and_rebuild_refreshes_them() {
    let project = Project::new();
    project.write("main.ddr", "fn main(){print(42)}");
    project.write("Sealed.app/Contents/Resources/å & text.txt", "original");
    success(&project.cli(&["build", "main.ddr", "--app", "-o", "Sealed.app"]));
    success(&verify(&project, "Sealed.app"));
    success(&verify(&project, "Sealed.app/Contents/MacOS/Sealed"));
    let envelope = project.read("Sealed.app/Contents/_CodeSignature/CodeResources");
    let envelope_text = String::from_utf8(envelope.clone()).unwrap();
    assert!(envelope_text.contains("<key>Resources/å &amp; text.txt</key>"));
    assert!(envelope_text.contains("<key>hash2</key>"));

    project.write("Sealed.app/Contents/Resources/å & text.txt", "changed");
    assert!(!verify(&project, "Sealed.app").status.success());
    success(&project.cli(&["build", "main.ddr", "--app", "-o", "Sealed.app"]));
    success(&verify(&project, "Sealed.app"));
    assert_ne!(
        project.read("Sealed.app/Contents/_CodeSignature/CodeResources"),
        envelope
    );
    project.write("Sealed.app/Contents/Resources/added.txt", "new resource");
    assert!(!verify(&project, "Sealed.app").status.success());
    success(&project.cli(&["build", "main.ddr", "--app", "-o", "Sealed.app"]));
    success(&verify(&project, "Sealed.app"));

    let info = project.read("Sealed.app/Contents/Info.plist");
    let changed = String::from_utf8(info).unwrap().replace(
        "<key>CFBundleDisplayName</key><string>Sealed</string>",
        "<key>CFBundleDisplayName</key><string>Renamed</string>",
    );
    project.write("Sealed.app/Contents/Info.plist", &changed);
    assert!(!verify(&project, "Sealed.app").status.success());
}

#[test]
#[cfg(target_os = "macos")]
fn renamed_bundle_keeps_and_seals_its_previous_executable() {
    let project = Project::new();
    project.write("main.ddr", "fn main(){print(42)}");
    success(&project.cli(&["build", "main.ddr", "--app", "-o", "Old.app"]));
    let original = project.read("Old.app/Contents/MacOS/Old");
    fs::rename(project.0.join("Old.app"), project.0.join("New.app")).unwrap();
    success(&project.cli(&["build", "main.ddr", "--app", "-o", "New.app"]));
    assert_eq!(project.read("New.app/Contents/MacOS/Old"), original);
    success(&verify(&project, "New.app"));
}

#[test]
fn unsupported_existing_bundle_entries_fail_before_publication_without_deleting_them() {
    for (name, expected) in [
        (
            "link",
            "bundle resources must be regular files or directories",
        ),
        ("signature", "unsupported existing bundle signature file"),
        ("root", "unsupported bundle entry outside Contents"),
    ] {
        let project = Project::new();
        project.write("main.ddr", "fn main(){print(42)}");
        success(&project.cli(&["build", "main.ddr", "--app", "-o", "Kept.app"]));
        let executable = project.read("Kept.app/Contents/MacOS/Kept");
        let info = project.read("Kept.app/Contents/Info.plist");
        let envelope = project.read("Kept.app/Contents/_CodeSignature/CodeResources");
        let kept = match name {
            "link" => {
                project.write("outside", "keep outside");
                fs::create_dir_all(project.0.join("Kept.app/Contents/Resources")).unwrap();
                symlink(
                    project.0.join("outside"),
                    project.0.join("Kept.app/Contents/Resources/link"),
                )
                .unwrap();
                "outside"
            }
            "signature" => {
                project.write("Kept.app/Contents/_CodeSignature/Extra", "keep extra");
                "Kept.app/Contents/_CodeSignature/Extra"
            }
            _ => {
                project.write("Kept.app/Extra", "keep root");
                "Kept.app/Extra"
            }
        };
        let bytes = project.read(kept);
        project.write("main.ddr", "fn main(){print(99)}");
        failure(
            &project.cli(&["build", "main.ddr", "--app", "-o", "Kept.app"]),
            expected,
        );
        assert_eq!(project.read(kept), bytes);
        assert_eq!(project.read("Kept.app/Contents/MacOS/Kept"), executable);
        assert_eq!(project.read("Kept.app/Contents/Info.plist"), info);
        assert_eq!(
            project.read("Kept.app/Contents/_CodeSignature/CodeResources"),
            envelope
        );
    }
}

#[test]
fn plain_output_cannot_be_a_hard_link_to_a_source() {
    let project = Project::new();
    let source = "fn main(){print(42)}";
    project.write("main.ddr", source);
    fs::hard_link(project.0.join("main.ddr"), project.0.join("output")).unwrap();
    failure(
        &project.cli(&["build", "main.ddr", "-o", "output"]),
        "output must not overwrite the source file",
    );
    assert_eq!(project.read("main.ddr"), source.as_bytes());
    assert_eq!(project.read("output"), source.as_bytes());
}

#[test]
fn invalid_app_commands_and_output_names_fail_without_publishing() {
    let project = Project::new();
    project.write("main.ddr", "fn main(){print(42)}");
    for command in ["check", "test"] {
        failure(
            &project.cli(&[command, "main.ddr", "--app"]),
            "--app is only supported by build and run",
        );
    }
    for name in [
        "output",
        "Output.App",
        ".app",
        "..app",
        "...app",
        "bad\nname.app",
    ] {
        failure(
            &project.cli(&["build", "main.ddr", "--app", "-o", name]),
            "--app output must end in a nonempty name followed by '.app'",
        );
        assert!(!project.0.join(name).exists());
    }
    failure(
        &project.cli(&["build", "main.ddr", "--app", "--app"]),
        "--app may only be specified once",
    );
    assert!(!project.0.join("build").exists());
}

#[test]
fn app_rejects_linux_targets_before_loading_source_or_publishing_output() {
    let project = Project::new();
    project.write("Kept.app/Contents/MacOS/Kept", "old executable");
    project.write("Kept.app/Contents/Info.plist", "old plist");
    project.write(
        "Kept.app/Contents/_CodeSignature/CodeResources",
        "old resources",
    );
    for target in ["arm64-linux", "x86_64-linux"] {
        for command in ["build", "run"] {
            let output = project.cli_exact(&[
                command,
                "absent.ddr",
                "--app",
                "--target",
                target,
                "-o",
                "Kept.app",
            ]);
            failure(&output, "--app requires target arm64-macos");
            assert!(String::from_utf8_lossy(&output.stderr).contains(target));
            assert!(output.stdout.is_empty());
            assert_eq!(
                project.read("Kept.app/Contents/MacOS/Kept"),
                b"old executable"
            );
            assert_eq!(project.read("Kept.app/Contents/Info.plist"), b"old plist");
            assert_eq!(
                project.read("Kept.app/Contents/_CodeSignature/CodeResources"),
                b"old resources"
            );
        }
    }
    assert!(!project.0.join("build").exists());
}
