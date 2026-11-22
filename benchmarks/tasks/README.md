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
| cpu | A record supplies a distinct seed and step count. Each worker advances an integer recurrence and returns its full sum and final state. | The Dunder reference reduces each step modulo 2³¹−1 by folding bits. It checks both fields of all 32 results. |
| array | A record supplies a primitive array snapshot, job index and pass count. Each worker copies and updates its private array and returns its weighted sum, first and last value. | The Dunder reference composes the passes into one affine transform per job. It checks all result fields and the original array's checksum and endpoints. |

The correctness tool can also call the `*_worker.ddr` modules directly using
the compiler passed to `--reference-compiler`. It checks the same expected
results for each worker.

Whole-process samples include process startup, argument parsing, input setup,
ordinary value copies, task-boundary clones, OS thread creation, work, joins,
printing and scope teardown. No clock/profile observer runs inside the program.
Filesystem caches may be warm. OS scheduling, core placement and frequency are
uncontrolled; the report records host/hardware and paired samples.

## Correctness without timing

```sh
dunder run tools/performance_tasks.ddr -- check /path/to/task-compiler \
  --reference-compiler build/selfhost/arm64-macos/dunder \
  --report build/task-throughput-check
```

This checks small and zero-work inputs at all 3 capacities, including empty and
single-element arrays. There are no timing calls in check mode.

## One sizing observation, then freeze

`workloads.json` deliberately starts with `frozen:false`. Proposed work counts
are 500000 steps/job and 192 passes over 1024 elements/job. These are not measured
calibration results. Stop competing builds and tests before sizing and running a series.

```sh
dunder run tools/performance_tasks.ddr -- calibrate /path/to/task-compiler \
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
dunder run tools/performance_tasks.ddr -- measure /path/to/task-compiler \
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

## Comparing compiler output

To check whether a compiler change preserves the generated binaries, run
the untimed image comparison on a single source checkout. The tool builds
the runtime kernels, a generated corpus and the compiler source with both
binaries, runs the programs and compares image hashes.

```sh
dunder run tools/check_performance_images.ddr -- /path/to/baseline \
  /path/to/candidate build/image-report --source-root .
```
