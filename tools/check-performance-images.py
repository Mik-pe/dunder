#!/usr/bin/env python3
"""Untimed preflight: identical normal images from two native compilers.

Uses the existing performance corpus/results from the explicitly chosen source
checkout. Checks four runtime kernels plus generated/selfhost/Tabell consumers.
No C compiler, latency observer or incremental cache is used.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

ENV = {'PATH': '/no-external-tools'}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('baseline', type=Path)
    parser.add_argument('candidate', type=Path)
    parser.add_argument('report', type=Path)
    parser.add_argument('--source-root', type=Path, required=True,
                        help='same frozen feature source checkout for both compilers')
    parser.add_argument('--baseline-sha256')
    args = parser.parse_args()
    source_root = args.source_root.resolve(strict=True)
    compilers = {name: getattr(args, name).resolve(strict=True) for name in ('baseline', 'candidate')}
    hashes = {name: sha(path) for name, path in compilers.items()}
    if args.baseline_sha256 and hashes['baseline'] != args.baseline_sha256:
        parser.error('baseline compiler hash differs from required frozen binary')
    report = args.report.resolve(); report.mkdir(parents=True, exist_ok=True)
    if any(report.iterdir()):
        parser.error('report directory must be empty')
    helper_path = source_root / 'tools/performance.py'
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location('dunder_performance', helper_path)
    helper = importlib.util.module_from_spec(spec); spec.loader.exec_module(helper)
    before = helper.source_hashes(); helper_hash = sha(helper_path); tool_hash = sha(Path(__file__))
    metadata = dict(complete=False, source_root=str(source_root), source_hashes=before,
                    performance_tool_sha256=helper_hash, tool_sha256=tool_hash,
                    compilers={name: dict(path=str(path), sha256=hashes[name]) for name, path in compilers.items()},
                    environment=ENV, cases={}, commands=[], scope='untimed image identity and exact consumer results; no C comparison')
    try:
        with tempfile.TemporaryDirectory(prefix='dunder-performance-images-') as directory:
            work = Path(directory).resolve()

            def invoke(command):
                result = subprocess.run([str(x) for x in command], cwd=work, env=ENV,
                                        capture_output=True, timeout=180)
                metadata['commands'].append(dict(command=[str(x) for x in command], status=result.returncode,
                                                stdout=result.stdout.decode(errors='replace'), stderr=result.stderr.decode(errors='replace')))
                if result.returncode or result.stderr:
                    raise ValueError((command, result.returncode, result.stdout, result.stderr))
                return result.stdout

            system, machine = platform.system(), platform.machine()
            if system == 'Darwin' and machine == 'arm64': entry = 'main'
            elif system == 'Linux' and machine in ('aarch64', 'arm64'): entry = 'linux'
            elif system == 'Linux' and machine == 'x86_64': entry = 'x86_linux'
            else: raise ValueError('unsupported native host')
            generated, metadata['generated_corpus'] = helper.corpus(work)
            cases = [(name, source_root / 'benchmarks/runtime' / (name + '.ddr'), inputs,
                      (str(expected) + '\n').encode()) for name, inputs, expected in helper.runtime_cases()]
            cases += [('generated', generated, [], b'595500\n'),
                      ('selfhost', source_root / 'selfhost' / (entry + '.ddr'), [], None)]
            for name, source, inputs, expected in cases:
                images = {}
                for variant, compiler in compilers.items():
                    image = work / (name + '-' + variant)
                    invoke([compiler, 'build', source, '-o', image])
                    images[variant] = dict(sha256=sha(image), bytes=image.stat().st_size)
                    if name == 'selfhost':
                        hello = work / ('hello-' + variant)
                        invoke([image, 'build', source_root / 'examples/hello.ddr', '-o', hello])
                        if invoke([hello]) != b'42\ntrue\n': raise ValueError('selfhost consumer result changed')
                    elif invoke([image, *inputs]) != expected:
                        raise ValueError((name, variant, 'independent consumer result changed'))
                metadata['cases'][name] = images
                if images['baseline'] != images['candidate']:
                    raise ValueError((name, 'normal emitted image differs; investigate before timing claims'))
                print('IDENTICAL', name, images['baseline']['sha256'], flush=True)
        if (helper.source_hashes() != before or sha(helper_path) != helper_hash or sha(Path(__file__)) != tool_hash or
                any(sha(path) != hashes[name] for name, path in compilers.items())):
            raise ValueError('source/compiler/tool changed during preflight')
        metadata['complete'] = True
    except Exception as error:
        metadata['error'] = repr(error)
        raise
    finally:
        (report / 'metadata.json').write_text(json.dumps(metadata, indent=2, sort_keys=True) + '\n')


if __name__ == '__main__':
    main()
