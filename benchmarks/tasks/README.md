# Source task throughput

These two programs measure source language tasks on ARM64 macOS. They do not
measure parallel compiler speedup and do not change the four-kernel C acceptance
suite. Capacity 1 is one-worker task scheduling with real OS thread creation,
not a direct-call serial baseline.

Each case builds **one binary**. Runtime capacity 1,2,4 is the only differing
input during its comparison. All capacities execute 32 jobs in 8 batches of 4
named TaskIDs. They use identical input values, kernel code, work counts and
result checks. Batching bounds the live snapshots and cached results. Typed key arrays
are also supported by the language. Workers have no IO, ambient state or nested task scheduling.

| Case | Work | Independent reference |
| --- | --- | --- |
| cpu | A record supplies a distinct seed and step count. Each worker advances an integer recurrence and returns its full sum and final state. | Python arbitrary-precision multiplication/remainder and sum, checking both fields of all 32 results. |
| array | A record supplies a primitive array snapshot, job index and pass count. Each worker copies and updates its private array and returns its weighted sum, first and last value. | Python lists and integer arithmetic for every element/pass. All result fields and the parent's unchanged original checksum/endpoints are checked. |

The `*_worker.ddr` modules are ordinary old-syntax Dunder. The correctness tool
can compile direct-call consumers of them with the frozen serial compiler;
these are untimed checks, not another throughput baseline.

Whole-process samples include process startup, argument parsing, input setup,
ordinary value copies, task-boundary clones, OS thread creation, work, joins,
printing and scope teardown. No clock/profile observer runs inside the program.
Filesystem caches may be warm. OS scheduling, core placement and frequency are
uncontrolled; the report records host/hardware and paired samples.

## Correctness without timing

```sh
python3 tools/performance-tasks.py check /path/to/task-compiler \
  --reference-compiler /path/to/whole-function-objects-final \
  --report build/task-throughput-check
```

This checks small and zero-work inputs at all 3 capacities, including empty and
single-element arrays. There are no timing calls in check mode.

## One sizing observation, then freeze

`workloads.json` deliberately starts with `frozen:false`. Proposed work counts
are 500000 steps/job and 192 passes over 1024 elements/job. These are not measured
calibration results. Stop competing builds and tests before sizing and running a series.

```sh
python3 tools/performance-tasks.py calibrate /path/to/task-compiler \
  build/task-throughput-sizing --target-ms 250
```

Sizing uses capacity 1 only and produces common work counts for all capacities.
It records one minimum-work observation, retaining clones/threads/setup/checksum,
and one full-work observation after checked warmups. They are sizing data, not a
latency distribution. Review `recommended-workloads.json`, retain its provenance,
and set `frozen:true`. Do not adapt counts per capacity or after seeing results.
The 250 ms target is proposed to amortize scheduling; whether it does so remains
an observed sizing result, not an assumption of task speedup.

```sh
python3 tools/performance-tasks.py measure /path/to/task-compiler \
  build/task-throughput-30 --manifest /path/to/frozen-workloads.json --rounds 30
```

Each case records 30 fresh processes per capacity, rotating all 6 orders so each
capacity occupies each position 10 times. One checked, unrecorded fresh process
per capacity precedes recording. Exact independent output is required for every
sample. `samples.csv` retains round/order/time/image identity; `summary.csv`
contains median, nearest-rank p95 and median per-round speedup over capacity 1.
`metadata.json` preserves inputs, commands/results, source/compiler/image hashes,
flags, environment, hardware and limits. No geometric aggregate or C claim is
produced. Source/compiler/manifest/image changes fail the run.

## Separate serial compiler gate

Run the existing compiler harness from the **same frozen feature source
checkout** for both binaries; its script location determines the source root:

```sh
python3 /path/to/feature-checkout/tools/performance.py \
  /path/to/whole-function-objects-final /path/to/frozen-task-compiler \
  /path/to/empty/compiler-report --suite compiler --include-check --rounds 30
```

The serial baseline must have SHA256
`2a0a78477985a6f7542cd90278743bde96d5591bd8e6f50d0c2b702f55dc6020`.
Generated modules, the actual selfhost compiler and Tabell are built and checked
in alternating baseline/candidate order, including check-only cases. Samples
use default safety checks, fresh processes, fresh outputs, no incremental cache
and no timing/profile flags. Report every median/p95 overhead, including small
regressions. Generated compilation must retain the 100000 lines/s p95 gate.

Before recording, compile the unchanged 4 runtime kernels with both compilers
and compare images, and require matching baseline/candidate hashes for all
normal emitted compiler-suite images. If normal images differ, investigate
before claiming semantic-hook overhead only. No runtime C timing rerun is
needed when images are identical. Compiler timing and task throughput are
separate observations and must be reported separately.

The untimed preflight enforces that image gate and validates every consumer:

```sh
python3 tools/check-performance-images.py /path/to/whole-function-objects-final \
  /path/to/frozen-task-compiler /path/to/empty/image-report \
  --source-root /path/to/feature-checkout \
  --baseline-sha256 2a0a78477985a6f7542cd90278743bde96d5591bd8e6f50d0c2b702f55dc6020
```
