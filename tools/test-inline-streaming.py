#!/usr/bin/env python3
"""Verify bounded inline UTF-8 iteration through ordinary source compilation."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')


def call(command, status=0, output=None, cwd=None):
    result = subprocess.run(list(map(str, command)), capture_output=True, timeout=60, cwd=cwd)
    assert result.returncode == status, (command, result.returncode, status,
                                         result.stdout, result.stderr)
    if output is not None:
        assert result.stdout == output, (command, result.stdout, output)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    with tempfile.TemporaryDirectory(prefix='dunder-inline-stream-') as directory:
        work = Path(directory)
        profiles = []
        for target in range(4):
            for mode in ('off', 'on'):
                image = work / f'control-{target}-{mode}'
                record = call([compiler, 'build', ROOT/'tests/fixtures/inline-streaming.ddr',
                               '--inline-opt=' + mode, '--target', TARGETS[target],
                               '--timings=json', '-o', image])
                data = json.loads(record.stderr)['inlining']
                if mode == 'on':
                    assert data['entries'] == 0 and data['calls'] >= 5, data
                    assert 0 < data['words'] <= data['estimated_words'] <= 65536, data
                else:
                    assert data['calls'] == data['words'] == data['estimated_words'] == 0, data
                if target == host:
                    call([image], output=b'42\n')
                elif target == 3:
                    call(['node', ROOT/'tools/runtime-contract-wasm.cjs', image, '--repeat'], output=b'42\n')
        for mode in ('off', 'on'):
            image = work / ('profile-' + mode)
            call([compiler, 'build', ROOT/'tests/fixtures/inline-streaming.ddr',
                  '--inline-opt=' + mode, '--runtime-profile', '-o', image])
            result = call([image], output=b'42\n')
            profiles.append(json.loads(result.stderr))
        assert profiles[0] == profiles[1], ('iteration changed executed runtime traffic', profiles)
        # High physical slots exercise cursor-preserving frame helpers. The
        # separate large caller crosses the shared growth budget into fallback.
        large = work/'large-frame.ddr'
        large.write_text('fn main(){' + ''.join(f'var v{i}:={i};' for i in range(600)) +
                         'var sum:=0;for scalar in "A🌍".runes(){sum=sum+i32(scalar)};'
                         'assert(sum==127822);print(42)}')
        bounded = work/'caller-budget.ddr'
        bounded.write_text('fn main(){var sum:=0;' +
                           'for scalar in "A".runes(){sum=sum+i32(scalar)};' * 140 +
                           'assert(sum==9100);print(42)}')
        for source in (large, bounded):
            for target in range(4):
                for mode in ('off', 'on'):
                    image = work/f'{source.stem}-{target}-{mode}'
                    result = call([compiler, 'build', source, '--inline-opt=' + mode,
                                   '--target', TARGETS[target], '--timings=json', '-o', image])
                    data = json.loads(result.stderr)['inlining']
                    if mode == 'on':
                        assert 0 < data['words'] <= data['estimated_words'] <= 4096, data
                        if source == bounded:
                            assert 0 < data['calls'] < 140, data
                    if target == host:
                        call([image], output=b'42\n')
                    elif target == 3:
                        call(['node', ROOT/'tools/runtime-contract-wasm.cjs', image], output=b'42\n')
        # A cached builtin-only expansion has no declaration IDs, but its
        # budget charge must still be replayed without emitting new inline code.
        cached = work/'cached.ddr'
        cached.write_text('fn sum(text:string)->i32{var n:=0;for scalar in text.runes(){n=n+i32(scalar)};n}'
                          'fn main(){print(sum("A🌍"))}')
        for target in range(3):
            images, records = [], []
            for round in range(2):
                image = work/f'cached-{target}-{round}'
                result = call([compiler, 'build', cached, '--inline-opt=on', '--incremental',
                               '--target', TARGETS[target], '--timings=json', '-o', image], cwd=work)
                images.append(image.read_bytes()); records.append(json.loads(result.stderr))
                if target == host:
                    call([image], output=b'127822\n')
            assert images[0] == images[1]
            assert records[0]['inlining']['calls'] == 1 and records[1]['inlining']['calls'] == 0, records
            assert records[0]['inlining']['estimated_words'] == records[1]['inlining']['estimated_words'] > 0, records
            assert records[1]['functions']['reused'] > 0, records
        # A body trap occurs after decoding exactly its current scalar.
        trap = work/'trap.ddr'
        trap.write_text('fn main(){print(17);for scalar in "🌍".runes(){assert(false)};print(42)}')
        for target in range(4):
            for mode in ('off', 'on'):
                image = work/f'trap-{target}-{mode}'
                call([compiler, 'build', trap, '--inline-opt=' + mode,
                      '--target', TARGETS[target], '-o', image])
                if target == host:
                    call([image], status=104, output=b'17\n')
                elif target == 3:
                    call(['node', ROOT/'tools/runtime-contract-wasm.cjs', image], status=125, output=b'17\n')
    print('inline streaming: all targets, mutation snapshots, nesting, continue/break/return, '
          'NUL and UTF-8 boundaries, large frames, budget fallback, cached builtin charges, '
          'repeated Wasm and unchanged runtime traffic passed')


if __name__ == '__main__':
    main()
