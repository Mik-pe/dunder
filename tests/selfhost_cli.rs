#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn ok(result: Output) -> Output {
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

#[test]
fn selfhost_cli_runs_arguments_selects_affected_tests_and_enforces_timeouts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let work = std::env::temp_dir().join(format!("dunder-selfhost-cli-{}", std::process::id()));
    fs::create_dir_all(&work).unwrap();
    let stage1 = work.join("stage1");
    let stage2 = work.join("stage2");
    ok(Command::new(env!("CARGO_BIN_EXE_dunder"))
        .args(["build", "selfhost/main.ddr", "-o"])
        .arg(&stage1)
        .current_dir(root)
        .output()
        .unwrap());
    ok(Command::new(&stage1)
        .args(["build", "selfhost/main.ddr", "-o"])
        .arg(&stage2)
        .current_dir(root)
        .output()
        .unwrap());
    let call = |args: &[&str]| {
        Command::new(&stage2)
            .args(args)
            .current_dir(&work)
            .output()
            .unwrap()
    };
    fs::write(
        work.join("main.ddr"),
        "import sys\nfn main()->i32{print(sys.arg(1));return 37}",
    )
    .unwrap();
    let run = call(&["run", "main.ddr", "--", "räksmörgås * \""]);
    assert_eq!(run.status.code(), Some(37));
    assert_eq!(run.stdout, "räksmörgås * \"\n".as_bytes());

    for (value, passed, skipped, status) in
        [(42, 2, 0, 0), (42, 0, 2, 0), (41, 0, 1, 1), (41, 0, 1, 1)]
    {
        fs::write(work.join("cases.ddr"), format!("fn value()->i32{{{value}}} @test fn changed(){{assert(value()==42)}} @test fn other(){{true}}")).unwrap();
        let out = call(&["test", "cases.ddr", "--affected"]);
        assert_eq!(
            out.status.code(),
            Some(status),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let output = String::from_utf8(out.stdout).unwrap();
        assert!(output.contains(&format!("{passed} passed")));
        assert!(output.contains(&format!("{skipped} unaffected")));
    }
    fs::write(
        work.join("slow.ddr"),
        "@test fn loops(){while true{}} @test fn succeeds(){true}",
    )
    .unwrap();
    let out = call(&["test", "slow.ddr", "--timeout-ms", "500"]);
    assert_eq!(out.status.code(), Some(1));
    let output = String::from_utf8(out.stdout).unwrap();
    assert!(
        output.contains("loops (exit 124)") && output.contains("1 passed, 1 failed"),
        "{output}"
    );

    fs::write(
        work.join("dunder.toml"),
        "name = \"hello\"\nroot = \"main.ddr\"\n",
    )
    .unwrap();
    for target in ["arm64-macos", "arm64-linux", "x86_64-linux"] {
        let image = work.join(format!("image-{target}"));
        ok(call(&[
            "build",
            "--target",
            target,
            "-o",
            image.to_str().unwrap(),
        ]));
        let bytes = fs::read(image).unwrap();
        assert!(if target == "arm64-macos" {
            bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe])
        } else {
            bytes.starts_with(b"\x7fELF")
        });
    }
    let _ = fs::remove_dir_all(work);
}
