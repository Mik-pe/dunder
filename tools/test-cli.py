#!/usr/bin/env python3
"""Exercise a native self-built Dunder compiler against isolated consumer projects."""

import argparse
import json
import os
import plistlib
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path


class Project:
    def __init__(self, compiler, root):
        self.compiler = compiler
        self.root = root

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(value, encoding="utf-8")
        return path

    def call(self, *args, cwd=None, status=0):
        result = subprocess.run(
            [self.compiler, *args], cwd=cwd or self.root,
            capture_output=True, encoding="utf-8", timeout=60,
        )
        assert result.returncode == status, (
            f"{args} exited {result.returncode}, expected {status}\n"
            f"{result.stdout}{result.stderr}"
        )
        return result


def modules_and_manifest(p):
    p.write("dunder.toml", 'name = "consumer"\nroot = "main.ddr"\ntest_root = "cases.ddr"\n')
    p.write("main.ddr", "import sys\nimport link as lib\nfn main()->i32{print(sys.arg(1));return lib.answer()}\n")
    p.write("real.v1/depth/module.ddr", "import ../../shared/value as v\npub fn answer()->i32{v.answer()}\n")
    source = p.write("shared/value.ddr", "pub fn answer()->i32{42}\n")
    (p.root / "link.ddr").symlink_to("real.v1/depth/module.ddr")
    p.write("cases.ddr", '@test fn selected(){true}\n')
    nested = p.root / "nested"
    (nested / "dunder.toml").mkdir(parents=True)
    argument = 'räksmörgås * " with spaces'
    assert p.call("run", "--", argument, cwd=nested, status=42).stdout == argument + "\n"
    assert "1 passed, 0 failed" in p.call("test", cwd=nested).stdout
    listing = p.call("build-tests", "--list", cwd=nested).stdout.splitlines()
    assert len(listing) == 1 and listing[0].endswith("cases.ddr::selected")
    p.call("build-tests", "--group", "0", "-o", "group", cwd=nested)
    subprocess.run([nested / "group"], cwd=nested, check=True, timeout=10)
    p.call("build-tests", "--group", "8", cwd=nested, status=1)
    saved = p.root / "published"
    p.call("build", "-o", saved)
    original = saved.read_bytes()
    p.write("shared/value.ddr", "pub fn answer()->i32{42}\nfn unused()->i32{true}\n")
    for action in ("build", "run", "check", "test", "build-tests"):
        result = p.call(action, "main.ddr", status=1)
        assert "value.ddr:2:" in result.stderr, result
    p.call("build", "-o", saved, status=1)
    assert saved.read_bytes() == original
    errors = p.call("check", "main.ddr", "--agent", status=1)
    diagnostic = json.loads(errors.stdout)
    assert diagnostic["kind"] == "error" and diagnostic["path"].endswith("value.ddr")
    assert 0 <= diagnostic["start"] <= diagnostic["end"] <= len(source.read_bytes())
    original_source = "pub fn answer()->i32{42}\n"
    p.write("shared/value.ddr", original_source)
    alias = p.root / "linked-output"
    os.link(source, alias)
    p.call("build", "-o", alias, status=1)
    assert alias.read_text() == original_source
    p.write("dunder.toml", '# package\nname = "consumer"\nroot = main.ddr\n')
    assert "dunder.toml:3:" in p.call("build", cwd=nested, status=2).stderr


def targets_and_cache(p):
    p.write("portable.ddr", "fn main(){print(42)}\n")
    p.write("platform.ddr", 'capability unsafe\npub unsafe fn state()->ptr{static_bytes("linux",16)}\n')
    p.write("with-import.ddr", "import platform\nfn main(){print(42)}\n")
    for name, magic in (
        ("aarch64-apple-darwin", b"\xcf\xfa\xed\xfe"),
        ("aarch64-unknown-linux-gnu", b"\x7fELF"),
        ("x86_64-unknown-linux-gnu", b"\x7fELF"),
        ("wasm32-unknown-unknown", b"\x00asm"),
    ):
        for source in ("portable", "with-import"):
            p.call("build", source + ".ddr", "--target", name, "-o", source)
        data = (p.root / "portable").read_bytes()
        assert data.startswith(magic) and data == (p.root / "with-import").read_bytes()
    p.write("cached.ddr", "import calc\nfn main(){print(calc.answer())}\n")

    def incremental(expected):
        result = p.call("run", "cached.ddr", "--incremental", "--timings", "-o", "cached")
        assert result.stdout == str(expected) + "\n", result
        return int(re.search(r"functions (\d+) reused", result.stderr).group(1))

    p.write("calc.ddr", "fn twice(n:i32)->i32{n+n}\npub fn answer()->i32{twice(21)}\n")
    incremental(42)
    assert incremental(42) > 0
    fragments = list((p.root / "build/.dunder-cache").rglob("*.fn"))
    assert fragments
    for fragment in fragments:
        fragment.write_bytes(b"invalid fragment")
    incremental(42)
    p.write("calc.ddr", "fn twice(n:i32)->i32{n+n}\npub fn answer()->i32{twice(22)}\n")
    incremental(44)


