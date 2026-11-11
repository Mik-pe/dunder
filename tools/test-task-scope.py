#!/usr/bin/env python3
"""Actual OS task-scope scheduling, ownership and key validation; no timings."""
import argparse
import importlib.util
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('task_native_probe', ROOT / 'tools/test-task-native.py')
provider = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provider)


def c_probe():
    source = provider.C_PROBE.replace('struct packet {', '''
static _Atomic unsigned active, peak, drops, bad_drop;
static pthread_mutex_t gate_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t gate_cond = PTHREAD_COND_INITIALIZER;
static unsigned arrived;
static int gate_open;
static void begin_active(void) {
    unsigned now = atomic_fetch_add(&active, 1) + 1;
    unsigned old = atomic_load(&peak);
    while (old < now && !atomic_compare_exchange_weak(&peak, &old, now)) {}
    const char *cap = getenv("TASK_SCOPE_CAP");
    if (cap && atoi(cap) >= 2) {
        pthread_mutex_lock(&gate_lock);
        if (!gate_open) {
            if (++arrived == 2) { gate_open = 1; pthread_cond_broadcast(&gate_cond); }
            else while (!gate_open) pthread_cond_wait(&gate_cond, &gate_lock);
        }
        pthread_mutex_unlock(&gate_lock);
    }
}
struct packet {''')
    source = source.replace('atomic_fetch_add(&starts, 1);', 'atomic_fetch_add(&starts, 1); begin_active();')
    source = source.replace('free(p);\n    return NULL;', 'atomic_fetch_sub(&active, 1); free(p);\n    return NULL;')
    source = source.replace('if (fault() == 1) return EAGAIN;', 'if (fault() == 1 && atomic_load(&creates) == 2) return EAGAIN;')
    return source + r'''
void scope_drop(void) {
    atomic_fetch_add(&drops, 1);
    if (atomic_load(&active)) atomic_fetch_add(&bad_drop, 1);
}
int scope_check(int mode, int jobs, int array, int cap, int expected_drops) {
    unsigned c = atomic_load(&creates), j = atomic_load(&joins), u = atomic_load(&unmaps);
    unsigned d = atomic_load(&drops), p = atomic_load(&peak), a = atomic_load(&active);
    if (atomic_load(&bad_drop)) return 1;
    if (mode == 0) {
        if (c != (unsigned)jobs || j != c || u != (array ? c : 0) || a ||
            atomic_load(&starts) != c || atomic_load(&preserved) != c ||
            d != (unsigned)expected_drops || !p || p > (unsigned)cap) return 2;
        if (cap == 1 ? p != 1 : p < 2) return 3;
        return 0;
    }
    // Fatal errors do not unwind. Previously opened scopes may have completed
    // normally, but the failing scope must retain every parent-owned root.
    if (mode == 1) return c != 2 || j || u || d;
    if (mode == 2) return c != 2 || j != 1 || u || d;
    if (mode == 3) return c != 2 || j != 1 || u != 1 || d;
    if (mode == 4) return c != 1 || j || u || d;
    if (mode == 5) return c != 2 || j != 1 || u != (unsigned)array || d != (array ? 2u : 0u);
    if (mode == 6) return c || j || u || d;
    return 4;
}
'''


def call(command, status=0, env=None):
    result = subprocess.run(list(map(str, command)), cwd=ROOT, capture_output=True,
                            text=True, timeout=30, env=env)
    assert result.returncode == status, (command, result.returncode, status, result.stdout, result.stderr)
    assert result.stderr == '', (command, result.stderr)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    if platform.system() != 'Darwin' or platform.machine() not in ('arm64', 'aarch64'):
        raise SystemExit('scheduler execution requires ARM64/macOS; other worker targets are unsupported')
    with tempfile.TemporaryDirectory(prefix='dunder-task-scope-') as directory:
        work = Path(directory)
        emitter = work / 'emitter'
        call([compiler, 'build', ROOT / 'tests/fixtures/task-scope-emitter.ddr', '-o', emitter])
        (work / 'probe.c').write_text(c_probe())
        (work / 'probe.S').write_text(provider.asm_probe())
        library = work / 'scope-probe.dylib'
        call(['clang', '-std=c11', '-O2', '-dynamiclib', work / 'probe.c', work / 'probe.S', '-o', library])
        tests = [(mode, cap, 0) for cap in (1, 2, 4) for mode in ('scalar', 'array', 'unawaited', 'discard')]
        tests += [(mode, 1, 112) for mode in ('wrong-gen', 'wrong-type', 'wrong-index', 'stale', 'owner', 'generation', 'limit')]
        tests += [(mode, 2, 112) for mode in ('create-fail', 'join-fail', 'unmap-fail')]
        for mode, cap, expected in tests:
            image = work / f'{mode}-{cap}'
            call([emitter, image, mode, cap, library])
            env = os.environ.copy()
            env['TASK_SCOPE_CAP'] = str(cap)
            env['TASK_NATIVE_FAULT'] = str({'create-fail': 1, 'join-fail': 2, 'unmap-fail': 3}.get(mode, 0))
            run = call([image], expected, env)
            assert run.stdout == '', (image, run.stdout)
            print(f'PASS {mode} cap{cap}: real OS workers, status{expected}', flush=True)
        print(f'{len(tests)} scheduler images passed; barrier proves actual overlap, peak stays bounded, no elapsed-time claims')


if __name__ == '__main__':
    main()
