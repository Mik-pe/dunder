# Demand-driven SSA: measurement archive

SSA remains **off by default**. Enabling it is rejected for this milestone: the
candidate still adds 21.26% to selfhost and 29.47% to Tabell build medians, without
an established overall runtime gain. The runtime/C geometric mean is 1.184134
with SSA on; the worst case is text at 1.337566. The C goal remains open.

The same-source SSA-on comparison does show a real **27.65% reduction in selfhost
SSA build cost**. It compares the integrated bundle of demand-driven SSA, explicit
source-call inputs and liveness changes with the prior compiler; it does not
isolate causality to one mechanism. Tabell's SSA-on p95 worsens by 20.41%, and the
standard-off selfhost check median rises by 1.95%. These observations are retained.

## Frozen identities and protocol

- Measurement checkout: `9be8668d7b543919cfc2ff0ea05abca946e951f9`.
- Prior compiler: `5f92b8d8fb9980f861664d21717664ac0f581e4ce76817e6580509b9b1846295`.
- Candidate: `9e5b774adff87835bbdf1cee07f19c2801facd6d4f08a1148d5a79c5bbe04de1`.
- SSA module: `8c6d26bf5aa515fed9ca4d0577263f84b7372ffed2fb2576faa757d2c27b2e57`.
- Explicit call-input module: `a550cf6e36f82bb38d469cc258c86cca48f5967dcd4581d9a62f87a0ac3c8602`.
- Harness: `c45fe8b25a2b430ca0e38ee0654ff343459d60e8682532b3a200502b483eb7d5`.

Both compiler hosts consume the **same final source checkout**. Three sequential,
predeclared cohorts ran once, under an exclusive measurement window: prior/new
SSA off (360 samples), prior/new SSA on (360), and the same candidate off/on with
C (720). All 1,440 samples are present, in 48 groups of 30 fresh processes. No
scouting, repeat series, omitted samples or outlier trimming occurred. Capacity
remained four workers. No incremental cache or phase/profile observers were used.
The source/tool/compiler/HEAD/ref guards stayed unchanged throughout measurement.

The first two cohorts each contain three builds and three `check` cases for two
variants. The final cohort adds four runtime cases for SSA off, SSA on and C.
The generated corpus and all workload counts are identical across comparisons.
Compiler variants alternate order; the three runtime variants cycle through all
six permutations. p95 is nearest rank: the 29th sorted observation of 30.

These are process wall times, including startup, with warm OS filesystem caches.
Outputs are checked after the measured build interval; all runtime checksums are
checked against independent Python references. The runtime samples are short and
show substantial tails; [samples](user-profile/samples.csv) retain every value.
No kernel-only speedup or broad C parity follows from these measurements.

**Selfhost means compiling `selfhost/main.ddr`, the serial entry.** It is not a
time for `main_parallel.ddr` or for the complete bootstrap chain. No SSA producer
profile was measured or activated in this wave. Historical v1 build premiums
from [the earlier archive](../2026-11-12-ssa-foundation/README.md) use their own
source freeze; they are not paired ratios for this wave.

The untimed [runtime image preflight](off-runtime-images/metadata.json) ran at
`7c3d5824a43b0904519661cc53136336ff13a261`, before documentation/test-tool-only
changes. Its complete production source and harness hash maps match the timed
freeze exactly. All four default-off runtime images are byte-identical across
hosts and their independent results pass. Therefore no additional off/off
runtime timing series was run. The preflight contains 8 builds and 8 executions,
with all 16 stdout/stderr pairs retained. Executable payloads are omitted from
this archive; their hashes and sizes are preserved in metadata.

## Archived evidence

The original metadata, CSVs, logs, validation JSON, measurement summary and
preflight controller/raw streams are copied byte-for-byte. CSV CRLF is retained.
[commands.json](commands.json) records the completed invocations and sessions;
[archive-manifest.json](archive-manifest.json) maps each measurement/preflight copy to its original
path, size and SHA-256. [checksums.sha256](checksums.sha256) covers the archive
files. The raw metadata intentionally keeps original absolute paths.

[all-cohorts-validation.json](all-cohorts-validation.json) independently checks
sample cardinality, unique rounds, recomputed median/p95, image identities and
cross-cohort consistency. [tails.csv](tails.csv) is a derived 48-row convenience
view with min/median/p95/max; it does not replace the raw samples. All generated
outputs were reproducible within each variant and all consumer checks passed.

[freeze.json](freeze.json) records the verification provenance supplied before
measurement. The completed verification and separate ABI evidence below were
appended after all measurement processes ended.

## Correctness and independent ABI observations

[`verification/final-acceptance.json`](verification/final-acceptance.json)
binds the exact compiler/source identities to the exit-0
[`full verification log`](verification/full-verify-7c3d582.log) at `7c3d582`.
The verified compiler/runtime/frozen-seed/test Dunder sources did not change
before measurement or publication. Documentation and the flag-probe's native
host selection changed afterward; that portable tool was separately rerun
successfully. The standard remains the measured 9e5b image, with SSA off and
four workers. Its serial recovery image is recorded separately.

