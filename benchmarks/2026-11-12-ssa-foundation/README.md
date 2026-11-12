# SSA foundation: measured cost, producer experiment and correctness evidence

SSA remains **off by default**. Neither enabling it for ordinary user builds
nor using it to optimize the released compiler's producer build is adopted
from these results.

With SSA enabled for user builds, selfhost build median increased 70.92% and
Tabell 42.34%. Only sort improved among the four runtime medians; their combined
candidate/C ratio was 1.18075 versus 1.17826 with SSA off. The producer experiment
then found build changes of only -0.29% to -0.06%, with worse tails and a 1.83%
selfhost-check regression. These results establish no representative speedup
or C-goal achievement.

## Frozen source and protocol

All latency observations used clean revision
`c4fd247894cd0bf1509dad9d9bd4234ce4c8a728`, on Apple M5 with 24 GiB memory,
Darwin 27.2. The final `selfhost/mir_ssa.ddr` SHA256, including the lazy
constructor and correctness fixes, was
`32b6d7e8ec9a92cf1ec1ef067ad704418d91d3a87689257802c8f7e25ed08298`.

| Compiler image | SHA256 |
| --- | --- |
| Standard parallel compiler, produced with SSA off | `5f92b8d8fb9980f861664d21717664ac0f581e4ce76817e6580509b9b1846295` |
| Same current parallel compiler source, produced with SSA on | `28b64220afbf72f90fb76e94642c620218686403e2ae9ae9d5beb4971720ba53` |

Every latency case/variant used 30 fresh processes. Compiler variants alternate
order; runtime variants rotate all six baseline/candidate/C permutations.
No incremental cache, clock/profile flags or diagnostic observer was enabled
in recorded commands. Whole-process wall time includes startup. Compiler build
durations include loading, compilation, image/link/sign and publication;
runtime durations include program setup/work/output/teardown, not compilation.
Consumer validation runs outside recorded build durations. Runtime binaries
receive one checked unrecorded execution;
there is no compiler warmup. Filesystem caches can be warm. p95 is nearest rank,
the 29th of 30 sorted observations. No selective repeats were performed.

The generated workload is the existing reproducible 101024-line corpus:
21 modules, 1001 functions, 508056 tokens, 2216254 bytes, with real module calls.
Its source SHA256 is
`a2738a69c2de4735bd6f94a5c93be3cb12228a530ec74e5a5e4fd45f59ec014f`.
It must print 595500; every selfhost result must build/run hello (`42`, `true`),
and every Tabell result must produce the exact expected CSV.

**Selfhost latency always means compiling `selfhost/main.ddr`, the serial-entry
dependency graph.** It is not the time to compile `main_parallel.ddr` with
source task certification or to run the complete bootstrap/fixedpoint chain.
The parallel executable is the host doing the timed compilation. Its own
producer builds and fixedpoint proofs below are separate, untimed acceptance.

Source, binary and tool guards passed at the end of both matrices. Complete
hardware/environment/flag/source hashes and all samples remain in metadata.
The main checkout remained clean at the frozen revision through measurement.

## A: same compiler, user policy SSA off versus on

[`user-profile/metadata.json`](user-profile/metadata.json),
[`samples.csv`](user-profile/samples.csv),
[`summary.csv`](user-profile/summary.csv) and [`run.log`](user-profile/run.log)
preserve 720 samples: 360 compiler/check processes and 360 runtime processes.
Both compiler paths identify the exact same standard 5f92 image. Baseline flags
are empty, selecting default SSA off; candidate adds only `--mir-ssa=on`.
Both use default 4 workers. Check remains a serial Session/backend-disabled
control, so its rows are not SSA optimization throughput claims.

Milliseconds, median / p95:

| Operation | Input | SSA off | SSA on | Median change |
| --- | --- | ---: | ---: | ---: |
| Build | Generated | 606.871 / 615.212 | 613.026 / 618.533 | +1.01% |
| Build | Serial selfhost source | 762.977 / 767.749 | 1304.080 / 1314.487 | +70.92% |
| Build | Tabell | 17.564 / 19.516 | 25.001 / 34.246 | +42.34% |
| Check | Generated | 185.714 / 187.017 | 185.534 / 186.548 | -0.10% |
| Check | Serial selfhost source | 208.722 / 209.712 | 208.939 / 209.781 | +0.10% |
| Check | Tabell | 5.069 / 5.497 | 5.087 / 5.650 | +0.34% |

