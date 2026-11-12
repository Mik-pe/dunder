#!/usr/bin/env python3
"""Untimed paired allocation observations of one compiler source, two producers."""
from pathlib import Path
import hashlib
import importlib.util
import json
import statistics
import subprocess
import sys

sys.dont_write_bytecode = True
REPORT = Path(__file__).resolve().parent
ROOT = REPORT.parents[2]
ENV = {'PATH': '/no-external-tools'}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


perf = module('attribution_performance', ROOT / 'tools/performance.py')
reader = module('attribution_reader', ROOT / 'tools/profile-compiler.py')
producers = {
    'old': ROOT / 'build/compiler-dogfood/task-scopes-integrated',
    'new': ROOT / 'build/compiler-dogfood/session-record-serial',
}
expected_producers = {
    'old': '598cfb372d932795c5a55825a41a73e43b02c0b29b719ab2af1cd09e2b4d5c91',
    'new': '126788e56a5d81a9273410d375bc231e231fe24d38fba61fb432f73a5688e6f2',
}
assert {p.name for p in REPORT.iterdir()} == {'observer.py'}, 'preserve existing evidence'
for name, producer in producers.items():
    assert sha(producer) == expected_producers[name]
sources_before = perf.source_hashes()
tools_before = {str(path): sha(path) for path in
                (Path(__file__), ROOT / 'tools/performance.py',
                 ROOT / 'tools/profile-compiler.py', ROOT / 'tools/profile-dogfood.py')}
source = ROOT / 'selfhost/main.ddr'
metadata = {
    'protocol': 'compiler-codegen-producer-allocation-v1', 'complete': False,
    'rounds': 5, 'environment': ENV, 'source_entry': str(source),
    'source_hashes': sources_before, 'tools_sha256': tools_before,
    'checkout': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
    'working_tree': subprocess.check_output(['git', 'status', '--short'], cwd=ROOT, text=True),
    'producers': {name: {'path': str(path), 'sha256': sha(path)} for name, path in producers.items()},
    'commands': [], 'observers': {}, 'samples': [],
    'scope': [
        'Same current serial compiler algorithm/source compiled by old and new producer code generation.',
        'Whole process executed allocator/ownership events, including setup/frontend/Session/pipeline/image/publication.',
        'No elapsed-time observer, --timings or --profile-compiler is used for the observed builds.',
        'This is not an old-versus-new compiler algorithm comparison or MIR-only attribution.',
        'retain/release entries include null and immortal handles; byte counters are traffic, not peak/live memory.',
        'Old producer current-source output need not equal the old producer executable; no fixedpoint guard is weakened.',
    ],
}
(REPORT / 'raw').mkdir()
(REPORT / 'outputs').mkdir()
for name in producers:
    (REPORT / 'observers' / name).mkdir(parents=True)
    (REPORT / 'work' / name).mkdir(parents=True)


def call(command, cwd):
    index = len(metadata['commands'])
    result = subprocess.run([str(x) for x in command], cwd=cwd, env=ENV,
                            capture_output=True, timeout=180)
    stdout = REPORT / 'raw' / f'{index:04}.stdout'
    stderr = REPORT / 'raw' / f'{index:04}.stderr'
    stdout.write_bytes(result.stdout); stderr.write_bytes(result.stderr)
    metadata['commands'].append({'command': [str(x) for x in command], 'cwd': str(cwd),
                                 'status': result.returncode,
                                 'stdout': str(stdout.relative_to(REPORT)),
                                 'stderr': str(stderr.relative_to(REPORT))})
    assert result.returncode == 0, (command, result.returncode, result.stdout, result.stderr)
    return index, result


try:
    corpus, info = perf.corpus(REPORT)
    metadata['generated_corpus'] = info
    metadata['generated_sources_sha256'] = {str(p.relative_to(REPORT)): sha(p) for p in sorted(corpus.parent.glob('*.ddr'))}
    observers = {}
    for name, producer in producers.items():
        observer = REPORT / 'observers' / name / 'dunder'
        _, result = call([producer, 'build', source, '-o', observer, '--runtime-profile'], REPORT / 'work' / name)
        assert result.stderr == b''
        observers[name] = observer
        metadata['observers'][name] = {'path': str(observer), 'sha256': sha(observer),
                                       'bytes': observer.stat().st_size,
                                       'source_entry_sha256': sha(source),
                                       'producer_flags': ['--runtime-profile']}
        print('BUILT allocation observer', name, flush=True)
    image_digest = None
    for round_number in range(5):
        order = ('old', 'new') if round_number % 2 == 0 else ('new', 'old')
        for position, name in enumerate(order):
            image = REPORT / 'outputs' / f'{name}-{round_number}'
            index, result = call([observers[name], 'build', corpus, '-o', image], REPORT / 'work' / name)
            profile = reader.allocation_profile(result.stderr)
            assert result.stdout == ('built ' + str(image) + '\n').encode()
            digest = sha(image)
            if image_digest is None:
                image_digest = digest
            assert digest == image_digest, (name, round_number, 'consumer image changed')
            _, consumer = call([image], REPORT / 'work' / name)
            assert consumer.stdout == b'595500\n' and consumer.stderr == b''
            metadata['samples'].append({'round': round_number, 'order': position, 'producer': name,
                                         'command_index': index, 'profile': profile,
                                         'consumer_sha256': digest, 'consumer_bytes': image.stat().st_size})
        print('COMPLETE allocation pair', round_number + 1, flush=True)
    assert perf.source_hashes() == sources_before
    assert all(sha(Path(path)) == digest for path, digest in tools_before.items())
    assert all(sha(path) == expected_producers[name] for name, path in producers.items())
    assert all(sha(path) == metadata['observers'][name]['sha256'] for name, path in observers.items())
    assert all(sha(REPORT / path) == digest for path, digest in metadata['generated_sources_sha256'].items())
    summary = {}
    counters = metadata['samples'][0]['profile']['counters']
    for counter in counters:
        values = {name: [sample['profile']['counters'][counter] for sample in metadata['samples']
                         if sample['producer'] == name] for name in producers}
        medians = {name: statistics.median(v) for name, v in values.items()}
        summary[counter] = {'old': {'median': medians['old'], 'min': min(values['old']), 'max': max(values['old'])},
                            'new': {'median': medians['new'], 'min': min(values['new']), 'max': max(values['new'])},
                            'new_over_old': medians['new'] / medians['old'] if medians['old'] else None}
    (REPORT / 'summary.json').write_text(json.dumps(summary, indent=2, sort_keys=True) + '\n')
    metadata['complete'] = True
    metadata['consumer_sha256'] = image_digest
except Exception as error:
    metadata['error'] = repr(error)
    raise
finally:
    (REPORT / 'metadata.json').write_text(json.dumps(metadata, indent=2, sort_keys=True) + '\n')
