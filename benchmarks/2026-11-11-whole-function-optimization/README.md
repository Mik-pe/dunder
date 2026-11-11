# Whole-function optimization milestones

These measurements preserve successive architectural comparisons on the unchanged
runtime and compiler cohorts. The latest verified default profile is
[final-serial-objects](final-serial-objects/summary.csv), described below. Each directory records exact compiler and source
hashes, flags, 30 fresh process samples, median, p95 and image size. Incremental
caching was disabled and normal safety checks remained enabled. C used Clang
`-O3 -ftrapv`; its programs do not have Dunder array bounds checks.

| Comparison | Numeric off/baseline → on/candidate median | Runtime candidate/C geometric mean | Generated build off/baseline → on/candidate | Self-build off/baseline → on/candidate |
| --- | --- | --- | --- | --- |
| Same compiler, frame pass off/on, inline off | 7.639 → 7.534 ms | 1.355 | 565.326 → 569.728 ms | 438.489 → 481.801 ms |
| Same compiler, frame pass off/on, inline on | 7.497 → 6.438 ms | 1.270 | 597.823 → 597.953 ms | 465.522 → 514.654 ms |
| Pre-MIR compiler → combined default profile | 6.941 → 5.909 ms | 1.304 | 332.541 → 565.250 ms | 224.147 → 463.532 ms |

Numbers across rows are separate measurement series. Compare paired variants
within a row. The combined profile is the actual default configuration at that
milestone; the 1.270 ablation result is not a substitute for its 1.304 ratio.
The combined profile's worst runtime/C ratio was 1.412 (text). Its generated
build p95 was 569.485 ms (177,395 source lines/s), and self-build p95 466.466 ms.

Inlining exposes real callee bodies to the general frame pass. On the unchanged
numeric loop, the frame pass replaced eight of nine static FP-relative loads
and all four static FP-relative stores when inlining was enabled. No runtime
benefit is attributed to scan/sort comparisons with identical output images.

These candidates precede the raw-frame entry initialization fix and explicit
source-object memory context. They are historical evidence, not validation of
the later profile. A regression discovered after these measurements showed
that initializing a promoted home with zero changed raw frame observations;
that initialization has since been replaced with a load of the original bits.
No C-level performance claim follows from these results: the architecture still
has a substantial compile-time cost and runtime remains above the C reference.


## Source-memory and value passes: full-plan candidate

The `memory-values-full-plan` ablation uses the same frozen compiler
`a6833f5784f29ed944ccabbffdc50d007a611622c230bf832897f5ad2bf747a8`
on both sides. Baseline disables `--mir-memory` and `--mir-values`; candidate
uses the complete default profile. Both keep bounded inlining and generic
frame promotion enabled. This is an intermediate, correctness-verified
candidate, withheld from publication as the default because build throughput
fails the target.

| Case | Both new mechanisms off | Both on | On / C |
| --- | --- | --- | --- |
| Numeric runtime median | 5.650 ms | 5.048 ms | 1.098 |
| Scan runtime median | 3.105 ms | 2.584 ms | 1.051 |
| Text runtime median | 3.725 ms | 3.504 ms | 1.361 |
| Sort runtime median | 3.171 ms | 3.045 ms | 1.291 |
| Generated build median | 564.572 ms | 2049.406 ms | — |
| Self-build median | 502.662 ms | 1488.135 ms | — |
| Tabell build median | 14.104 ms | 36.553 ms | — |

Runtime/C geometric mean is 1.193, worst case 1.361. Generated build p95
is 2071.989 ms, only 48,757 source lines/s; self-build p95 is 1500.290 ms.
Check-only remains unchanged. Runtime p95 has considerable process-latency
variation in this series and is retained in the raw summary; median gains
must not be read as a claim that every tail-latency metric improved.

The unchanged-source probe independently checks 18 native outputs with the
value pass off/on. Numeric has 63 operand/value rewrites and 35 deleted pure
materializations; text has 68 and 39. These counts and byte sizes describe the
complete emitted source image, not a precise inner-loop dump. Their original
Wasm builds require native `sys.arg` and are explicitly BLOCKED; separate
portable value and memory fixtures execute on Wasm.

Full `tools/verify-selfhost.sh` passed, including the bootstrap fixed point,
138 native dogfood groups, 124 Wasm groups with 14 explicit native-only groups,
50 diagnostics, 56 check/build contracts and 46 runtime contracts per runtime.
The separate value matrix built 256 images and executed 128 host/Wasm cases,
with 232 independent oracle pairs and 24 declared call/trap coverage gaps.