The generated on-profile still passes the 100000 lines/s p95 gate, at 163328
lines/s. This does not offset the selfhost/Tabell costs: their build p95
regressions were 71.21% and 75.47% respectively.

### Runtime/C result, without a parity claim

C used Apple clang 21.0.0, `-std=c11 -O3 -ftrapv`; its exact executable hash,
version and reference source hash are recorded. C lacks Dunder array bounds
checks, so these four kernels do not establish broad language equivalence.

| Kernel | Off median ms | On median ms | C median ms | On/off change | On/C |
| --- | ---: | ---: | ---: | ---: | ---: |
| Numeric | 4.961895 | 5.051521 | 4.533812 | +1.81% | 1.11419 |
| Scan | 2.497937 | 2.556000 | 2.411396 | +2.32% | 1.05997 |
| Text | 3.403417 | 3.419250 | 2.540480 | +0.47% | 1.34591 |
| Sort | 2.957980 | 2.850396 | 2.330958 | -3.64% | 1.22284 |

The median-ratio geometric mean is 1.18075 on versus 1.17826 off. The goal remains
open: geometric mean at most 1.10 and every individual ratio at most 1.25. Sort's
measured gain is retained; it does not justify an overall runtime improvement.
All runtime p95 values, including numeric 9.436 ms off versus 12.095 ms on, remain
in the raw summary. These short whole-process timings have noisy tails.

All four Dunder runtime images actually changed under SSA; this was not a
comparison of identical program bytes. Full SHA256 values:

| Kernel | SSA off | SSA on |
| --- | --- | --- |
| Numeric | `4a727f7d00586dedbafdb41235b8474a39ac4ea28cafb54570a8fe621d557a74` | `02f6a94bdc51f7649616c181ea0b3dad95a1947cf7e840a9c2d147784cc606a2` |
| Scan | `d8e87c9a41d53a38ee0d22c3b47cbe50f1c57244f4da6d6a1f763b64e1e3e94a` | `8163b67175353a00ba6a6891d8733cd5a03e0b2832a7cc0359a5fdf049c48299` |
| Text | `38860ccc0cdfaab15fe6893387b7037950f54544b2903d7d2cbbd43947f7ed47` | `87095aa20b6ab3a0ab9cb8735582ca88043d9d23bfe4bd614727e45a3cd42ebe` |
| Sort | `3a981e308007b09ae454bd1fcbdd1976f3843876e08e2a24c177293023994ee3` | `84e440de59f1b3bc0c83871d6b5428aefb2f1cf15312ed41ec15359ce086f265` |

Generated output stayed unchanged. The serial selfhost image changed from
`b45f1c8d11d90471e6d92ca7019839335971a6b48b843f3c23fd1fbc55fe91c7` to
`6dd2efb2c0a0644943064f8d5f495a1c044bbb77264dbf492adbfbc98b3c1569`.
Tabell changed from `b87caa7b466dc4397edf57b0932bf6efb2fe9662a8104ca72b2eafbe38471029`
to `9142838d6d87db95b41e4264671b5d65be0b3af25215b647b7d20c73312410e4`.
Each variant remained byte-reproducible across rounds and all consumers passed.

## B: optimize the compiler producer, keep user policy SSA off

[`producer-profile/metadata.json`](producer-profile/metadata.json),
[`samples.csv`](producer-profile/samples.csv),
[`summary.csv`](producer-profile/summary.csv) and
[`run.log`](producer-profile/run.log) preserve 360 compiler/check samples.
Both host executables contain the exact same current `main_parallel.ddr`
source and default 4 workers. The difference is whether SSA was enabled while
producing that host executable. Neither measured variant receives any SSA flag:
both compile their inputs with the same conventional default user policy off.

Milliseconds, median / p95:

