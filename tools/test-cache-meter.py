#!/usr/bin/env python3
"""Check nested cache accounting and byte identity, without latency claims."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
STAGES = ('identity_serialize', 'identity_hash', 'metadata_read', 'content_hash',
          'hex_decode', 'validation', 'commit', 'serialization', 'publication')
PARTITION = ('declarations', 'model_parse', 'verify_pre', 'simplify', 'verify_post',
             'allocation', 'scalar_emit', 'legacy_body', 'fragment_cache', 'finalize', 'range_analysis')


def cache_detail(data):
    parent = data['compiler_detail']
    assert len(parent['phases']) == 11 and set(parent['phases']) == set(PARTITION)
    assert parent['covered'] == sum(parent['phases'].values())
    assert parent['covered'] + parent['other'] == data['phases']['compiler']
    detail = parent['cache_detail']
    assert detail['enabled'] is True and detail['clock_ok'] is True and detail['unit'] == 'ns'
    assert detail['overlaps_parent'] == 'fragment_cache' and detail['included_in_covered'] is False
    assert set(detail['phases']) == set(detail['calls']) == set(STAGES)
    assert set(detail['work']) == {s + ('_records' if s == 'validation' else '_words' if s == 'commit' else '_bytes') for s in STAGES}
    assert all(type(value) is int and value >= 0 for category in ('phases', 'calls', 'work') for value in detail[category].values())
    assert type(detail['total']) is int and detail['total'] >= 0
    assert detail['total'] == sum(detail['phases'].values()) <= parent['phases']['fragment_cache']
    for stage in STAGES:
        unit = '_records' if stage == 'validation' else '_words' if stage == 'commit' else '_bytes'
        if detail['calls'][stage] == 0:
            assert detail['phases'][stage] == detail['work'][stage+unit] == 0
    return detail


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--node', default='node')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    node = shutil.which(args.node)
    assert node, 'Node is required for Wasm execution'
    host = 0 if platform.system() == 'Darwin' else (1 if platform.machine() in ('arm64', 'aarch64') else 2)
    images = executions = 0
    with tempfile.TemporaryDirectory(prefix='dunder-cache-meter-') as temporary:
        work = Path(temporary).resolve()
        source = work / 'main.ddr'
        source.write_text('fn inc(n:i32)->i32{n+1}\nfn main(){print(inc(41))}\n')

        def call(command, cwd, expected=None):
            result = subprocess.run([str(v) for v in command], cwd=cwd,
                                    env={'PATH': '/no-external-tools'}, capture_output=True, timeout=120)
            assert result.returncode == 0, (command, result.returncode, result.stdout, result.stderr)
            if expected is not None:
                assert result.stdout == expected, (command, result.stdout, expected)
            return result

        for target, target_name in enumerate(('arm64-macos', 'arm64-linux', 'x86_64-linux', 'wasm32')):
            for inline in ('off', 'on'):
                cwd = work / f'{target}-{inline}'; cwd.mkdir()
                plain = cwd / 'plain'
                call([compiler, 'build', source, '-o', plain, '--target', target_name, '--inline-opt='+inline], cwd)
                expected = plain.read_bytes()
                for label in ('cold', 'warm', 'timings-only', 'plain-incremental', 'invalid-cache'):
                    if label == 'invalid-cache' and target != 3:
                        files = sorted((cwd / 'build/.dunder-cache').rglob('*.fn'))
                        assert files
                        path = files[0]; original = path.read_bytes()
                        values = [int(original[i:i+8], 16) for i in range(128, len(original), 8)]
                        values[2 if values[0] == 0x44554644 else 0] = 0
                        payload = ''.join(f'{v:08x}' for v in values).encode()
                        key = original[:64]
                        path.write_bytes(key + hashlib.sha256(key+payload).hexdigest().encode() + payload)
                    image = cwd / label
                    extra = [] if label == 'plain-incremental' else ['--timings=json']
                    if label not in ('plain-incremental', 'timings-only'):
                        extra += ['--profile-compiler']
                    result = call([compiler, 'build', source, '-o', image, '--target', target_name,
                                   '--inline-opt='+inline, *(['--incremental'] if target != 3 else []), *extra], cwd)
                    assert image.read_bytes() == expected, (target, inline, label, 'instrumentation changed code')
                    images += 1
                    if label == 'plain-incremental':
                        assert result.stderr == b''
                    else:
                        data = json.loads(result.stderr)
                        if label == 'timings-only':
                            assert 'compiler_detail' not in data
                        else:
                            detail = cache_detail(data)
                            if target == 3:
                                assert detail['total'] == 0 and all(v == 0 for v in detail['calls'].values())
                            else:
                                assert detail['calls']['metadata_read'] > 0
                                if label == 'warm':
                                    assert data['functions']['reused'] > 0
                                    assert detail['calls']['hex_decode'] > 0 and detail['calls']['commit'] > 0
                                    assert detail['calls']['serialization'] == detail['calls']['publication'] == 0
                                if label == 'invalid-cache':
                                    assert detail['calls']['validation'] > detail['calls']['commit']
                                    assert detail['calls']['publication'] > 0
                    if target in (host, 3):
                        command = [image] if target == host else [node, '-e',
                            "const fs=require('node:fs');const p=new WebAssembly.Instance(new WebAssembly.Module(fs.readFileSync(process.argv[1])),{dunder:{print_i32:console.log}}).exports;process.exitCode=p.main()||0", image]
                        call(command, cwd, b'42\n'); executions += 1
                print('PASS cache-meter', target_name, inline, 'code identity, cold/warm/failure accounting, parent partition', flush=True)
    print(f'{images} unchanged images; {executions} host/Wasm executions; nested counts excluded from 11-phase covered total')


if __name__ == '__main__':
    main()
