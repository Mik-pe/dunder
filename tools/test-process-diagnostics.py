#!/usr/bin/env python3
"""Check process failure attribution without retries or changed exit policies.

The original native suite's isolated exit126 remains INCONCLUSIVE. These tests
verify diagnostic behavior, including synthetic OS failures, not its cause.
Darwin fault injection is test-only dyld interposition; other native targets
are cross-built and normal host behavior is exercised on either OS.
"""
import argparse
import errno
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'tests/fixtures/process-diagnostics.ddr'
TARGETS = {'arm64-macos': 0, 'arm64-linux': 1, 'x86_64-linux': 2}


def call(command, *, env, status=0, stderr=subprocess.PIPE):
    result = subprocess.run([str(x) for x in command], capture_output=False,
                            stdout=subprocess.PIPE, stderr=stderr, env=env,
                            timeout=60, restore_signals=True)
    assert result.returncode == status, (command, result.returncode, status,
                                         result.stdout, result.stderr)
    return result


def outcome(result, **expected):
    lines = [line for line in result.stdout.decode().splitlines() if line.startswith('OUTCOME ')]
    assert len(lines) == 1, (result.stdout, result.stderr)
    actual = dict(field.split('=', 1) for field in lines[0].split()[1:])
    assert len(actual) == 12, actual
    for name, value in expected.items():
        assert actual[name] == str(value), (name, value, actual, result.stderr)
    return actual


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--cc', default=os.environ.get('CC', 'cc'))
    args = parser.parse_args()
    compiler = args.compiler.resolve()
    host = 'arm64-macos' if sys.platform == 'darwin' else (
        'arm64-linux' if platform.machine() in ('arm64', 'aarch64') else 'x86_64-linux')
    env = os.environ.copy()
    for key in ('DYLD_INSERT_LIBRARIES', 'LD_PRELOAD', 'DUNDER_PROCESS_DIAGNOSTIC_FAULT'):
        env.pop(key, None)
    cases = 0
    with tempfile.TemporaryDirectory(prefix='dunder-process-diagnostics-') as temporary:
        work = Path(temporary)
        images = {}
        for mode in ('off', 'on'):
            for target in TARGETS:
                image = work / f'probe-{target}-{mode}'
                call([compiler, 'build', SOURCE, '--target', target,
                      f'--inline-opt={mode}', '-o', image], env=env)
                images[target, mode] = image

        library = work / 'faults.dylib'
        successful_child = work / 'success.sh'
        successful_child.write_text('#!/bin/sh\nexit 0\n')
        successful_child.chmod(0o755)
        if sys.platform == 'darwin':
            call([args.cc, '-std=c11', '-Wall', '-Wextra', '-Werror', '-dynamiclib',
                  ROOT / 'tests/fixtures/process-diagnostics-faults.c', '-o', library], env=env)

        for mode in ('off', 'on'):
            image = images[host, mode]
            missing = work / 'does-not-exist'

            def run(action='detailed', child='quiet-child', *, path=image,
                    timeout=5000, status=0, fault=None, closed_pipe=False):
                nonlocal cases
                selected = env.copy()
                if fault is not None:
                    selected['DYLD_INSERT_LIBRARIES'] = str(library)
                    selected['DUNDER_PROCESS_DIAGNOSTIC_FAULT'] = fault
                command = [image, action, path, child, TARGETS[host], timeout]
                if not closed_pipe:
                    result = call(command, env=selected, status=status)
                else:
                    read, write = os.pipe()
                    os.close(read)
                    try:
                        result = call(command, env=selected, status=status, stderr=write)
                    finally:
                        os.close(write)
                cases += 1
                return result

            normal = run(action='wrapper', child='normal-child')
            assert normal.stdout == b'child-output\n' and normal.stderr == b'child-error\n', normal
            for timeout in (0, 5000):
                normal = run(timeout=timeout)
                record = outcome(normal, code=0, stage='child_exit', errno=0, raw_wait_status=0)
                assert int(record['pid']) > 0 and normal.stderr == b'', normal
            normal = run(action='wrapper', child='mask-child')
            assert normal.stdout == b'child-mask-unchanged\n' and normal.stderr == b'', normal

            exited = run(child='exit126', status=126)
            outcome(exited, code=126, stage='child_exit', errno=0, raw_wait_status=126 << 8)
            assert exited.stderr == b'', exited
            wrapper_exit = run(action='wrapper', child='exit126', status=126)
            assert wrapper_exit.stdout == b'' and wrapper_exit.stderr == b'', wrapper_exit

            failed_exec = run(path=missing, status=127)
            outcome(failed_exec, code=127, stage='child_exit', errno=0, raw_wait_status=127 << 8)
            assert failed_exec.stderr == f'dunder process child stage=execv errno={errno.ENOENT}\n'.encode(), failed_exec
            for action, stage in [('invalid-path', 'argv_path'), ('invalid-arg', 'argv_argument')]:
                invalid = run(action=action, status=126)
                outcome(invalid, code=126, stage=stage, errno=0, pid=0, raw_wait_status=0)
                assert invalid.stderr == b'', invalid

            signaled = run(child='signal-child', status=143)
            outcome(signaled, code=143, stage='signal', errno=0, raw_wait_status=15)
            assert signaled.stderr == b'', signaled
            timed_out = run(child='timeout-child', timeout=20, status=124)
            outcome(timed_out, code=124, stage='timeout', errno=0, raw_wait_status=9)
            assert timed_out.stderr == b'', timed_out

            parent_mask = run(action='parent-mask', path=missing, status=127)
            outcome(parent_mask, code=127, stage='child_exit')
            closed = run(path=missing, status=127, closed_pipe=True)
            outcome(closed, code=127, stage='child_exit', raw_wait_status=127 << 8)

            # Preserve the old "(exit N)" substring used by CLI consumers while
            # appending independently readable stage/errno/raw-status evidence.
            suite = run(action='suite', status=1)
            assert suite.stderr == b'', suite
            assert suite.stdout.startswith(b'FAIL probe_group (exit 126) stage=child_exit errno=0 raw_wait_status=32256 pid='), suite
            assert suite.stdout.endswith(b'0 passed, 1 failed\n'), suite
            successful_suite = run(action='suite', path=successful_child)
            assert successful_suite.stdout == b'PASS probe_group\n1 passed, 0 failed\n' and successful_suite.stderr == b'', successful_suite

            if sys.platform != 'darwin':
                continue
            for fault, stage, error in [('fork', 'fork', errno.EAGAIN),
                                        ('start_clock', 'start_clock', errno.EINVAL)]:
                failed = run(fault=fault, status=126)
                outcome(failed, code=126, stage=stage, errno=error, raw_wait_status=0)
                if fault == 'start_clock':
                    outcome(failed, clock_errno=error)
                assert failed.stderr == b'', failed

            child_group = run(fault='child_group', status=126)
            outcome(child_group, code=126, stage='child_exit', errno=0, raw_wait_status=126 << 8)
            assert child_group.stderr == f'dunder process child stage=child_setpgid errno={errno.EPERM}\n'.encode(), child_group
            closed_group = run(fault='child_group', status=126, closed_pipe=True)
            outcome(closed_group, code=126, stage='child_exit', raw_wait_status=126 << 8)

            # Failed masking must skip write even with a closed reader, then
            # preserve the original _exit code instead of dying from SIGPIPE.
            for fault, path, status in [('child_group_mask', image, 126), ('mask', missing, 127)]:
                for closed_pipe in (False, True):
                    failed = run(fault=fault, path=path, status=status, closed_pipe=closed_pipe)
                    outcome(failed, code=status, stage='child_exit', raw_wait_status=status << 8)
                    if not closed_pipe:
                        assert failed.stderr == b'', failed

            ignored = run(fault='parent_group')
            outcome(ignored, code=0, stage='child_exit', parent_setpgid_errno=errno.EACCES)
            assert ignored.stderr == b'', ignored
            interrupted = run(fault='wait_interrupt')
            outcome(interrupted, code=0, stage='child_exit', wait_errno=errno.EINTR, probe_errno=0)
            assert interrupted.stderr == b'', interrupted
            failed_wait = run(fault='wait_probe', status=126)
            outcome(failed_wait, code=126, stage='wait_probe', errno=errno.ECHILD,
                    wait_errno=errno.ECHILD, probe_errno=errno.ESRCH)
            assert failed_wait.stderr == b'', failed_wait

            failed_clock = run(child='timeout-child', fault='current_clock', status=124)
            outcome(failed_clock, code=124, stage='timeout', errno=errno.EINVAL,
                    clock_errno=errno.EINVAL, raw_wait_status=9)
            fallback = run(child='timeout-child', timeout=20, fault='group_kill', status=124)
            outcome(fallback, code=124, stage='timeout', group_kill_errno=errno.EACCES,
                    fallback_kill_errno=0, raw_wait_status=9)
            failed_fallback = run(child='timeout-child', timeout=20, fault='fallback_kill', status=124)
            outcome(failed_fallback, code=124, stage='timeout', group_kill_errno=errno.EACCES,
                    fallback_kill_errno=errno.EPERM, raw_wait_status=0)
            interrupted_sleep = run(child='timeout-child', timeout=20, fault='sleep_interrupt', status=124)
            outcome(interrupted_sleep, code=124, stage='timeout', sleep_errno=errno.EINTR,
                    raw_wait_status=9)

            no_errno = run(path=missing, fault='no_errno', status=127)
            outcome(no_errno, code=127, stage='child_exit', errno=0, raw_wait_status=127 << 8)
            assert no_errno.stderr == b'dunder process child stage=execv errno=0\n', no_errno

        injection = 'Darwin fault injection passed' if sys.platform == 'darwin' else 'Darwin fault injection not run on this host'
        print(f'process diagnostics: {cases} host cases; all three native targets x inline off/on built; {injection}')


if __name__ == '__main__':
    main()