| Operation | Input | Off-produced host 5f92 | On-produced host 28b | Median change |
| --- | --- | ---: | ---: | ---: |
| Build | Generated | 609.173 / 615.364 | 607.418 / 618.897 | -0.288% |
| Build | Serial selfhost source | 762.589 / 765.569 | 762.146 / 766.673 | -0.058% |
| Build | Tabell | 18.181 / 18.782 | 18.157 / 21.068 | -0.133% |
| Check | Generated | 186.003 / 187.467 | 186.067 / 187.194 | +0.035% |
| Check | Serial selfhost source | 208.630 / 210.696 | 212.441 / 213.457 | +1.827% |
| Check | Tabell | 5.106 / 5.584 | 5.100 / 5.864 | -0.130% |

These small build-median changes do not demonstrate a useful producer gain.
Tabell build p95 increased 12.17%; selfhost check median/p95 increased 1.83%/1.31%.
Generated on-produced-host p95 remains 163232 lines/s, but the producer profile
is not enabled on this evidence. No B runtime/C series or runtime gain is
claimed; both variants emit the same default-off program bytes.

All three consumer images matched exactly across both hosts and all rounds:

| Consumer | SHA256 |
| --- | --- |
| Generated | `6f6d09211dd21ccfed218a17160bf3893cb173ffb673fc46df45b14bd71a59d7` |
| Serial selfhost source | `b45f1c8d11d90471e6d92ca7019839335971a6b48b843f3c23fd1fbc55fe91c7` |
| Tabell | `b87caa7b466dc4397edf57b0932bf6efb2fe9662a8104ca72b2eafbe38471029` |

## Policy and producer fixedpoint evidence

[`policy-proof/metadata.json`](policy-proof/metadata.json) retains the separate
out-of-band command trace and raw stdout/stderr. It includes the actual
[`source.ddr`](policy-proof/source.ddr) and the exact
[`acceptance-reference.py`](policy-proof/acceptance-reference.py) used for its
independent integer results. Its `--timings=json` clock records are diagnostic
observations used to inspect policy and actual counters; they are outside both
latency distributions.

On the source probe, default-profile SSA off reported all twelve SSA counters
zero. SSA on actually optimized one function: 5 cells, 510 values, 4 parameters,
5 registers, 16 removed loads, 20 copies, 139304 analysis units, 0 budget refusals.
The same productive result held with the acceptance tool's isolation flags;
those extra flags were never used in A or B. Results for 0, 1, 6, 31 loop iterations
matched the independent wrapping-arithmetic reference. Positive counters are
evidence of pass selection, not evidence of profitable emitted code.

Both frozen hosts, invoked unflagged on current `main_parallel.ddr`, reported
MIR/optimization enabled but SSA disabled with zero SSA counters, emitted the
exact off-produced 5f92 image, and built/ran hello 42/true. This proves that
producer-side optimization did not change the default user policy.

[`self-producer/identity.json`](self-producer/identity.json) records read-only
hashes of the three preserved on-produced binaries; their
[`first.log`](self-producer/first.log), [`second.log`](self-producer/second.log)
and [`four.log`](self-producer/four.log) are copied unchanged. All three hashes
are exactly 28b above. SSA was explicitly on in every production: first/second
with jobs 1, then jobs 4. The copied logs contain publication lines, not a full
argument trace; the role/flag provenance is recorded separately in identity.

The fixedpoint comparison requires `--mir-ssa=on` again when the on-produced
compiler recompiles itself. An unflagged selfbuild deliberately emits the off
image 5f92, not the on image 28b. This is the expected policy separation, not a
reason to weaken bootstrap comparison or silently enable SSA for users.
These producer builds are acceptance, not a whole-bootstrap latency result.

## Correctness reports and coverage limits

[`verification/full-verify-c4fd247.log`](verification/full-verify-c4fd247.log)
is the completed exit 0 full verification before the matrices.
[`final-acceptance.json`](verification/final-acceptance.json) binds its log,
the measured standard compiler, final SSA module and acceptance counts.
Compiler/runtime sources and frozen seeds did not change between that
verification and the measurements or publication.

The integrated
[`parallel report`](verification/parallel-integrated-c4fd247.json) records
108 images and 76 actual executions (64 ARM64/macOS, 12 Wasm).
The [`frame-exit report`](verification/frame-exits-integrated-c4fd247.json)
retains raw observer regressions and explicit producer-contract checks,
including 32 terminal/source images. Linux targets are image-only in these
reports.

Two SSA QA reports are preserved verbatim:

