#!/usr/bin/env python3
"""Build real suffix rewrites for all targets; execute host/Wasm and trap cases."""
import argparse
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('compiler', type=Path)
args = parser.parse_args()
compiler = args.compiler.resolve(strict=True)
host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)

def code_size(image, target):
    # Mach-O pages and signing padding can leave total file size unchanged.
    # Inspect its actual __text section rather than mistaking padding for code.
    if target != 0:
        return len(image)
    at = 32
    for _ in range(int.from_bytes(image[16:20], 'little')):
        command = int.from_bytes(image[at:at + 4], 'little')
        size = int.from_bytes(image[at + 4:at + 8], 'little')
        if command == 25:
            for section in range(int.from_bytes(image[at + 64:at + 68], 'little')):
                start = at + 72 + section * 80
                if image[start:start + 16].rstrip(b'\0') == b'__text':
                    return int.from_bytes(image[start + 40:start + 48], 'little')
        at += size
    raise AssertionError('missing Mach-O __text section')

with tempfile.TemporaryDirectory(prefix='dunder-machine-ir-') as directory:
    work = Path(directory)
    harness = work / 'emitter'
    build = subprocess.run([compiler, 'build', ROOT / 'tests/fixtures/machine-ir-emitter.ddr', '-o', harness], capture_output=True, timeout=60)
    assert build.returncode == 0, build.stderr.decode(errors='replace')
    for expected in (42, 41):
        for target in range(4):
            images = {}
            for mode in ('none', 'compact', 'split'):
                image = work / f'{expected}-{target}-{mode}'
                result = subprocess.run([harness, image, str(target), mode, str(expected)], capture_output=True, timeout=10)
                assert result.returncode == 0, (expected, target, mode, result.stderr, result.stdout)
                removed = int(result.stdout.strip())
                assert removed == (0 if mode == 'none' else 8), (target, mode, removed)
                images[mode] = image.read_bytes()
                if target == host:
                    run = subprocess.run([image], capture_output=True, timeout=10)
                    assert run.returncode == (0 if expected == 42 else 104), (expected, target, mode, run.returncode, run.stderr)
                if target == 3:
                    run = subprocess.run(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], capture_output=True, timeout=10)
                    assert run.returncode == (0 if expected == 42 else 125), (expected, target, mode, run.returncode, run.stderr)
            assert images['compact'] == images['split'], (expected, target, 'split/convenience image mismatch')
            reduction = code_size(images['none'], target) - code_size(images['compact'], target)
            assert reduction > 0, (expected, target, 'code did not shrink')
            if target in (0, 1):
                assert reduction == 32, (expected, target, reduction)
            print('PASS', f'expected={expected}', f'target={target}', 'real words removed; split/convenience identical', flush=True)
    for mode in ('duplicate-code', 'duplicate-imports', 'grow-code', 'grow-imports'):
        result = subprocess.run([harness, work / mode, str(host), mode, '42'], capture_output=True, timeout=10)
        assert result.returncode != 0, (mode, 'invalid commit lifecycle accepted')
        print('PASS', mode, 'rejected', flush=True)
print('All four target images built; host and Wasm executed success/trap cases; invalid commits rejected')
