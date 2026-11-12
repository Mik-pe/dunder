# Isolated language tasks: correctness and cost

Source revision `b00b14f`, ARM64/macOS. The frozen candidate is
`598cfb372d932795c5a55825a41a73e43b02c0b29b719ab2af1cd09e2b4d5c91`;
the preceding serial compiler is
`2a0a78477985a6f7542cd90278743bde96d5591bd8e6f50d0c2b702f55dc6020`.
All series use the normal complete safety/optimization profile. Source,
compiler, tool, input and image hashes are retained with raw samples.

This milestone implements [source-language tasks](../../docs/tasks.md).
The compiler still runs its own function passes serially. Language-task
throughput below is a separate observation from serial compiler cost and
from the four-program C acceptance suite.

## Serial compiler cost: gate passed

Both compilers build the same feature source checkout, using 30 fresh
processes per variant/case, warm OS file cache, no Dunder artifact cache
and one build worker. Timings include publication/signing. All seven ordinary
program images are byte-identical: numeric, scan, text, sort, generated,
selfhost and Tabell. Independent consumer results passed. No fresh C timing
series was run because the runtime images are unchanged.

| Build | Baseline median | Candidate median | Candidate p95 | Median overhead |
| --- | --- | --- | --- | --- |
| Generated | 946.793 ms | 946.924 ms | 953.284 ms | +0.01% |
| Selfhost | 816.913 ms | 820.711 ms | 831.154 ms | +0.46% |
| Tabell | 20.064 ms | 20.198 ms | 23.251 ms | +0.67% |

Generated p95 reaches **105,975 source lines/s**, above the 100,000 gate.
Build p95 overheads are +0.16%, +1.26% and +11.07%; Tabell's absolute p95
increases 2.317 ms. Check-only medians increase 1.59%, 1.72% and 1.92%;
p95 increases 1.65%, 1.17% and 5.93%. These regressions remain in the report.
There is no claim that every latency metric is unchanged or improved.

The old compiler also emits exactly the new candidate when compiling the
feature source, confirming compatibility with the serial bootstrap grammar.
The full frozen-seed bootstrap independently reached its fixed point.

## Source tasks: paired whole-process throughput

Each case uses one binary with identical inputs and work counts. Runtime
capacity 1, 2 or 4 is the only changed input. All capacities execute 32 jobs
in eight batches of four, including input setup, ordinary copies, boundary
clones, real OS thread creation, computation, joins, output and scope teardown.
Capacity 1 still uses task threads; it is not a direct-call serial baseline.

| Case | Capacity 1 median / p95 | Capacity 2 median / p95 | Capacity 4 median / p95 | Paired median speedup at 2 / 4 |
| --- | --- | --- | --- | --- |
| CPU recurrence | 251.960 / 258.072 ms | 130.201 / 135.328 ms | 70.359 / 74.531 ms | 1.936x / **3.587x** |
| Private array updates | 240.427 / 248.874 ms | 123.268 / 130.838 ms | 63.608 / 70.967 ms | 1.951x / **3.783x** |

There are 30 fresh processes per capacity/case, 180 recorded processes total.
All six capacity permutations rotate evenly: each capacity occupies each
position ten times per case. Every sample has exact independent Python
reference output. Parent input snapshots are checked unchanged in the array
case. Raw per-round paired ratios, medians and nearest-rank p95 are retained.

One capacity-1 sizing observation preceded the series. Minimum-work cost was
2.907/2.938 ms, below 10% of the 250 ms sizing target. Common counts were
then frozen: CPU 2,054,162 steps/job; array 3,111 passes/job over 1,024 elements.
Original proposals, observations, recommendations and the frozen manifest
are retained. Counts were never adapted per capacity or after the series.
Hardware, OS, source/image hashes and environment are in metadata. Core
placement, frequency and other OS scheduling were uncontrolled.

These finite pure CPU workloads demonstrate useful language parallelism.
They do not establish parallel compiler speedup, a direct-call speedup, C-level
code quality, broad application performance or peak memory acceptance.

## Verification

Full `tools/verify-selfhost.sh` passed, including frozen-seed bootstrap,
138 native dogfood groups, 124 Wasm groups with 14 explicit native-only groups,
50 diagnostics, 56 all-target check/build contracts and 46 runtime contracts
per native/Wasm runtime. Existing MIR/scalar/value/ownership/CLI contracts
remain in the retained full log.

New source coverage includes 21 positive Data/lifetime programs, 25 rejected
programs checked through build and check, five actual status-112 traps,
namespace shadowing, scalar/inline flag variants and incremental helper edits.
Task programs conservatively bypass fragment reuse. Native provider coverage
executes 14 real OS images, and scope scheduling executes another 22. A
separate C/assembly oracle checks ABI, overlap, bounded peak workers,
exact-once joins/reclamation, all-joins-before-parent-drop and finite OS
failure paths. Poisoned index/Entry pointers prove wrong generation/type
rejection occurs before dereference; address reuse does not revive old IDs.
Wasm unit tests exercise metadata and code generation, **not Wasm threads**.

The inactive ordered FunctionObject linker additionally passed 32 serial/link
image comparisons and 16 native/Wasm executions. Its global label/import-site
ownership and source order are ready for the next compiler integration;
frontend sessions and compiler job dispatch remain to be connected.
