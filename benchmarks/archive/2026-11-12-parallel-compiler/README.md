# Parallel compiler: frozen comparisons and allocation attribution

The same compiler image with four workers reduced median build time 36.13%
for the generated corpus and 18.89% for the actual selfhost source versus one
worker. All three output images were identical between worker settings.
Against the previous compiler, the selected four-worker profile improved those
build medians 35.44% and 17.35%. This is measured compiler parallelism, separate
from the earlier source-language task-throughput cohort.

Here selfhost means compiling the frozen serial-entry source
`selfhost/main.ddr`, producing image `126788e5...`. The A/B latency cases do not
build `main_parallel.ddr` with source task certification or time the complete
`bootstrap.sh` chain. The new default bootstrap performs more verification
stages, and no whole-bootstrap wall-time improvement has been measured.

Runtime kernel images did not change. This archive establishes no runtime code
improvement or C parity: the candidate/C median geometric mean was 1.178, above
the 1.10 goal, and text/sort remained above the individual 1.25 limit.

## Source, binaries and protocol

All measurements used clean source revision
`26a27616af2083cf945738eb42240e67f284504e`, with default safety checks, on Apple M5
with 24 GiB memory, Darwin 27.2. Exact hardware, environment, command flags, complete
source hashes and tool hashes are retained in each metadata file.
The later bootstrap-publication checks below use script revision 60b9c6b and
are separate from this measurement source freeze.

Frozen executable SHA256 identities:

| Role | Frozen path under `build/compiler-dogfood/` | SHA256 |
| --- | --- | --- |
| Previous compiler | `task-scopes-integrated` | `598cfb372d932795c5a55825a41a73e43b02c0b29b719ab2af1cd09e2b4d5c91` |
| New serial Session compiler | `session-record-serial` | `126788e56a5d81a9273410d375bc231e231fe24d38fba61fb432f73a5688e6f2` |
| New parallel compiler | `mir-parallel-candidate` | `bbd35784aff9ecf2aaee1953dc6161849dac269556360a1964349e2647252b0c` |

The new serial executable was an allocation-observer producer and correctness
reference. It was not directly timed against the previous compiler here.

Each latency case/variant used 30 fresh processes. Two compiler variants
alternated order each round; the three runtime variants rotated all six orders.
Incremental caching and compiler timing/profile flags were disabled. Recorded
wall time includes process startup; compiler build time includes loading,
compilation, image/link/sign and publication. Consumers were validated outside
recorded build durations. Filesystem caches could be warm; no cold-build claim
is made. Runtime binaries had one checked unrecorded execution before recording;
there was no compiler warmup. p95 is nearest rank, the 29th of 30 sorted samples.

The generated input was the existing reproducible `performance.corpus`:
101024 token-bearing lines, 508056 tokens, 2216254 bytes, 21 modules and 1001
functions with real cross-module calls. Its combined source SHA256 was
`a2738a69c2de4735bd6f94a5c93be3cb12228a530ec74e5a5e4fd45f59ec014f`.
It prints 595500. Every selfhost image built and ran hello (`42`, `true`), and
every Tabell image produced the exact checked-in expected CSV.

No selective repeats or 6/8-worker scouting were performed. Comparisons A and B
are separately observed series; their ratios are not multiplied to invent a
serial-only or total speedup.

## A: same parallel image, one worker versus default four

[`workers/metadata.json`](workers/metadata.json),
[`samples.csv`](workers/samples.csv), [`summary.csv`](workers/summary.csv) and
[`run.log`](workers/run.log) preserve 360 recorded compiler/check processes.
Both variants had the identical parallel compiler SHA above; baseline flags
were `--jobs=1`, candidate flags were empty, selecting the entry's default 4.

Milliseconds, median / p95:

The selfhost row times the frozen `selfhost/main.ddr` serial-entry dependency
graph. It excludes the parallel entry's source task certification and the
bootstrap production/fixedpoint chain.