def isolated_tests(p):
    for val, passed, skipped, code in ((42, 2, 0, 0), (42, 0, 2, 0), (41, 0, 1, 1), (41, 0, 1, 1)):
        p.write("affected.ddr", f"fn value()->i32{{{val}}}\n@test fn changed(){{value()==42}}\n@test fn other(){{true}}\n")
        result = p.call("test", "affected.ddr", "--affected", status=code)
        assert f"{passed} passed" in result.stdout and f"{skipped} unaffected" in result.stdout
    p.write("timeout.ddr", '''
capability unsafe
import sys
@extern("c","fork") unsafe fn fork()->i32
@extern("c","usleep") unsafe fn sleep(us:u32)->i32
@extern("c","_exit") unsafe fn finish(status:i32)
@test fn descendant(){
    unsafe {
        pid:=fork()
        assert(pid>=0)
        if pid==0 {
            print("child-ready")
            waiting:=sleep(1000000)
            write:=sys.write_text("escaped", "child survived")
            finish(0)
        }
        while true { waiting:=sleep(10000) }
    }
}
@test fn next(){true}
''')
    report = p.call("test", "timeout.ddr", "--timeout-ms", "250", status=1).stdout
    assert "child-ready" in report and "exit 124" in report and "PASS timeout.ddr::next" in report and ", 1 failed" in report, report
    time.sleep(1)
    assert not (p.root / "escaped").exists()


def scalar_optimizer_controls(p):
    p.write('scalar.ddr', 'fn sum(values:&[]i64)->i64{var result:i64=0;var i:=0;'
            'while i<values.len{result=result+values[i];i=i+1};result}\n'
            'fn main(){values:[]i64=[20,22];print(sum(&values))}\n')
    def build(mode, name, incremental=False):
        flags = ['--incremental'] if incremental else []
        result = p.call('build', 'scalar.ddr', '--scalar-opt=' + mode,
                        '--timings=json', '-o', name, *flags)
        return json.loads(result.stderr)
    enabled = build('on', 'scalar-on')['optimizer']
    disabled = build('off', 'scalar-off')['optimizer']
    assert enabled['enabled'] and enabled['modeled'] >= 1 and enabled['emitted'] >= 1, enabled
    assert enabled['proven_bounds'] >= 1, enabled
    assert not disabled['enabled'] and all(disabled[k] == 0 for k in
            ('modeled', 'emitted', 'rewrites', 'proven_bounds', 'fallbacks')), disabled
    for binary in ('scalar-on', 'scalar-off'):
        result = subprocess.run([p.root / binary], capture_output=True, text=True, timeout=10)
        assert result.returncode == 0 and result.stdout == '42\n', result
    on = p.call('check', 'scalar.ddr', '--agent', '--scalar-opt=on')
    off = p.call('check', 'scalar.ddr', '--agent', '--scalar-opt=off')
    assert on.stdout == off.stdout and on.stderr == off.stderr == ''
    build('on', 'cached-on', True)
    assert build('on', 'cached-on', True)['functions']['reused'] > 0
    assert build('off', 'cached-off', True)['functions']['reused'] == 0
    assert build('off', 'cached-off', True)['functions']['reused'] > 0
    assert build('on', 'cached-on', True)['functions']['reused'] > 0
    p.write('scalar-tests.ddr', 'fn answer()->i32{40+2}\n@test fn works(){assert(answer()==42)}\n')
    for mode, expected in [('on', '1 passed'), ('on', '1 unaffected'),
                           ('off', '1 passed'), ('off', '1 unaffected'), ('on', '1 passed')]:
        result = p.call('test', 'scalar-tests.ddr', '--affected', '--scalar-opt=' + mode, '-o', 'scalar-results')
        assert expected in result.stdout, result
    p.call('check', 'scalar.ddr', '--scalar-opt=unknown', status=2)


