use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Work(PathBuf);
impl Drop for Work {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn check(output: Output) -> Output {
    assert!(output.status.success(), "{output:?}");
    output
}

fn build(compiler: &Path, root: &Path, source: &Path, output: &Path) {
    check(
        Command::new(compiler)
            .current_dir(root)
            .arg("build")
            .arg(source)
            .arg("-o")
            .arg(output)
            .env("PATH", "/no-external-tools")
            .output()
            .unwrap(),
    );
}

#[test]
fn dunder_emits_x86_elf_and_rebuilds_itself_on_linux_x86() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let work =
        Work(std::env::temp_dir().join(format!("dunder-selfhost-x86-{}", std::process::id())));
    fs::create_dir(&work.0).unwrap();
    let source = root.join("selfhost/x86_linux.ddr");
    let stage1 = work.0.join("stage1");
    let stage2 = work.0.join("stage2");
    build(
        Path::new(env!("CARGO_BIN_EXE_dunder")),
        root,
        &source,
        &stage1,
    );
    build(&stage1, root, &source, &stage2);
    let image = fs::read(&stage2).unwrap();
    assert_eq!(&image[..7], b"\x7fELF\x02\x01\x01");
    assert_eq!(u16::from_le_bytes(image[18..20].try_into().unwrap()), 62);
    assert_eq!(
        u64::from_le_bytes(image[24..32].try_into().unwrap()),
        0x401000
    );

    let rejected = work.0.join("rejected.ddr");
    for action in [
        "other:=data",
        "data=[]",
        "data.reserve(2)",
        "mutate(&mut data)",
    ] {
        fs::write(&rejected, format!("fn mutate(x:&mut []i32){{x.clear()}} fn main(){{region s{{var data:[]i32=s.alloc(1);{action}}}}}")).unwrap();
        let failure = Command::new(&stage1)
            .arg("build")
            .arg(&rejected)
            .arg("-o")
            .arg(work.0.join("invalid"))
            .output()
            .unwrap();
        assert!(!failure.status.success(), "accepted {action}");
        assert!(!work.0.join("invalid").exists());
    }
    let fixture = root.join("tests/fixtures/regions.ddr");
    build(&stage1, root, &fixture, &work.0.join("region"));
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        let stage3 = work.0.join("stage3");
        build(&stage2, root, &source, &stage3);
        assert!(
            image == fs::read(&stage3).unwrap(),
            "stage2 and stage3 differ"
        );
        for (input, expected) in [
            ("examples/fibonacci.ddr", "6765\ntrue\n"),
            ("dogfood/arena/main.ddr", "28\n"),
            ("tests/fixtures/regions.ddr", "3000000011\n"),
        ] {
            let output = work.0.join("program");
            build(&stage3, root, &root.join(input), &output);
            assert_eq!(
                check(Command::new(output).output().unwrap()).stdout,
                expected.as_bytes()
            );
        }
    }
}
