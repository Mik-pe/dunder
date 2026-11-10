#!/usr/bin/env python3
"""Exercise native file grants, module authority and directory cleanup."""
import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile

FIXTURES = Path(__file__).resolve().parents[1] / "tests/fixtures/fs_handles"
REJECTIONS = [
    ('x := fs.grant_read("other")', "capability filesystem"),
    ('target := fs.grant_write', "capability filesystem"),
    ('forged := fs.WriteFile { path: "other" }', "opaque struct"),
    ('path := input.path', "opaque struct"),
    ('input.path = "other"', "opaque struct"),
    ('change(&mut input.path)', "opaque struct"),
    ('result := fs.write(&input, "wrong grant")', "type mismatch"),
    ('unsafe { p := ptr(0) }', "capability unsafe"),
]


def check_grants(compiler, project):
    for source in FIXTURES.glob("*.ddr"):
        shutil.copyfile(source, project / source.name)
    content = "hämtad 🌍\n"
    (project / "källa.txt").write_text(content, encoding="utf-8")
    image = project / "copy"
    build = [str(compiler), "build", "main.ddr", "-o", str(image)]
    subprocess.run(build, cwd=project, check=True, timeout=30)
    previous = image.read_bytes()
    subprocess.run([image], cwd=project, check=True, timeout=15)
    if (project / "resultat.txt").read_bytes() != (content + "✓\n").encode():
        raise AssertionError("destination contents differ from delegated read/write")
    if (project / "arbete").exists() or (project / "källa.txt").read_text() != content:
        raise AssertionError("directory cleanup or source preservation failed")
    worker = project / "worker.ddr"
    source = worker.read_text()
    for body, message in REJECTIONS:
        # Placing the attempted access in an unused function checks the owner's
        # authority even while the optimizer removes the function from the image.
        worker.write_text(source + '\nfn change(value:&mut string){}\n'
                          + 'fn other(input:&mut fs.ReadFile){ ' + body + ' }\n')
        for command in ([str(compiler), "check", "main.ddr"], build):
            result = subprocess.run(command, cwd=project, capture_output=True,
                                    text=True, timeout=30)
            if result.returncode != 1 or "worker.ddr:" not in result.stderr or message not in result.stderr:
                raise AssertionError(f"{body!r}: {result.returncode} {result.stderr}")
        if image.read_bytes() != previous:
            raise AssertionError("a failed build replaced the valid native image")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    compiler = parser.parse_args().compiler.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="dunder-files-") as directory:
        check_grants(compiler, Path(directory))
    print(f"PASS delegated file copy, directory cleanup and {len(REJECTIONS)} authority checks")


if __name__ == "__main__":
    main()