| Report | Compiler SHA256 | SSA module SHA256 |
| --- | --- | --- |
| [`ssa-followup383.json`](verification/ssa-followup383.json), historical follow-up | `f37d0ad2074e0e09bd21e308a1ebd0663e0db76e1d3856b6368e73945e9a9c81` | `8485fcc1b9d86b2365818398d88f0c5edb85322274ed807243e8aa23cd9ea2a7` |
| [`ssa-integrated-c4fd247.json`](verification/ssa-integrated-c4fd247.json), final integrated report | `5f92b8d8fb9980f861664d21717664ac0f581e4ce76817e6580509b9b1846295` | `32b6d7e8ec9a92cf1ec1ef067ad704418d91d3a87689257802c8f7e25ed08298` |

Each reports `images_built=554`, `executions=292`, `independent_pairs=438` and
`explicit_gap_pairs=116`. The historical follow-up is not relabeled as a test
of the final lazy-constructor module. The reports explicitly limit interpreter
coverage: calls, FP, consumed unknown scratch and external exits are gaps;
actual observing images execute. Linux targets are built but not executed on
the ARM64/macOS host. The expression-stack case provides semantic coverage
without promising a stack optimization. This finite cohort is not a universal
proof of the pass.

### Native ABI probe and bounded runtime audit

The final [`native ABI receipt`](verification/abi/result-5f92b8d8fb99.json)
passes X19–X28, D8–D15 and FP/SP sentinels through a real source callback,
promoted internal calls with twelve arguments, text runtime calls, and a
twelve-argument C call. Its image hash is identical to the preserved
[`previous standard`](verification/abi/result-bbd35784aff9.json) and
[`repair-only compiler`](verification/abi/result-f37d0ad2074e.json) results.
The [`probe driver`](verification/abi/probe.py) reuses the existing independent
assembly oracle in `tools/test-task-native.py`; source and generated C/assembly
are preserved alongside the receipts.

The [`runtime inventory`](verification/abi/runtime-inventory.log) covers seven
target/profile variants. Its bounded instruction audit found no explicit
X25–X28 use in the examined native runtime helpers; the six Wasm custom memory
operations were separately checked against their producer. This is evidence
for the documented source ABI contract, not permission to narrow raw call
read/clobber masks.

The separate [`call-input survey`](verification/abi/call-input-analysis.json)
is read-only preparation for a future explicit call-operand model. It is not
an active compiler policy or proof for arbitrary external code. In particular,
the x86 foreign-call pseudo-operation reads logical register 17 as its target,
although the current generic decoder does not expose that operand. The current
conservative call masks retain it. Any future narrowing must first represent
this real input and preserve unknown-call behavior.

## Reproduction and preservation

Metadata, CSV and logs are verbatim copies. Binary outputs and scratch compiler
executables are omitted; their identities remain in metadata. Original CSV
CRLF bytes are preserved; Git checks can use `core.whitespace=cr-at-eol`.
`checksums.sha256` covers every other archived file. Historical absolute scratch
paths identify the original run and need not exist in the archive.

The archived ABI drivers originally ran from `build/abi-audit`, two directory
levels below the repository root. To rerun them, copy that archived ABI
directory to a fresh ignored directory at the same depth before running
`probe.py <compiler>`. The native probe needs Clang and macOS tools and imports
the existing repository oracle. It generates a dylib and rewrites its own
probe source with that dylib's absolute path; a different directory can change
the consumer image hash. No probe binaries or dylibs are archived.

Restore source c4fd247 and matching frozen hosts, then use empty output paths:

```sh
python3 tools/performance.py build/selfhost/arm64-macos/dunder \
  build/selfhost/arm64-macos/dunder build/reproduce-ssa-user \
  --candidate-flag=--mir-ssa=on --suite all --include-check --rounds 30 \
  --c-compiler /usr/bin/clang
python3 tools/performance.py build/selfhost/arm64-macos/dunder \
  build/ssa-foundation/self-producer/on build/reproduce-ssa-producer \
  --suite compiler --include-check --rounds 30
```

Producer construction, fixedpoint and policy/clock observers must remain
outside the latency series. No source, compiler, tool or Git-reference changes
were made during either matrix, and neither activation decision was changed
after seeing a preferred subset of the data.