The principal next correction is structural: replace the full Function plan
and exact full snapshots with a single exclusive optimization transaction and
a sparse undo arena. Preserve the same dataflow, canonical contracts, external
exit observations and mandatory final verification. No reduced safety profile
or benchmark-specific eligibility rule is used to recover build time.


## Exclusive transaction: build-cost gate

`exclusive-compiler` compares the full-plan a683 candidate with the exclusive
sparse-undo compiler
`f9c3f38774332306e3dce4755e7898c59c17b4d8e6928a2033032e0cc00485bc`.
Both use the complete profile and compile the same new source checkout. The
generated and Tabell images are byte-identical; self-host images differ, and
both produced compilers successfully build and run the checked hello program.
The sparse candidate reproduces its own frozen image. This is a 30-round
compiler-only gate; it is not a new runtime measurement.

| Build | Full-plan median | Exclusive median | Exclusive p95 |
| --- | --- | --- | --- |
| Generated | 2038.260 ms | 1572.310 ms | 1584.977 ms |
| Self-host | 1474.853 ms | 1176.074 ms | 1187.099 ms |
| Tabell | 35.243 ms | 28.208 ms | 31.197 ms |

The generated build improves 22.9%, but 63,738 lines/s at p95 still fails the
100,000 target. Check-only remains comparable. This candidate therefore stays
behind the build-cost gate. Targeted verification passed 30 native and 30 Wasm
unit groups, 256 image comparisons (all exactly match the previous value
engine), 128 host/Wasm executions and a bootstrap fixed point. Full correctness
verification must run again on the candidate that clears the remaining gate.

The next structural correction projects the MUST fact domain onto the GP
registers that can actually participate in this function's value operations.
Implicit ABI live-outs still feed full physical liveness, while unknown writes
and calls kill every affected tracked fact. This reduces per-block analysis
state without reducing the language or introducing source-name rules.


The separate `exclusive-attribution` observation uses one clock/allocation
pair per case, not a latency series. Observer variants emitted identical
program images. The generated case attributes 1218.584 ms to the MIR pipeline;
self-host attributes 897.232 ms and Tabell 21.606 ms. These instrumented phase
clocks identify where to investigate, not acceptance times.

Allocation/ARC counts cover the entire compiler invocation, unlike the phase
clock. In particular, the 39.3 million generated-case release calls include
null/immortal checks: the counter increments before these early returns. A
source audit found no descriptor retain/release for borrowed field indexing
and no large value-array iteration snapshots in current MIR modules. Therefore
these counters do not establish ARC overhead inside the MIR value solver;
the compact-domain change targets its actual per-block/register state.


## Compact fact domain: build-cost gate

`compact-compiler` uses 30 attempts per variant with the exclusive compiler
as baseline and compact compiler
`20747ca7918906b7958feacc692a0a6f3f92998533038153e0b73088e76b300d`
as candidate. Both use the same complete profile and source checkout.

| Build | Exclusive median | Compact median | Compact p95 |
| --- | --- | --- | --- |
| Generated | 1590.552 ms | 1052.696 ms | 1068.744 ms |
| Self-host | 1179.310 ms | 843.187 ms | 864.602 ms |
| Tabell | 28.876 ms | 20.907 ms | 23.966 ms |

The generated build improves 33.8% but still narrowly misses 100,000 lines/s.
This remains a build-cost gate, not an acceptance claim. Compact/full-domain
verification passed 36 native and 36 Wasm groups, 384 images and 192 executions.
128 pairs have identical semantic MIR, optimization counters and images; all
256 historical baseline/on hashes are unchanged. In that finite corpus the
compact domain has 2–6 slots and charges 576,780 aggregate units against
1,491,644 for full domains. These are structural work counts, not latency.

A following identity-transfer correction skips fact-table access when the
complete GP write/clobber projection is empty. SP, FP and NZCV remain separate;
actual GP W writes retain their zero-extension effects. All 384 image,
counter, unit and oracle records remain exactly unchanged by that correction.


## Sparse transfers: complete-profile check before local certificates

`sparse-pre-certification` compares the frozen pre-MIR compiler with
`8e6c7426dc7115d39ce0e199097196baa14354d81bb046ca78cbcaa2c47d2e1a`.
This 30-round series includes C and the complete default profile. All four
candidate runtime images exactly match the full-plan ablation images, so the
analysis/transaction changes have preserved that generated code.

Runtime medians (baseline → candidate / C) are numeric 6.592 → 5.073 / 4.589 ms,
scan 3.060 → 2.530 / 2.472 ms, text 3.756 → 3.461 / 2.586 ms and sort
3.147 → 2.941 / 2.379 ms. Candidate/C geometric mean is 1.170 and worst
case 1.338. This still fails the C acceptance goal.

