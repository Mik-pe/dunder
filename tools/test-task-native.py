#!/usr/bin/env python3
"""Exercise the internal provider on real ARM64/macOS pthreads, without timings.

Production emission has no C shim. The test-only dylib below wraps actual OS
calls, injects finite failure outcomes, and calls the emitted start routine
with an independent assembly C-ABI preservation oracle.
"""
import argparse
import os
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
C_PROBE = r'''
#include <pthread.h>
#include <sys/mman.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>
#include <errno.h>
typedef void *(*start_fn)(void *);
extern int abi_probe(start_fn, void *);
static _Atomic unsigned creates, joins, unmaps, starts, preserved;
struct packet { start_fn start; void *control; };
static int fault(void) {
    const char *value = getenv("TASK_NATIVE_FAULT");
    return value ? atoi(value) : 0;
}
static void *probe_start(void *raw) {
    struct packet *p = raw;
    atomic_fetch_add(&starts, 1);
    int good = abi_probe(p->start, p->control);
    ((uint64_t *)p->control)[9] = good;
    if (good) atomic_fetch_add(&preserved, 1);
    free(p);
    return NULL;
}
int probe_create(pthread_t *thread, const pthread_attr_t *attr, start_fn fn, void *arg) {
    atomic_fetch_add(&creates, 1);
    if (fault() == 1) return EAGAIN;
    struct packet *p = malloc(sizeof(*p));
    if (!p) return ENOMEM;
    *p = (struct packet){ fn, arg };
    int result = pthread_create(thread, attr, probe_start, p);
    if (result) free(p);
    return result;
}
int probe_join(pthread_t thread, void **result) {
    atomic_fetch_add(&joins, 1);
    int error = pthread_join(thread, result);
    // The real join completes so this test process can finish cleanly. The
    // provider receives a failure and must not inspect/reclaim child storage.
    if (!error && fault() == 2) return EINVAL;
    return error;
}
int probe_unmap(void *base, size_t bytes) {
    atomic_fetch_add(&unmaps, 1);
    if (fault() == 3) { errno = EINVAL; return -1; }
    return munmap(base, bytes);
}
int probe_check(int error, int jobs, int array) {
    unsigned expected_starts = error == 1 ? 0 : (unsigned)jobs;
    unsigned expected_unmaps = error == 1 || error == 2 || !array ? 0 : (unsigned)jobs;
    return atomic_load(&creates) != (unsigned)jobs ||
        atomic_load(&joins) != expected_starts ||
        atomic_load(&unmaps) != expected_unmaps ||
        atomic_load(&starts) != expected_starts ||
        atomic_load(&preserved) != expected_starts;
}
'''


def asm_probe():
    lines = ['.text', '.globl _abi_probe', '.p2align 2', '_abi_probe:',
             'stp x29, x30, [sp, #-176]!', 'mov x29, sp']
    for index, reg in enumerate(range(19, 29, 2)):
        lines.append(f'stp x{reg}, x{reg+1}, [sp, #{16+index*16}]')
    for index, reg in enumerate(range(8, 16, 2)):
        lines.append(f'stp d{reg}, d{reg+1}, [sp, #{96+index*16}]')
    lines += ['str x0, [sp, #160]', 'str x1, [sp, #168]', 'mov x16, x0', 'mov x0, x1']
    for reg in range(19, 29):
        lines.append(f'mov x{reg}, #{reg}')
    for reg in range(8, 16):
        lines += [f'mov x10, #{2000+reg}', f'fmov d{reg}, x10']
    lines += ['blr x16', 'cbnz x0, Lbad', 'mov x10, sp', 'cmp x29, x10', 'b.ne Lbad',
              'and x10, x10, #15', 'cbnz x10, Lbad']
    for reg in range(19, 29):
        lines += [f'cmp x{reg}, #{reg}', 'b.ne Lbad']
    for reg in range(8, 16):
        lines += [f'fmov x10, d{reg}', f'cmp x10, #{2000+reg}', 'b.ne Lbad']
    lines += ['mov x0, #1', 'b Ldone', 'Lbad:', 'mov x0, #0', 'Ldone:']
    for index, reg in enumerate(range(8, 16, 2)):
        lines.append(f'ldp d{reg}, d{reg+1}, [sp, #{96+index*16}]')
    for index, reg in enumerate(range(19, 29, 2)):
        lines.append(f'ldp x{reg}, x{reg+1}, [sp, #{16+index*16}]')
    lines += ['ldp x29, x30, [sp], #176', 'ret', '']
    return '\n'.join(lines)


def call(command, *, env=None):
    result = subprocess.run(list(map(str, command)), capture_output=True, text=True,
                            cwd=ROOT, timeout=30, env=env)
    if result.returncode or result.stdout or result.stderr:
        # Compiler/emitter success messages are retained below, never silently
        # retry a failed launch or replace its original outcome.
        if result.returncode:
            raise AssertionError((command, result.returncode, result.stdout, result.stderr))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    if platform.system() != 'Darwin' or platform.machine() not in ('arm64', 'aarch64'):
        raise SystemExit('native task execution requires ARM64/macOS; no cross-target execution claimed')
    with tempfile.TemporaryDirectory(prefix='dunder-task-native-') as directory:
        work = Path(directory)
        emitter = work / 'emitter'
        call([compiler, 'build', ROOT / 'tests/fixtures/task-native-emitter.ddr', '-o', emitter])
        (work / 'probe.c').write_text(C_PROBE)
        (work / 'probe.S').write_text(asm_probe())
        dylib = work / 'probe.dylib'
        call(['clang', '-std=c11', '-O2', '-dynamiclib', work / 'probe.c', work / 'probe.S', '-o', dylib])
        real_modes = ('scalar', 'array', 'batch', 'discard', 'discard-cached')
        probe_modes = (*real_modes, 'create-fail', 'join-fail', 'unmap-fail', 'discard-unmap-fail')
        executed = 0
        for probe, modes in ((False, real_modes), (True, probe_modes)):
            for mode in modes:
                image = work / (mode + ('-probe' if probe else '-real'))
                call([emitter, image, mode, str(dylib) if probe else ''])
                env = os.environ.copy()
                env['TASK_NATIVE_FAULT'] = str({'create-fail': 1, 'join-fail': 2,
                                              'unmap-fail': 3, 'discard-unmap-fail': 3}.get(mode, 0))
                result = call([image], env=env)
                assert result.stdout == result.stderr == '', (image, result.stdout, result.stderr)
                executed += 1
                print(f'PASS {mode}: ' + ('real pthreads + independent C ABI/fault probe' if probe else 'direct system imports'), flush=True)
        print(f'{executed} native images passed; 4-worker isolation, parent/child lifetimes, cached/discarded results, exact-once joins and OS failure states; no timings')


if __name__ == '__main__':
    main()