| Operation | Input | jobs=1 | default 4 | Median change |
| --- | --- | ---: | ---: | ---: |
| Build | Generated | 953.718 / 962.263 | 609.106 / 616.753 | -36.13% |
| Build | Selfhost | 874.940 / 878.757 | 709.677 / 713.322 | -18.89% |
| Build | Tabell | 20.284 / 21.668 | 18.017 / 20.864 | -11.18% |
| Check | Generated | 187.449 / 188.877 | 187.409 / 188.482 | -0.02% |
| Check | Selfhost | 198.439 / 202.436 | 198.241 / 200.262 | -0.10% |
| Check | Tabell | 5.102 / 5.787 | 5.078 / 5.888 | -0.48% |

The four-worker generated p95 corresponds to 163800 lines/s, passing the
100000 lines/s gate. Check uses the serial Session in both variants; its rows
are controls, not parallel-check speedup claims.

All three output images matched byte for byte across worker settings and all
rounds. Their SHA256 identities are the candidate values in the image table
below. Work/budget policy remained the same; only orchestration differed.

## B: previous compiler versus selected default-four profile

[`profile/metadata.json`](profile/metadata.json),
[`samples.csv`](profile/samples.csv), [`summary.csv`](profile/summary.csv) and
[`run.log`](profile/run.log) preserve 720 recorded processes: 360 compiler/check
and 360 runtime processes, including C. Both compiler variants used empty flags
and the same current source checkout. The candidate was the parallel image,
not the serial executable. This measures the total selected profile change.

Milliseconds, median / p95:

The selfhost row again builds the frozen serial entry `selfhost/main.ddr`.
The 17.35% reduction does not measure a complete bootstrap or the compilation of
`main_parallel.ddr`; that entry's selfhosting fixed point is separate functional
evidence below.

| Operation | Input | Previous 598cf | New default 4 | Median change |
| --- | --- | ---: | ---: | ---: |
| Build | Generated | 953.519 / 957.662 | 615.609 / 619.768 | -35.44% |
| Build | Selfhost | 862.428 / 865.752 | 712.768 / 716.325 | -17.35% |
| Build | Tabell | 20.069 / 20.919 | 18.048 / 20.714 | -10.07% |
| Check | Generated | 190.310 / 192.882 | 188.194 / 189.159 | -1.11% |
| Check | Selfhost | 195.690 / 196.487 | 198.933 / 200.810 | +1.66% |
| Check | Tabell | 5.154 / 5.664 | 5.157 / 5.996 | +0.06% |

The candidate generated p95 is 163003 lines/s. Selfhost check p95 increased
2.20%, and Tabell check p95 increased 5.85%; the build gains do not imply every
operation improved.

Selfhost and Tabell images intentionally differ across old/new producers after
the generic unique-record take change. Both remained reproducible within each
variant, and all consumer outputs were checked. Image identity across the two
different producers was not required for these cases:

| Input | Previous 598cf SHA256 | New SHA256 |
| --- | --- | --- |
| Generated | `6f6d09211dd21ccfed218a17160bf3893cb173ffb673fc46df45b14bd71a59d7` | same |
| Selfhost serial source | `1ea03b61c428045a255b41ab3d986266ad7464a08096e1587526dcba190606f2` | `126788e56a5d81a9273410d375bc231e231fe24d38fba61fb432f73a5688e6f2` |
| Tabell | `dc0177bfad4f28f87348ef606ff7cc101f322c5900e29af0e20e6c536d41847b` | `b87caa7b466dc4397edf57b0932bf6efb2fe9662a8104ca72b2eafbe38471029` |

### Runtime and C remain a separate gate

C used Apple clang 21.0.0 with `-std=c11 -O3 -ftrapv`; exact compiler/version/hash
and reference-source hash are in metadata. C has no Dunder array bounds checks,
so these four kernels do not establish broad language parity.

All four Dunder runtime images were identical between old and new compilers:

