#!/usr/bin/env python3
"""Compare shared inline/leaf UTF-8 steps and unchanged creation semantics."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')
TEXTS = ('', 'A\0z\x7f\n\t',
         ''.join(map(chr, (0, 0x7f, 0x80, 0x7ff, 0x800, 0xd7ff, 0xe000,
                          0xffff, 0x10000, 0x10ffff))),
         'é e\u0301 Ελληνικά 🌍', 'ASCII boundary ' * 4,
         ''.join(map(chr, (0x7f, 0x80, 0x7ff, 0x800, 0xd7ff, 0xe000,
                          0xffff, 0x10000, 0x10ffff))))


def call(command, status=0, output=None):
    result = subprocess.run([str(x) if not isinstance(x, bytes) else x for x in command],
                            capture_output=True, timeout=60)
    assert result.returncode == status, (command, result.returncode, status, result.stdout, result.stderr)
    if output is not None:
        assert result.stdout == output, (command, result.stdout, output)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    baseline = args.baseline.resolve(strict=True) if args.baseline else None
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    with tempfile.TemporaryDirectory(prefix='dunder-utf8-next-') as directory:
        work = Path(directory)
        harness = work / 'emitter'
        call([compiler, 'build', ROOT / 'tests/fixtures/utf8-next-emitter.ddr', '-o', harness])
        for selected, text in enumerate(TEXTS):
            source = work / 'text'
            source.write_bytes(text.encode('utf-8'))
            oracle = work / 'expected'
            offset = 0
            lines = []
            for scalar in text:
                offset += len(scalar.encode('utf-8'))
                lines.append(f'{ord(scalar)} {offset}\n')
            oracle.write_text(''.join(lines))
            for source_kind in ('literal', 'slice', 'decoded', 'argv'):
                if source_kind == 'argv' and '\0' in text:
                    continue
                for target in range(4):
                    if target == 3 and source_kind == 'argv':
                        continue
                    profiles = []
                    for mode in ('leaf', 'inline'):
                        image = work / f'{target}-{source_kind}-{mode}'
                        profiling = target == host
                        call([harness, image, target, mode, source, oracle,
                              source_kind, 'profile' if profiling else 'plain'])
                        if target == host:
                            arguments = [text] if source_kind == 'argv' else []
                            run = call([image, *arguments], output=b'')
                            record = json.loads(run.stderr)
                            assert record['kind'] == 'runtime_profile'
                            profiles.append(record)
                            if source_kind == 'literal':
                                assert all(value == 0 for value in record['counters'].values())
                            if source_kind == 'argv' and text:
                                for bad in (b'\xff', b'\xc0\xaf', b'\xe2', b'\xed\xa0\x80', b'\xf4\x90\x80\x80'):
                                    invalid = call([image, bad], status=106, output=b'')
                                    assert not invalid.stderr, 'invalid argv reported after main instead of failing during creation'
                        elif target == 3:
                            call(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], output=b'')
                    if profiles:
                        assert profiles[0] == profiles[1], (selected, source_kind, 'inline added runtime traffic')
            print(f'PASS text {selected}: widths/boundaries, cursor and GPR contract; leaf/inline traffic identical', flush=True)

        consumer = ROOT / 'tests/fixtures/utf8-next-values.ddr'
        for target, name in enumerate(TARGETS):
            image = work / f'consumer-{target}'
            call([compiler, 'build', consumer, '--target', name, '-o', image])
            if baseline:
                old = work / f'baseline-{target}'
                call([baseline, 'build', consumer, '--target', name, '-o', old])
                assert image.read_bytes() == old.read_bytes(), (name, 'default wrapper image changed')
            if target == host:
                call([image], output=b'42\n')
            elif target == 3:
                call(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image, '--repeat'], output=b'42\n')
        for spelling in ('"\\xff"', '"\\xc0\\xaf"', '"\\xe2"', '"\\xed\\xa0\\x80"', '"\\xf4\\x90\\x80\\x80"'):
            source = work / 'invalid.ddr'
            source.write_text('fn main(){for scalar in ' + spelling + '.runes(){print(42)}}')
            rejected = call([compiler, 'check', source, '--agent'], status=1)
            if baseline:
                old = call([baseline, 'check', source, '--agent'], status=1)
                assert rejected.stdout == old.stdout
        source = work / 'bad-boundary.ddr'
        source.write_text('fn main(){for scalar in "å"[1..2].runes(){print(42)}}')
        for target in (host, 3):
            image = work / f'bad-boundary-{target}'
            call([compiler, 'build', source, '--target', TARGETS[target], '-o', image])
            if target == host:
                call([image], status=106, output=b'')
            else:
                call(['node', ROOT / 'tools/runtime-contract-wasm.cjs', image], status=125, output=b'')
    identity = 'default images byte-identical to baseline' if baseline else 'default image comparison not requested'
    print('UTF-8 next: exact bounded vocabulary/check mode, valid sources and creation failures passed; '
          f'{identity}; all targets built, host/Wasm executed, cross-native not executed')


if __name__ == '__main__':
    main()
