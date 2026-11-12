#!/usr/bin/env python3
"""Four-runtime-only untimed image/result preflight; no clock/profile calls."""
from pathlib import Path
import hashlib
import importlib.util
import json
import platform
import subprocess
import sys

sys.dont_write_bytecode = True
REPORT = Path(__file__).resolve().parent
ROOT = REPORT.parents[2]
ENV = {'PATH': '/no-external-tools'}
assert {p.name for p in REPORT.iterdir()} == {'preflight.py'}, 'preserve existing evidence'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


spec = importlib.util.spec_from_file_location('runtime_preflight_performance', ROOT / 'tools/performance.py')
perf = importlib.util.module_from_spec(spec)
spec.loader.exec_module(perf)
compilers = {'baseline': ROOT / 'build/ssa-pruned/baseline',
             'candidate': ROOT / 'build/selfhost/arm64-macos/dunder'}
expected = {'baseline': '5f92b8d8fb9980f861664d21717664ac0f581e4ce76817e6580509b9b1846295',
            'candidate': '9e5b774adff87835bbdf1cee07f19c2801facd6d4f08a1148d5a79c5bbe04de1'}
assert all(sha(path) == expected[name] for name, path in compilers.items())
source_before = perf.source_hashes()
tool_before = {str(p): sha(p) for p in (Path(__file__), ROOT / 'tools/performance.py')}
metadata = {'protocol': 'four-runtime-untimed-images-v1', 'complete': False,
            'scope': 'Default SSA off, default four workers, ordinary safety; no clocks/profilers/timing series.',
            'controller_argv': [sys.executable, *sys.argv], 'host': platform.uname()._asdict(),
            'checkout': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
            'working_tree': subprocess.check_output(['git', 'status', '--short'], cwd=ROOT, text=True),
            'environment': ENV, 'compiler_flags': {'baseline': [], 'candidate': []},
            'compilers': {name: {'path': str(path), 'sha256': expected[name]} for name, path in compilers.items()},
            'source_sha256': source_before, 'tools_sha256': tool_before, 'cases': {}, 'commands': []}
(REPORT / 'raw').mkdir()
(REPORT / 'images').mkdir()


def call(command):
    result = subprocess.run(list(map(str, command)), cwd=ROOT, env=ENV,
                            capture_output=True, timeout=180)
    index = len(metadata['commands'])
    for channel in ('stdout', 'stderr'):
        (REPORT / 'raw' / f'{index:03}.{channel}').write_bytes(getattr(result, channel))
    metadata['commands'].append({'argv': list(map(str, command)), 'cwd': str(ROOT), 'status': result.returncode,
                                'stdout': f'raw/{index:03}.stdout', 'stderr': f'raw/{index:03}.stderr'})
    assert result.returncode == 0 and result.stderr == b'', (command, result.returncode, result.stdout, result.stderr)
    return result.stdout


try:
    for name, arguments, answer in perf.runtime_cases():
        source = ROOT / 'benchmarks/runtime' / (name + '.ddr')
        result = {'source': str(source), 'source_sha256': sha(source), 'arguments': arguments,
                  'expected_stdout': str(answer) + '\n', 'images': {}}
        metadata['cases'][name] = result
        for variant, compiler in compilers.items():
            image = REPORT / 'images' / (name + '-' + variant)
            call([compiler, 'build', source, '-o', image])
            assert call([image, *arguments]) == (str(answer) + '\n').encode(), (name, variant)
            result['images'][variant] = {'path': str(image), 'sha256': sha(image), 'bytes': image.stat().st_size}
        images = result['images']
        result['images_equal'] = images['baseline']['sha256'] == images['candidate']['sha256']
        print(name, 'IDENTICAL' if result['images_equal'] else 'CHANGED',
              images['baseline']['sha256'], images['candidate']['sha256'], flush=True)
    assert perf.source_hashes() == source_before
    assert all(sha(path) == expected[name] for name, path in compilers.items())
    assert all(sha(Path(path)) == digest for path, digest in tool_before.items())
    metadata['off_runtime_series_required'] = any(not row['images_equal'] for row in metadata['cases'].values())
    metadata['complete'] = True
except Exception as error:
    metadata['error'] = repr(error)
    raise
finally:
    (REPORT / 'metadata.json').write_text(json.dumps(metadata, indent=2, ensure_ascii=False, sort_keys=True) + '\n')