| Kernel | Unchanged SHA256 |
| --- | --- |
| Numeric | `4a727f7d00586dedbafdb41235b8474a39ac4ea28cafb54570a8fe621d557a74` |
| Scan | `d8e87c9a41d53a38ee0d22c3b47cbe50f1c57244f4da6d6a1f763b64e1e3e94a` |
| Text | `38860ccc0cdfaab15fe6893387b7037950f54544b2903d7d2cbbd43947f7ed47` |
| Sort | `3a981e308007b09ae454bd1fcbdd1976f3843876e08e2a24c177293023994ee3` |

| Kernel | Previous median ms | New median ms | C median ms | New/C |
| --- | ---: | ---: | ---: | ---: |
| Numeric | 5.027 | 5.009 | 4.547 | 1.102 |
| Scan | 2.567 | 2.604 | 2.445 | 1.065 |
| Text | 3.432 | 3.393 | 2.595 | 1.308 |
| Sort | 2.944 | 2.927 | 2.330 | 1.256 |

The median-ratio geometric means were 1.180 previous and 1.178 new. The goal
remains open: geometric mean <= 1.10 and every individual ratio <= 1.25. Small
median motion for identical binaries is not a delivered runtime improvement.
Runtime tails were noisy: scan p95 was 5.414 ms previous and 11.544 ms new despite
identical code. All p95 values are retained in the raw summary; no repeat was
selected to replace this series.

## Untimed whole-compiler allocation observations

[`allocations/metadata.json`](allocations/metadata.json),
[`summary.json`](allocations/summary.json), [`observer.py`](allocations/observer.py)
and every [`raw/`](allocations/raw/) stdout/stderr file are retained unchanged.

The old 598cf and new serial 1267 executables each compiled the SAME current
`selfhost/main.ddr` with `--runtime-profile`. Thus both observed programs contain
the current serial Session algorithm. This isolates producer-codegen allocation
differences, including record take; it is not a comparison of old versus new
compiler algorithms or a single-pass ablation. Main entry source SHA256 was
`46fb502ad907d50add2a3e6243767dbf4a4d17d82fa2d3f0c97b025020aa0cea`;
the complete dependency source hashes are in metadata.

Instrumented observer image SHA256 values:

- Old producer: `4fde18d1b7de761d779b97e77ea71fc15873400ce8fd9fc6ed2bd90bef6addfa`
- New producer: `cd9add5f9c2be83163d9f14ee6eae69f54ac98a2d7bedb7efe1dd2145463a83c`

After both latency series finished, five fresh-process pairs alternated old/new
observer order on one frozen generated corpus. There were no compiler timing
or clock-profile flags. The existing 16-counter reader validated each record.
All ten generated consumer images matched the generated SHA above and printed
595500. All counters were identical across the five runs of each observer.

| Whole-process event/traffic | Old-producer host | New-producer host | Change |
| --- | ---: | ---: | ---: |
| Record clone entries | 16580 | 2085 | -87.42% |
| Array clone entries | 14657 | 2675 | -81.75% |
| Allocation entries | 427821 | 401058 | -6.26% |
| Requested bytes | 257115853 | 231574017 | -9.93% |
| Copied bytes | 150257524 | 126037736 | -16.12% |

Record construction entries stayed 108240 and string-slice entries 213358. These
are whole-process observations, from startup through publication, not MIR-only
counters or timing claims. Retain/release counters count entries, including
null/immortal handles; byte counters describe traffic, not live/peak memory.
The old producer is not expected to be a fixed point of the current source;
the existing profiler tool's fixedpoint guard was not weakened.

## Verification and preservation

[`verification/pre-standard-full-verify.log`](verification/pre-standard-full-verify.log)
is the completed full verification at `26a2761`, before changing bootstrap/default
compiler selection. Root confirmed exit 0. This log does not claim full
verification after selecting the parallel compiler as standard.

The subsequent full standard run at script revision `60b9c6b` also passed,
with the four-worker compiler selected by `bootstrap.sh`:

- [`standard-full-verify.log`](verification/standard-full-verify.log) retains
  the complete `./tools/verify-selfhost.sh` output, exit 0.
- [`standard-verification.json`](verification/standard-verification.json)
  records the command, source revision, binary/log hashes and coverage.
