#!/usr/bin/env python3
"""Detached final-object cache writes and cache-hit capture without MIR replay.

Compare serial native3/DUFD bytes, delayed object bytes, and restored/linked
images on every target. Execute native/Wasm results; malformed capture/save
inputs are rejected atomically. This is a functional test, never a timing run.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import sys

sys.dont_write_bytecode = True
import wasm_host

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
SOURCES = ('selfhost/mir_lower.ddr', 'selfhost/fragments.ddr',
           'tests/mir_fragment.ddr', 'tests/fixtures/mir-fragment-emitter.ddr')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--source-root', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path, default=ROOT/'build/mir-fragment-qa')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True); root = args.source_root.resolve()
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=True)
    host = {'Darwin': {'arm64': 0}, 'Linux': {'aarch64': 1, 'arm64': 1, 'x86_64': 2}}[platform.system()][platform.machine()]
    log, images = [], []; executions = payloads = 0
    hashes = {p: hashlib.sha256((root/p).read_bytes()).hexdigest() for p in SOURCES}

    def run(command):
        command = list(map(str, command))
        result = subprocess.run(command, cwd=root, capture_output=True, text=True, timeout=120)
        log.append({'command': command, 'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
        assert result.returncode == 0, log[-1]
        return result.stdout

    def check_payload(path, extended):
        data = path.read_bytes(); key, signature, raw = data[:64], data[64:128], data[128:]
        assert signature == hashlib.sha256(key+raw).hexdigest().encode(), path
        assert len(raw) % 8 == 0
        n = [int(raw[i:i+8], 16) for i in range(0, len(raw), 8)]
        if extended:
            assert n[:2] == [0x44554644, 1] and n[5:7] == [2, 10]
            assert n[-2:] == [2, 8]
            words, labels, relocs = n[2:5]; header = 7
            assert len(n) == 7+words+labels+3*relocs+2
        else:
            words, labels, relocs = n[:3]; header = 3
            assert len(n) == 3+words+labels+3*relocs
        # Each body has an actual end label and an unmarked private label;
        # the future function/callback namespace must never leak into a cache.
        expected_labels = 3 if path.name.endswith('.a') else 2
        assert labels == expected_labels, (path, labels)
        offsets = n[header+words:header+words+labels]
        assert words in offsets and 0xffffffff in offsets, (path, offsets)
        for i in range(relocs):
            at, target, kind = n[header+words+labels+3*i:header+words+labels+3*i+3]
            assert at < words and 0 <= kind <= 20
            assert target < labels or 0x80000000 <= target < 0x80000003
        return data

    js_bin = wasm_host.js_runtime()
    try:
        run([compiler, 'test', 'tests/mir_fragment.ddr', '-o', output/'unit'])
        for target in TARGETS:
            unit = output/('unit-'+target)
            run([compiler, 'build-tests', 'tests/mir_fragment.ddr', '--target', target, '-o', unit])
            if target == 'wasm32': run([js_bin, 'tools/test-wasm.mjs', '--execute', unit])
        emitter = output/'emitter'
        run([compiler, 'build', 'tests/fixtures/mir-fragment-emitter.ddr', '-o', emitter])
        assert run([emitter, 'invalid', output/'invalid.fn', host, 'unused', 'ordinary', 0]).strip() == 'objects=2'
        for target, name in enumerate(TARGETS):
            for extended in (False, True):
                fmt = 'inline' if extended else 'ordinary'
                for padding in (0, 1):
                    pair = []
                    base = output/f'{name}-{fmt}-{padding}'
                    for mode in ('serial', 'object', 'restore'):
                        image = output/f'{name}-{fmt}-{padding}-{mode}'
                        assert run([emitter, mode, base, target, image, fmt, padding]).strip() == 'objects=2'
                        for suffix in ('a', 'b'):
                            direct = check_payload(Path(str(base)+'.serial.'+suffix), extended)
                            detached = check_payload(Path(str(base)+'.'+suffix), extended)
                            assert direct == detached, (name, fmt, padding, suffix, 'serial/object payload')
                            if mode == 'restore':
                                replayed = check_payload(Path(str(base)+'.roundtrip.'+suffix), extended)
                                assert detached == replayed, (name, fmt, padding, suffix, 'restore/capture payload')
                            payloads += 1
                        data = image.read_bytes(); pair.append(data)
                        images.append({'target': name, 'format': fmt, 'padding': padding, 'mode': mode,
                                       'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
                        if target == host:
                            assert run([image]) == ''; executions += 1
                        elif target == 3:
                            run([js_bin, 'tools/runtime-contract-wasm.cjs', image]); executions += 1
                    assert pair[0] == pair[1] == pair[2], (name, fmt, padding, 'serial/object/cache image')
                # Restore the same cache into a different prefix/private-label
                # base, then compare with the independently compiled reference.
                rebased = output/f'{name}-{fmt}-rebased-restore'
                base0 = output/f'{name}-{fmt}-0'
                run([emitter, 'restore', base0, target, rebased, fmt, 1])
                assert rebased.read_bytes() == (output/f'{name}-{fmt}-1-serial').read_bytes()
                images.append({'target': name, 'format': fmt, 'padding': 1, 'mode': 'rebased-restore',
                               'bytes': rebased.stat().st_size, 'sha256': hashlib.sha256(rebased.read_bytes()).hexdigest()})
                if target == host: assert run([rebased]) == ''; executions += 1
                elif target == 3: run([js_bin, 'tools/runtime-contract-wasm.cjs', rebased]); executions += 1
        assert hashes == {p: hashlib.sha256((root/p).read_bytes()).hexdigest() for p in SOURCES}, 'source changed during run'
        report = {'result': 'passed', 'images': images, 'executions': executions, 'payload_comparisons': payloads,
                  'scope': 'inactive cache-object API; trusted semantic producer, structural capture only; no MIR on hit',
                  'compiler_sha256': hashlib.sha256(compiler.read_bytes()).hexdigest(), 'source_sha256': hashes}
        (output/'summary.json').write_text(json.dumps(report, indent=2)+'\n')
        print(f'PASS detached cache objects: {len(images)} images, {executions} executions, {payloads} exact payload comparisons; atomic slice/save rejection')
    finally:
        (output/'commands.json').write_text(json.dumps(log, indent=2)+'\n')


if __name__ == '__main__':
    main()