Generated build median is 1009.245 ms, p95 1021.580 ms: narrowly below the
100,000 lines/s requirement. Self-build median/p95 are 822.790/827.235 ms;
Tabell 23.005/29.734 ms. Relative to the original compiler, build cost remains
substantial (3.10x generated median, 3.44x self-build). These regressions are
retained rather than hidden behind the later improvements from the expensive
full-plan candidate. A following inductive row certificate removes a redundant
whole-function pre-DCE verification only for strictly locally validated edits;
unconditional final verification and the conservative fallback remain.


## Locally checked certificates: serial build gate passed

`certified-before-objects` is the complete-profile 30-round comparison of the
pre-MIR compiler with
`e311e94bc42d58d967319be0ae78ffb4b9a8a0c5e5faaa816994c2d13558065d`.
Generated build median/p95 are 926.366/936.691 ms: **107,852 source lines/s at
p95**, clearing the generated build target. Self-build is 746.583/753.359 ms;
Tabell 20.944/28.598 ms. Relative to the original compiler, medians still cost
2.88x generated and 3.14x self-build. Check-only is comparable.

All four runtime images still exactly match the full-plan candidate. In this
series candidate/C geometric mean is 1.175 and worst case 1.346 (text).
The C runtime goal therefore remains unmet. These are the same machine-code
images as earlier series, with fresh samples rather than selected best C times.
The private certificate rule retained the same existing rewrites and mandatory
final verification; targeted coverage passed 40 native and 40 Wasm groups plus
384 target images and 192 executions. The newly integrated FunctionObject
adapter is verified and measured separately before final publication.


## Final serial profile with detached function objects

`final-serial-objects` measures source revision `6c7df69` with frozen compiler
`2a0a78477985a6f7542cd90278743bde96d5591bd8e6f50d0c2b702f55dc6020`
against the pre-MIR compiler
`4b73d04b90e252741bc39e926a4c03b775710a6cfdaece1dc29dd413a39cf105`.
Both use their normal complete profile. There is one build worker, 30 fresh
processes per case/variant, warm OS cache and no Dunder artifact cache. This is
still a serial compiler; detached objects prepare independent function jobs.

| Runtime | Baseline median | Candidate median | C median | Candidate / C |
| --- | --- | --- | --- | --- |
| Numeric | 6.626 ms | 5.079 ms | 4.644 ms | 1.094 |
| Scan | 3.064 ms | 2.530 ms | 2.447 ms | 1.034 |
| Text | 3.734 ms | 3.435 ms | 2.592 ms | 1.325 |
| Sort | 3.089 ms | 2.916 ms | 2.345 ms | 1.243 |

All four runtime images exactly match the expensive full-plan candidate.
The runtime/C geometric mean is **1.168**, worst case **1.325** (text): the
1.10/1.25 C acceptance target remains unmet. Medians improve 23.3%, 17.4%,
8.0% and 5.6% against the original compiler. C uses `-O3 -ftrapv` but does
not include Dunder's array bounds checks. Raw p95 values remain in the report;
process-latency variation does not imply uniform tail improvements.

| Build | Baseline median | Candidate median | Candidate p95 |
| --- | --- | --- | --- |
| Generated | 320.242 ms | 925.403 ms | 936.195 ms |
| Self-host | 238.060 ms | 745.986 ms | 749.256 ms |
| Tabell | 9.343 ms | 21.176 ms | 28.677 ms |

The generated corpus reaches **107,909 source lines/s at p95**, passing the
100,000 build-throughput gate. Compared with the original compiler, build
medians still cost 2.89x generated and 3.13x self-host. Check-only medians are
comparable: generated 184.841 → 184.146 ms, self-host 157.418 → 157.886 ms
and Tabell 5.000 → 5.006 ms. These are measured costs, not a claim that all
incremental, memory or broad workload acceptance criteria are met.

Full `tools/verify-selfhost.sh` passed on this source. The retained log includes
the bootstrap fixed point, 138 native dogfood groups, 124 Wasm groups with 14
explicit native-only groups, 50 diagnostics, 56 all-target check/build
contracts and 46 runtime contracts per native/Wasm runtime. Value coverage
includes 40 native plus 40 Wasm unit groups, 384 target images, 192 executions,
348 independent oracle pairs and 36 explicit call/trap gaps. Detached objects
include 13 unit groups, 16 serial/object image comparisons and 8 native/Wasm
executions. Cross-target image builds do not claim Linux execution.

Next architecture work separates function verification/optimization/lowering
from the single-owner frontend and merges relocatable results in stable source
order. Native task-provider and task-boundary analyses are being developed
separately; source-language threading and parallel compilation are not enabled
by this measured milestone. Single-thread runtime still needs better value and
place representation, allocation and memory-versioned load reuse to close the
text/sort gap to C.
