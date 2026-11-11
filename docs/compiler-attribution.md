# Attribute the compiler's own work

`tools/profile-compiler.py` collects compiler phase clocks and executed
allocation traffic as separate observations. It is an attribution tool, not a
latency benchmark and not part of the four-kernel C acceptance suite.

```sh
python3 tools/profile-compiler.py /path/to/current/dunder \
  build/compiler-attribution --rounds 5
```

The tool requires an empty report directory and an uninstrumented compiler for
the native host. It first builds the host selfhost entry normally and requires
its bytes to match the input compiler. Thus the checkout and compiler cannot
silently describe different compiler implementations. An externally re-signed
or otherwise changed compiler also fails this exact fixed-point check.

It then builds **one** allocation-instrumented compiler using
`--runtime-profile`. Each default source is compiled twice per round:

1. The original compiler uses `--profile-compiler --timings=json` and supplies
   the nonoverlapping compiler subphases, calls and work units.
2. The allocation-instrumented compiler uses neither timing option nor a runtime
   profiling build flag. Its own executed allocation report supplies traffic;
   the program it produces remains uninstrumented.

Collection order alternates, but the observers never run together. Combined
clock/allocation stderr, unavailable clocks, null durations, missing reports,
wrong units and malformed counters are errors. Missing observations are never
replaced by zeros. There is no outer wall-clock timer or p95 latency claim.

## Inputs and result checks

Defaults are the existing 101,024-line generated compiler corpus, the host
selfhost entry, and Tabell. The corpus comes from `tools/performance.py`; no
second stress-corpus definition is introduced.

- Every generated output must print exactly `595500`.
- Every selfhost output must equal the input compiler and build a hello image
  that prints `42` and `true`.
- Every Tabell output must reproduce the project's complete expected CSV.
- All output images must be byteidentical across both observers and all rounds
  for the same source. Profiling changes observers, not these program images.

These executions happen after collecting each compiler observation. Their
commands and raw output are recorded too. All output, validation images, source
corpus files and logs live in the report directory. Compiler and program child
processes receive only `PATH=/no-external-tools`. The source tree is read-only.
Child stdin is closed. Timeout logs retain captured output and an explicit
timeout outcome rather than inventing an exit status or profiling record.

Select a known source or provide a custom source with repeated `--case`:

```sh
python3 tools/profile-compiler.py /path/to/dunder build/self-attribution \
  --case selfhost/main.ddr
python3 tools/profile-compiler.py /path/to/dunder build/pod-attribution \
  --case benchmarks/dogfood/pod.ddr --expected-stdout /path/to/known-output \
  --program-arg 3 --program-arg 1729
```

Known selfhost/Tabell paths retain their authoritative checks. An unknown
custom program is compiled and compared by image identity; it is not executed
and its semantics are not certified without `--expected-stdout`. Optional
execution arguments require exactly one custom case and a result reference.
Such executions use the report's validation directory as their working
directory; supplied code is responsible for any absolute-path operations.
For custom data-driven programs, pass input paths as program arguments and
declare each data file with repeated `--input-file /absolute/path`. Those bytes
and canonical paths are then checked too. The tool cannot infer arbitrary file
access performed by a supplied program.

## Reading the report

`metadata.json` checkpoints every command, cwd, environment, flags, exit status
and raw stdout/stderr path, compiler identities, source/input/helper hashes,
generated-corpus counts and complete JSON observations. It remains marked
incomplete, with a failure description, when collection fails. Source imports
are discovered with the existing independent lexer; source/compiler hashes and
canonical import resolutions are checked again at completion. Embedded standard-library semantics belong to
the recorded compiler image; rebuild-source hashes are also retained.

`summary.json` gives per-case phase-duration medians and allocation-counter lower medians as
navigation aids. Read the raw records for per-round work counts and variation.
Lower medians keep uint64 counters exact even with an even number of rounds.
The eleven compiler detail intervals must sum to `covered`; `covered + other`
must equal the driver compiler interval. Driver phases must sum to their total.
Durations describe the clock-instrumented compiler and include its observer
overhead; they do not measure the uninstrumented compiler's latency.
The compiler phase named `allocation` is register allocation, not the heap
allocator. Individual phase medians need not sum to a median total.

Allocation traffic covers the allocation-instrumented compiler's entire build,
including arguments, loading, compilation, image generation and publication.
It is not a per-phase allocation breakdown. Paths and process-ID formatting can
affect traffic slightly; commands and paths are retained. Retain/release count
helper entries, including null and immortal values, not actual reference-count
changes or frees. Requested/class bytes exclude the allocator header as stated
in the runtime record. No resident/live/peak-memory interpretation is implied.

The runtime schema is validated by the shared reader in
`tools/profile-dogfood.py`, including all 16 uint64 counters and fresh/reused
allocation classification. Results guide the next general language/compiler
mechanism; performance acceptance remains a separate uninstrumented measurement.