- [`standard-parallel-qa.json`](verification/standard-parallel-qa.json)
  records 108 exact image comparisons and 76 actual executions (64 ARM64 macOS,
  12 Wasm). Linux targets were compared as images; no Linux execution is claimed.

The standard run rebuilt the serial compiler from the checksum-verified seed,
then required serial-produced, one-worker-produced and four-worker-produced
parallel compiler images to match. The published standard has exactly the
measured `bbd35784...` bytes; its serial recovery sidecar has `126788e5...`.
Both the pre-standard and standard runs passed 138 native dogfood groups,
124 Wasm groups with 14 native-only groups, and 46 runtime contracts on each
executed target. Compiler sources and frozen seeds did not change between
measurement and activation. Latency was not repeated for script/documentation
changes that left the measured executable unchanged.

### Bootstrap publication controls, separate revision 60b9c6b

[`identity.json`](acceptance/bootstrap-publication/identity.json),
[`commands.json`](acceptance/bootstrap-publication/commands.json),
[`results.json`](acceptance/bootstrap-publication/results.json) and
[`probe.py`](acceptance/bootstrap-publication/probe.py) preserve a subsequent
publication probe using scripts from
`60b9c6beebef0047256330ca11795aab6bf07467`. No latency observations were added.

The scope is deliberately split:

- **Stubbed shell flow on three host selections:** ARM64 macOS, ARM64 Linux
  and x86-64 Linux. `bootstrap.sh --serial` reached a two-build serial fixed
  point, returned `dunder-serial`, and preserved the standard `dunder` SHA,
  inode and nanosecond mtime. This does not execute native Linux compilers.
- **Stubbed late-failure controls with real cmp/SHA/gzip/mv tools:** after the
  serial and `--jobs 1` fixed points succeeded, `--jobs 4` either exited 73 or emitted a
  mismatching image. Both failed with empty stdout, returned no success path,
  retained their failed staging directory, and preserved the existing standard
  SHA/inode/mtime and parallel sentinel. The mismatch case exited 1.
- **Three real ARM64 macOS compiler productions:** standalone
  `bootstrap-parallel.sh` without an argument delegated exactly once to
  `bootstrap.sh --serial`, without recursion. Its serial-produced, one-worker and
  four-worker compiler images reached the exact frozen parallel SHA `bbd35784...`
  given above. Published sidecar help declared default 4, and it compiled a
  consumer that printed 42. Only serial-bootstrap delegation was stubbed for
  this native case; the three compiler productions and consumer execution
  were real.

These controls are separate from the full standard verification above.
Scratch compilers/staging directories are omitted; the probe requires the
matching frozen serial/parallel binary inputs and original repository path to
be reproduced in a new scratch directory.

Sources, tools, compiler/observer binaries, generated inputs and per-variant
images passed the relevant no-change/reproducibility guards. Main remained
clean `26a2761` through measurement completion. Binaries, generated corpus copies
and output executables were omitted from this small archive; metadata retains
their identities, commands and results. Absolute scratch paths in metadata are
historical provenance. `checksums.sha256` covers every other archived file.
Raw CSV files retain their original CRLF bytes; use Git's `cr-at-eol`
whitespace setting when checking the preserved data rather than rewriting it.

To reproduce, restore source `26a2761` and matching frozen executables first. Use
new empty report directories; do not rerun into this archive:

```sh
python3 tools/performance.py build/compiler-dogfood/mir-parallel-candidate \
  build/compiler-dogfood/mir-parallel-candidate build/reproduce-workers \
  --baseline-flag=--jobs=1 --suite compiler --include-check --rounds 30
python3 tools/performance.py build/compiler-dogfood/task-scopes-integrated \
  build/compiler-dogfood/mir-parallel-candidate build/reproduce-profile \
  --suite all --include-check --rounds 30 --c-compiler /usr/bin/clang
```

For allocation-only reproduction, copy `allocations/observer.py` alone into a
new `build/compiler-dogfood/<report>/` directory and run it there. It validates
the frozen producer hashes and writes counters/raw results without latency
samples. Run observers only after latency work has ended.
