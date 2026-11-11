# Compiler dogfood cohorts

Four consumer programs exercise costs that the four scalar runtime kernels do
not cover. They are a separate before/after and attribution suite. They do not
change the existing C acceptance suite, its geometric mean, or its thresholds.
There are no C references for these cohorts yet.

| Program | Frozen work per iteration | Independent Python result |
| --- | --- | --- |
| `pod.ddr` | Produce a mixed POD record, copy it, change every field in one copy, refine another and consume all four through value arguments. | Tuple calculations, field mutation and four separate result sums. All fields, both boolean states, original and copied values contribute. |
| `decimal.ddr` | Parse every decimal spelling, rotating its position each round. Inputs include signs, separators, both i64 limits, malformed text and overflow before trailing junk. | Positive arbitrary-precision accumulation, left-to-right error precedence, accepted/invalid/overflow counts, signed residues and position-weighted values/errors. |
| `lexer.ddr` | Call the actual `selfhost/lexer.scan`, alternating interning on/off, over frozen compiler source. Consume every token kind, decoded spelling, byte span and interned name ID. | A Python regex lexer and independent escape decoder; UTF-8 byte offsets; independently assigned first-use name IDs; full-spelling byte hash. The oracle covers the frozen valid input, not every diagnostic of the language lexer. |
| `symbols.ddr` | Populate and update a map, copy it, remove and reinsert keys, then query current and saved versions. Keys include empty strings, Unicode, common prefixes and different byte spellings of similar text. | Python dictionaries, weighted current/snapshot values, lengths, deletions and missing-key results. Bucket order is not part of the result. |

The lexer input contains copies of `opt_ir.ddr`, `opt_alloc.ddr` and
`scalar_rules.ddr` from revision `162fb65`. Its original file hashes and the
hashes of all frozen input files are in `workloads.json`. The consumer imports
the current compiler lexer normally; both compilers being compared build the
same final source tree. This makes it a test of generated programs as well as
the compiler's own data structures. It is not a comparison of different input
sizes or algorithms.

## Check correctness first

```sh
python3 tools/profile-dogfood.py check /path/to/dunder
```

This builds each consumer and runs three iterations, comparing exact stdout
with the Python reference. It records no time measurements. The same check
should pass for baseline and candidate. All compiler and consumer processes
run with `PATH=/no-external-tools` and no inherited environment variables.

## Calibrate once and freeze counts

The checked-in inputs and work counts are frozen. `workloads.json` records the
designated baseline and the [raw calibration](../2026-11-11-ownership-leases/calibration-revision2/).
Revision 2 counts are POD 593,041; decimal 14,479; lexer 284; symbols 445.
Revision 1 sized the first launch of a newly built image while comparison
used a warmup. Its short-work results are preserved; revision 2 sizes after
the same checked warmup used by comparison. The
comparison rejects any manifest with `frozen: false`.

For a future version of the workload, only the controlling parent should run
calibration and timing series. Calibrate one designated baseline, then retain
exactly the same counts for every variant:

```sh
python3 tools/profile-dogfood.py calibrate /path/to/baseline \
  build/dogfood-calibration --target-ms 150
```

Calibration verifies one unrecorded warmup, then times each current count once
and writes proposals to
`recommended-workloads.json`. This single observation is sizing evidence, not
a performance distribution. Review that file, set `frozen` to `true`, and retain
it as the versioned workload manifest. Do not adapt counts per compiler or
per sample. The small correctness check always retains its separate count.

```sh
python3 tools/profile-dogfood.py compare /path/to/baseline /path/to/candidate \
  build/dogfood-comparison --manifest /path/to/frozen-workloads.json --rounds 30
```

Comparison builds both variants, verifies one unrecorded warmup per binary, and
runs 30 fresh processes per variant and cohort with alternating order. Every
sample must produce the exact independent result. Times cover process startup,
input reading, setup, the work and checksum output. Input files may be in the
warm OS cache. Sources, input files, manifest and compiler hashes are checked
again at completion. `samples.csv`, `summary.csv` and `metadata.json` preserve
order, median, nearest-rank p95, image identities, flags, inputs and references.
No cross-cohort aggregate or C-parity claim is produced.

Use repeated `--baseline-flag=...` and `--candidate-flag=...` for controlled
compiler ablations. All modes accept repeated `--case pod` / `decimal` / `lexer`
/ `symbols` to select cohorts; flags and selection are recorded in metadata.

## Allocation attribution

With a compiler supporting `--runtime-profile`:

```sh
python3 tools/profile-dogfood.py attribute /path/to/dunder \
  build/dogfood-profile --manifest /path/to/frozen-workloads.json
# Small integration check before a full attribution run:
python3 tools/profile-dogfood.py attribute /path/to/dunder \
  build/dogfood-profile-smoke --smoke
```

This builds instrumented native consumers, validates the same stdout, and
requires exactly one `runtime_profile` v1 JSON record from each successful run.
Missing reports, wrong units, malformed counters and inconsistent allocation
classification are errors; missing data are never replaced by zeros. Raw
stderr/stdout and parsed records are retained per cohort. There is no elapsed
time measurement in attribution mode.

The 16 counters describe executed traffic over the whole consumer invocation,
including argument/input setup and checksum production. Requested byte counts
exclude the 16-byte allocator header but include runtime payload headers, such
as a string length word. Size-class payload bytes exclude the allocator header;
fresh/reused allocation counts sum modulo uint64 to the allocation total.
Retain/release count helper entries, including null and immortal values.
The profiler's initial pool map and report formatting do not count as ordinary
allocations. No live/peak memory, resident-memory or kernel-only claim follows
from these traffic counters. Instrumented executions are attribution evidence;
the comparison series uses ordinary uninstrumented consumers.