def phase_timings(p):
    p.write("phases.ddr", "fn answer()->i32{42}\nfn main(){value:=answer();print(value)}\n")
    normal = p.call("build", "phases.ddr", "-o", "plain")
    assert normal.stdout == "built plain\n" and normal.stderr == "", normal
    phases = {"module_load_lex_resolve", "build_setup", "compiler", "image_format_sign", "publication"}

    def report(result, *, checking=False, target=None):
        # One complete record on stderr, with every measured interval accounted
        # for. No build/run output or check --agent JSON is mixed into it.
        assert len(result.stderr.splitlines()) == 1, result
        record = json.loads(result.stderr)
        assert record["kind"] == "timings" and record["version"] == 1, record
        assert record["operation"] == ("check" if checking else "build"), record
        assert record["scope"] == "compiler_pipeline" and record["clock"] == "monotonic", record
        assert record["clock_ok"] is True and record["unit"] == "ns", record
        if target:
            assert record["target"] == target, record
        assert record["modules"] == 1, record
        assert set(record["functions"]) == {"reused", "emitted"}, record
        assert record["functions"]["reused"] >= 0 and record["functions"]["emitted"] >= 0, record
        assert set(record["phases"]) == phases, record
        assert all(type(value) is int and value >= 0 for value in record["phases"].values()), record
        assert record["total"] == sum(record["phases"].values()) and record["total"] > 0, record
        assert record["phases"]["compiler"] > 0, record
        skipped = {"image_format_sign", "publication"} if checking else set()
        assert set(record["skipped_phases"]) == skipped, record
        assert all(record["phases"][name] == 0 for name in skipped), record
        return record

    measured = p.call("build", "phases.ddr", "--timings=json", "-o", "measured")
    counts = report(measured)["functions"]
    assert counts == {"reused": 0, "emitted": 2}, counts
    assert measured.stdout == "built measured\n"
    assert (p.root / "plain").read_bytes() == (p.root / "measured").read_bytes()
    human = p.call("build", "phases.ddr", "--timings", "-o", "human")
    assert "functions 0 reused, 2 emitted\n" in human.stderr and "timings module_load_lex_resolve=" in human.stderr, human
    for target in ("arm64-macos", "arm64-linux", "x86_64-linux", "wasm32"):
        report(p.call("build", "phases.ddr", "--target", target, "--timings=json", "-o", target), target=target)
    baseline = p.call("check", "phases.ddr", "--agent")
    checked = p.call("check", "phases.ddr", "--agent", "--timings=json")
    assert report(checked, checking=True)["functions"] == {"reused": 0, "emitted": 0}
    assert checked.stdout == baseline.stdout and baseline.stderr == ""
    assert json.loads(checked.stdout.splitlines()[-1])["kind"] == "ok"
    ran = p.call("run", "phases.ddr", "--timings=json", "-o", "ran")
    report(ran)
    assert ran.stdout == "42\n", ran
    report(p.call("build", "phases.ddr", "--target", "arm64-macos", "--app", "--timings=json", "-o", "Timed.app"), target="arm64-macos")
    p.write("phase-tests.ddr", "@test fn phase(){true}\n")
    tested = p.call("test", "phase-tests.ddr", "--timings=json", "-o", "phase-tests")
    report(tested)
    assert "1 passed, 0 failed" in tested.stdout, tested
    for _ in range(2):
        cached = report(p.call("build", "phases.ddr", "--incremental", "--timings=json", "-o", "cached"))
    assert cached["functions"]["reused"] > 0 and sum(cached["functions"].values()) == 2, cached
    p.call("build", "phases.ddr", "--timings=xml", status=2)
    # Exercise the reporter's clock-failure path independently of host timing.
    sources = Path(__file__).resolve().parents[1] / "selfhost"
    for module in ("cli", "timings"):
        p.write(module + ".ddr", (sources / (module + ".ddr")).read_text())
    p.write("clock-failure.ddr", "import timings\nfn main(){timings.report(true,false,0,1,0,1,-1,-1,-1,-1,-1,-1)}\n")
    unavailable = json.loads(p.call("run", "clock-failure.ddr").stderr)
    assert unavailable["clock_ok"] is False and unavailable["total"] is None, unavailable
    assert all(value is None for value in unavailable["phases"].values()), unavailable


def filesystem(p):
    (p.root / "invalid-utf8").write_bytes(b"\xf0\x80\x80\x80")
    p.write("filesystem.ddr", '''
import sys
fn main(){
    assert(sys.read_text("invalid-utf8").error==92)
    assert(sys.read_text("absent").error==2)
    assert(sys.mkdir("files").ok)
    assert(sys.write_text("files/å","one").ok)
    assert(sys.append_text("files/å", " två").ok)
    assert(sys.rename("files/å","files/data").ok)
    assert(sys.read_text("files/data").value=="one två")
    assert(sys.metadata("files/data").size==8)
    assert(!sys.read_text("files/data\\0suffix").ok)
    assert(sys.remove_file("files/data").ok)
    assert(sys.remove_dir("files").ok)
}
''')
    p.call("run", "filesystem.ddr")


def mac_bundle(p):
    name = "Å & Tools"
    app = p.root / (name + ".app")
    p.write(f"{app.name}/Contents/Resources/a & b", "preserved")
    p.write("bundle.ddr", "import sys\nfn main()->i32{print(sys.arg(1));return 17}")
    result = p.call("run", "bundle.ddr", "--app", "-o", app, "--", "macOS", status=17)
    assert result.stdout == "macOS\n"
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    assert info["CFBundleDisplayName"] == name and info["CFBundlePackageType"] == "APPL"
    assert (app / "Contents/MacOS" / info["CFBundleExecutable"]).is_file()
    subprocess.run(["/usr/bin/codesign", "--verify", "--strict", app], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    compiler = parser.parse_args().compiler.resolve(strict=True)
    cases = [modules_and_manifest, targets_and_cache, phase_timings, scalar_optimizer_controls, isolated_tests, filesystem]
    if sys.platform == "darwin":
        cases.append(mac_bundle)
    for case in cases:
        with tempfile.TemporaryDirectory(prefix="dunder-cli-") as directory:
            case(Project(compiler, Path(directory)))
        print("PASS", case.__name__, flush=True)


if __name__ == "__main__":
    main()
