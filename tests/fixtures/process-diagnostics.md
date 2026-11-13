The original full native test result of 133 passes and one Vulkan-group
exit126 remains **INCONCLUSIVE**. These fixtures test failure observability;
they do not reproduce or establish the cause of that earlier failure.

Run `python3 tools/test-process-diagnostics.py /absolute/path/to/dunder`.
The script builds all three native targets with inline off/on, executes the
host images, and on Darwin uses a separate C dyld interposer for controlled
foreign-call failures. Production code has no fault controls or added retries.
The host matrix has 31 cases per inline setting, including normal output,
ordinary child126, missing executable127, invalid NUL argv, signals, timeout,
closed stderr pipes, errno lookup failure, and the existing wait retry policy.

`run` retains its signature and numeric exit results. `run_detailed` exposes
an `Outcome` for callers needing attribution. `process.tests` retains normal
PASS output and the old `(exit N)` substring, appending the stage, errno,
raw wait-status slot, pid, and auxiliary errno fields on FAIL. An ordinary
child exit126 is `child_exit`, regardless of its numeric value. Auxiliary
fields retain the last failed call, not the last successful call. Errno zero
can mean unavailable access; it does not turn a failure into success.

Errno TLS lookup happens in the parent before clock start/fork; the temporary
`dlopen(NULL)` handle is closed there. The TLS slot belongs to permanently
loaded libc, as in the existing bundle filesystem implementation. Each failed
foreign call is followed by a raw errno load before another foreign call.
The clock wrappers return immediately on failure without intervening foreign
calls or managed cleanup.

The child error path uses preallocated fixed buffers and raw pointers/scalars.
Its decimal formatting needs at most 59 bytes of a 128-byte buffer, including
the longest prefix, signed i32 and newline. It calls only async-signal-safe
`sigprocmask`, `write`, and `_exit`; it performs no loader, allocator, managed
string, or normal logging operations. SIGPIPE is blocked only after a pre-exec
failure, immediately before reporting and exit. A failed mask operation skips
the best-effort write and preserves126/127. Successful exec and the parent keep
their original signal masks. The stderr note is deliberately best effort and
is not a reliable child-to-parent protocol.

The signal-set backing is zeroed, 8-byte aligned, and128 bytes long. The sole
set bit is SIGPIPE13. Darwin headers define a 4-byte sigset_t and SIG_BLOCK1;
the C fixture statically asserts these values against the host SDK. Linux
glibc's [signal-set definition](https://raw.githubusercontent.com/bminor/glibc/glibc-2.42/sysdeps/unix/sysv/linux/bits/types/__sigset_t.h)
has1024 bits on both supported64-bit ABIs;
[SIG_BLOCK](https://raw.githubusercontent.com/bminor/glibc/glibc-2.42/sysdeps/unix/sysv/linux/bits/sigaction.h)
is0 and [SIGPIPE](https://raw.githubusercontent.com/bminor/glibc/glibc-2.42/bits/signum-generic.h)
is13. The Linux targets are cross-build checks on a Darwin host, not claims of
Linux runtime execution.