- [`SSA acceptance`](verification/ssa-integrated-7c3d582.json): 670 images,
  360 native/Wasm executions, 438 independent exact-bit pairs and 232 explicit
  interpreter gaps. Calls, FP, external exits and consumed unknown scratch
  have separate actual-image coverage; they are not counted as interpreter
  proofs. All 61 Function fields are witnessed for budget rollback.
- [`Raw image comparison`](verification/raw-image-comparison.json): all 530
  common full-budget/off images match the archived v1 images byte-for-byte.
  The new flags3 metadata is real; historical exports are not rewritten.
- [`Parallel compiler acceptance`](verification/parallel-integrated-7c3d582.json):
  108 images and 76 actual executions (64 ARM64/macOS, 12 Wasm), with unchanged
  results, diagnostics and code across worker settings. Source SSA acceptance
  also passes 24 images and 17 native/Wasm executions with cache/receipt checks.
- [`Frame/exit acceptance`](verification/frame-exits-integrated-7c3d582.json)
  preserves the independent raw SP/register/frame observers and call fences.
- [`Raw-call flags`](verification/call-flags-portable.json): 48 images and 24
  actual macOS/Wasm executions retain incoming NZCV dependencies. The
  [`historical flags2 receipt`](verification/call-flags-before-v1.json) separates
  a deliberately invalid test-only deletion from the productive real value
  pass, which preserved Compare and observer results both before and after.
  No production miscompile is claimed. Stale flags2 metadata now fails
  re-verification. The earlier full-suite
  [`flag receipt`](verification/call-flags-integrated-7c3d582.json) is also retained.

The [wide sparse-state tests](../../tests/mir_ssa_sparse.ddr) pass in native
and Wasm test drivers (10 groups each, including shared ARM helpers). Their
129 live values exercise complete live-out words and loop transport. A closed
independent interpreter executes final `arm.finish` bytes against independent
arithmetic; these two kernels are not claimed as executed target machine code.
They use an explicit 33554432-unit stress budget. Production stays at 4194304.
Linux targets are image-only throughout these host-specific acceptance reports.

[`Native ABI off/on`](verification/abi/summary.json) uses the same independent
C/assembly sentinel oracle and source, covering X19–X28, D8–D15, FP/SP, a source
callback, promoted internal calls with twelve arguments, text, and a twelve-
argument C call. Both profiles pass. SSA on actually optimizes two functions,
with 11 cells, 14 removed loads and 17 copies. These functional builds are
outside the latency series. Their raw instrumented compiler JSON is preserved
only for policy/codegen counters; incidental clock fields are not analyzed.

The archived `verification/abi/probe.py` assumes a work directory three levels
below the repository root, such as `build/reproduce-ssa/abi`. Copy it and its
sources there before rerunning; it writes a dylib and embeds its absolute path,
so a different directory can change image hashes. Binary/dylib payloads are
omitted. The older `probe-original.py` is preserved as provenance, not the
current reproduction entrypoint.

## Static evidence for the next value backend

The [`text/sort audit`](runtime-gap/README.md) records six exact measured-image
identities, raw disassembly, source snapshots and mechanically checked selected
branch paths. On/C images were reconstructed without executing them, and their
SHA-256 values had to match the timing metadata exactly. The archived
[`classifier`](runtime-gap/classify.py) reproduces the path tables. These are
conditional static counts, not runtime frequencies or instruction-count speedups.

Text still uses the expression stack and a frame bool after the setup calls;
sort's register-resident shift still repeats addresses and a managed load.
The next coherent step is allocatable scalar/expression-stack values with
regional reload/synchronization, plus a distinct ordinary-memory/provenance
contract for safe load reuse. Current `source_objects` does not grant that heap
contract, and calls keep their memory fences. The audit is evidence for this
direction, not a claim that the proposed stage already exists or meets C parity.

To rerun the classifier in a fresh checkout, restore the archived
`user-profile` data under ignored `build/ssa-pruned/user-profile`; it also reads
the repository's earlier selected-path inputs under
`benchmarks/2026-11-11-whole-function-optimization/runtime-attribution`.
The copied original absolute paths remain historical provenance.

## Complete median and p95 results

Frozen source: `9be8668d7b543919cfc2ff0ea05abca946e951f9`. All 1,440 samples validated; 30 fresh processes per case/variant. Source, compiler, tool, HEAD and refs guards unchanged. No reruns or dropped samples.

Baseline compiler `5f92b8d8fb9980f861664d21717664ac0f581e4ce76817e6580509b9b1846295`; candidate `9e5b774adff87835bbdf1cee07f19c2801facd6d4f08a1148d5a79c5bbe04de1`. Same final source checkout for both. Default worker capacity 4. No incremental cache, profiling or phase-clock flags in samples. Warm OS cache.

The selfhost case compiles **selfhost/main.ddr**, the serial entry; it is neither a full bootstrap nor a timed build of main_parallel.ddr. Generated corpus: 101,024 token-bearing lines / 508,056 tokens, 21 modules / 1,001 functions. Consumer validation is outside recorded compile timings.

