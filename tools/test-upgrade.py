#!/usr/bin/env python3
"""Exercise the native updater against an offline GitHub Releases layout."""

import argparse
import gzip
import hashlib
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile


def host():
    machine = platform.machine()
    if sys.platform == "darwin" and machine == "arm64":
        return "arm64-macos"
    if sys.platform == "linux":
        if machine in ("arm64", "aarch64"):
            return "arm64-linux"
        if machine == "x86_64":
            return "x86_64-linux"
    raise RuntimeError(f"unsupported test host: {sys.platform} {machine}")


def release(directory, version, binary, *, archive_bytes=None, archive_sha=None, binary_sha=None):
    directory.mkdir(parents=True, exist_ok=True)
    archive = gzip.compress(binary, mtime=0) if archive_bytes is None else archive_bytes
    (directory / f"{host()}.gz").write_bytes(archive)
    (directory / "release.txt").write_text(
        f"version={version}\n"
        f"{host()}.gz.sha256={archive_sha or hashlib.sha256(archive).hexdigest()}\n"
        f"{host()}.sha256={binary_sha or hashlib.sha256(binary).hexdigest()}\n"
    )


def install_fixture(source, destination):
    # Signed executables need a fresh inode even when restoring a test fixture.
    # An in-place copy can leave stale macOS kernel code-signing state behind.
    descriptor, name = tempfile.mkstemp(prefix=".fixture-", dir=destination.parent)
    os.close(descriptor)
    staging = Path(name)
    try:
        shutil.copy2(source, staging)
        staging.replace(destination)
    finally:
        staging.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    compiler = parser.parse_args().compiler.resolve()
    original = compiler.read_bytes()
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp).resolve()
        root = work / "mirror"
        scratch = work / "tmp"
        scratch.mkdir()
        env = dict(os.environ, DUNDER_RELEASES_URL=root.as_uri() + "/", TMPDIR=str(scratch))
        env.pop("DUNDER_UPGRADE_URL", None)

        def cmd(*args, expect=0):
            res = subprocess.run(args, capture_output=True, text=True, env=env, timeout=30)
            assert res.returncode == expect, (args, res)
            return res

        version_text = cmd(compiler, "--version").stdout
        assert version_text.startswith("dunder ") and version_text.endswith(f" ({host()})\n"), version_text
        assert cmd(compiler, "-V").stdout == version_text
        version = version_text.split()[1]
        # The original executable must remain a wrong-version candidate after
        # every release, rather than eventually matching a fixed fixture tag.
        next_major = int(version.split(".", 1)[0]) + 1
        target_version = f"{next_major}.0.0"
        mismatched_version = f"{next_major + 1}.0.0"
        target_tag = f"v{target_version}"
        target_option = f"--version={target_tag}"
        assert "dunder upgrade" in cmd(compiler, "upgrade", "--help").stdout
        for options in (["--bad"], ["--version"], ["--version="], ["--version", "../bad"],
                        ["--version", "one", "--version", "two"]):
            cmd(compiler, "upgrade", *options, expect=2)
        latest = root / "latest/download"
        exact = root / "download" / target_tag
        missing = cmd(compiler, "upgrade", "--check", target_option, expect=1)
        assert exact.as_uri() + "/release.txt" in missing.stderr, missing
        release(latest, version, original)
        # Checking a local, source-built compiler is supported.
        assert "already up to date" in cmd(compiler, "upgrade", "--check").stdout

        installed = work / "build/selfhost" / host() / "dunder"
        installed.parent.mkdir(parents=True)
        install_fixture(compiler, installed)
        symlink = work / "dunder link"
        symlink.symlink_to(installed)
        initial = installed.stat()

        def unchanged():
            current = installed.stat()
            assert (current.st_dev, current.st_ino) == (initial.st_dev, initial.st_ino)
            assert installed.read_bytes() == original
            assert [p.name for p in installed.parent.iterdir()] == ["dunder"]
            assert not list(scratch.iterdir())
            assert symlink.is_symlink()

        def rejected(message):
            res = cmd(symlink, "upgrade", target_option, expect=1)
            assert message in res.stderr, res
            unchanged()

        def candidate(version):
            source = work / "v2.ddr"
            source.write_text(f'fn main() {{ print("dunder {version} ({host()})") }}\n')
            output = work / "candidate"
            cmd(compiler, "build", source, "-o", output)
            return output.read_bytes()

        new = candidate(target_version)
        release(exact, target_version, new)
        (exact / f"{host()}.gz").unlink()
        result = cmd(symlink, "upgrade", "--check", "--version", target_tag)
        assert f"update available: {version} -> {target_version}" in result.stdout, result
        unchanged()  # --check fetched only the manifest
        rejected("failed to download release archive")
        release(exact, mismatched_version, new)
        rejected("does not match requested tag")
        release(exact, target_version, new, archive_sha="0" * 64)
        rejected("SHA-256 mismatch")
        release(exact, target_version, new, archive_bytes=b"not gzip")
        rejected("failed to decompress")
        release(exact, target_version, new, binary_sha="0" * 64)
        rejected("SHA-256 mismatch")
        release(exact, target_version, b"not executable")
        rejected("failed self-check")
        release(exact, target_version, original)
        rejected("wrong version")
        # Force the identical, but mislabeled, artifact through verification.
        result = cmd(symlink, "upgrade", target_option, "--force", expect=1)
        assert "wrong version or host" in result.stderr, result
        unchanged()

        release(exact, target_version, new)
        # Older installations use an asset directory, which takes precedence
        # over a release root and still honors an explicit version check.
        env["DUNDER_UPGRADE_URL"] = exact.as_uri() + "/"
        direct = cmd(symlink, "upgrade", "--check")
        assert f"-> {target_version} ({host()})" in direct.stdout, direct
        cmd(symlink, "upgrade", "--check", f"--version=v{mismatched_version}", expect=1)
        env.pop("DUNDER_UPGRADE_URL")
        assert "already up to date" in cmd(symlink, "upgrade", "--check", "--force").stdout
        unchanged()
        with installed.open("rb") as previous:
            result = cmd(symlink, "upgrade", target_option)
            assert previous.read() == original, "explicit upgrade modified the previous inode"
        assert f"upgraded {installed} to {target_version} ({host()})" in result.stdout, result
        assert installed.read_bytes() == new and installed.stat().st_ino != initial.st_ino
        assert symlink.is_symlink() and [p.name for p in installed.parent.iterdir()] == ["dunder"]
        assert cmd(symlink, "--version").stdout == f"dunder {target_version} ({host()})\n"

        upgraded = installed.stat()
        with installed.open("rb") as previous:
            install_fixture(compiler, installed)
            assert previous.read() == new, "fixture restore modified the previous inode"
        restored = installed.stat()
        assert (restored.st_dev, restored.st_ino) != (upgraded.st_dev, upgraded.st_ino)
        assert installed.read_bytes() == original
        assert cmd(symlink, "--version").stdout == version_text
        release(latest, target_version, new)
        with installed.open("rb") as previous:
            result = cmd(symlink, "upgrade")
            assert previous.read() == original, "latest upgrade modified the previous inode"
        assert f"to {target_version}" in result.stdout and installed.read_bytes() == new
        published = installed.stat()
        assert (published.st_dev, published.st_ino) != (restored.st_dev, restored.st_ino)
        assert cmd(symlink, "--version").stdout == f"dunder {target_version} ({host()})\n"
        assert symlink.is_symlink() and [p.name for p in installed.parent.iterdir()] == ["dunder"]
        assert not list(scratch.iterdir())
    print("PASS test-upgrade")


if __name__ == "__main__":
    main()