## off-regression

| Operation/case | Baseline median / p95 ms | Candidate median / p95 ms | Median change | p95 change |
|---|---:|---:|---:|---:|
| compiler/generated | 611.208 / 618.320 | 612.923 / 619.590 | +0.28% | +0.21% |
| compiler/selfhost | 779.491 / 782.908 | 784.063 / 801.194 | +0.59% | +2.34% |
| compiler/tabell | 18.451 / 19.544 | 18.578 / 21.169 | +0.69% | +8.32% |
| check/generated | 186.704 / 187.860 | 187.116 / 188.321 | +0.22% | +0.25% |
| check/selfhost | 212.480 / 216.108 | 216.617 / 217.341 | +1.95% | +0.57% |
| check/tabell | 5.088 / 5.877 | 5.128 / 6.081 | +0.79% | +3.48% |

Image equality baseline/candidate: generated=identical, selfhost=identical, tabell=identical.

Generated candidate build p95 throughput: 163050 lines/s.

## on-architecture

| Operation/case | Baseline median / p95 ms | Candidate median / p95 ms | Median change | p95 change |
|---|---:|---:|---:|---:|
| compiler/generated | 621.405 / 622.797 | 623.655 / 625.834 | +0.36% | +0.49% |
| compiler/selfhost | 1316.979 / 1320.814 | 952.804 / 965.234 | -27.65% | -26.92% |
| compiler/tabell | 24.866 / 26.726 | 22.756 / 32.181 | -8.48% | +20.41% |
| check/generated | 186.936 / 188.464 | 187.742 / 188.481 | +0.43% | +0.01% |
| check/selfhost | 212.844 / 213.910 | 216.964 / 218.565 | +1.94% | +2.18% |
| check/tabell | 5.212 / 5.358 | 5.203 / 5.768 | -0.17% | +7.65% |

Image equality baseline/candidate: generated=identical, selfhost=different, tabell=different.

Generated candidate build p95 throughput: 161423 lines/s.

## user-profile

| Operation/case | Baseline median / p95 ms | Candidate median / p95 ms | Median change | p95 change |
|---|---:|---:|---:|---:|
| compiler/generated | 619.219 / 620.729 | 624.279 / 631.928 | +0.82% | +1.80% |
| compiler/selfhost | 785.522 / 788.351 | 952.521 / 954.722 | +21.26% | +21.10% |
| compiler/tabell | 18.054 / 18.621 | 23.375 / 25.484 | +29.47% | +36.86% |
| check/generated | 188.105 / 189.078 | 188.307 / 189.284 | +0.11% | +0.11% |
| check/selfhost | 217.172 / 219.864 | 216.994 / 218.138 | -0.08% | -0.78% |
| check/tabell | 5.202 / 5.630 | 5.312 / 5.820 | +2.10% | +3.38% |

Image equality baseline/candidate: numeric=different, scan=different, text=different, sort=different, generated=identical, selfhost=different, tabell=different.

| Runtime | SSA off median / p95 ms | SSA on median / p95 ms | C median / p95 ms | SSA on/off median | off/C | on/C |
|---|---:|---:|---:|---:|---:|---:|
| numeric | 5.055 / 7.582 | 5.062 / 8.180 | 4.496 / 5.898 | +0.14% | 1.124313 | 1.125893 |
| scan | 2.555 / 7.093 | 2.518 / 4.714 | 2.439 / 5.024 | -1.45% | 1.047319 | 1.032132 |
| text | 3.449 / 6.991 | 3.441 / 5.628 | 2.572 / 4.643 | -0.25% | 1.340944 | 1.337566 |
| sort | 2.982 / 6.802 | 2.987 / 4.863 | 2.361 / 4.452 | +0.16% | 1.262895 | 1.264899 |

Median runtime geometric mean versus C: SSA off 1.188327, SSA on 1.184134. The proposed C gate (geomean <=1.10, every case <=1.25) remains open for both profiles. All four runtime images change with SSA on; all checksums match independent references. C uses clang -std=c11 -O3 -ftrapv and has no Dunder array bounds checks. These short fresh-process measurements include process startup; runtime tails are retained and no broad C parity claim is justified.

Generated candidate build p95 throughput: 159866 lines/s.

## Scope and decision evidence

The demand-driven SSA architecture reduces same-source SSA-on selfhost median by 27.65%, while on/on Tabell p95 worsens. Direct candidate off/on still costs 21.26% for selfhost and 29.47% for Tabell at the median, with no overall runtime benefit established by the four kernels. Standard-off compile medians change by under 0.7%; selfhost check median is +1.95%. No profile is activated by this report. All raw samples and tails are retained.

Nearest-rank p95 is the 29th sorted observation of 30. `all-cohorts-validation.json` includes recomputed medians, p95, min/max, image SHA-256s, cross-cohort image consistency and artifact hashes. Ordinary default-off runtime images match the prior compiler exactly; no extra off/off runtime timing series was run.
