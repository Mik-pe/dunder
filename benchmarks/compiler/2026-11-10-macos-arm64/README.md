# Compiler and agent measurements, 2026-11-10

Measured on an Apple M5 with 24 GiB RAM, macOS 27.2 (26B123), ARM64. Each series has 30 measured rounds. Benchmark series ran sequentially while other team compilation and VM test workloads were idle. Ordinary desktop background activity remained present.

## Full compilation

The generated corpus has 101,024 lines, 2,216,254 source bytes, 508,056 lexical tokens, 1,001 functions and 21 modules. Each sample starts a fresh compiler process and writes a complete signed Mach-O executable with incremental caching disabled. The OS filesystem cache is warm. The first sample is included. Every output executable is run outside the measured interval and must print `595500` and exit successfully.

| Compiler | p50 ms | p95 ms | LOC/s at p95 | Image bytes |
| --- | ---: | ---: | ---: | ---: |
| Rust bootstrap, release | 53.995 | 60.391 | 1,672,832 | 2,035,712 |
| Dunder stage 3 | 759.654 | 794.219 | 127,199 | 7,354,496 |

The one-million-LOC/s target is met by the Rust bootstrap on this corpus. The selfhost remains below that target. These measurements use a generated scalar workload and do not establish throughput on every application.

Raw samples are in [bootstrap.csv](bootstrap.csv) and [selfhost.csv](selfhost.csv). The bootstrap report also records internal phase timings. Full process elapsed time is used for both results.

## Agent latency

The deterministic project contains 20 test groups. One unrecorded warmup round precedes measurement. Each measured round changes one group's transitive callee, runs `check --agent`, runs `test --affected`, and runs `test --affected` again with no further changes. The harness verifies one passing affected group and 19 unaffected groups, followed by 20 unaffected groups.

| Compiler | Operation | p50 ms | p95 ms |
| --- | --- | ---: | ---: |
| Rust bootstrap | `check --agent` | 3.395 | 6.280 |
| Rust bootstrap | One affected group | 33.332 | 150.270 |
| Rust bootstrap | No source change | 4.548 | 7.868 |
| Dunder stage 3 | `check --agent` | 3.403 | 3.907 |
| Dunder stage 3 | One affected group | 143.183 | 149.495 |
| Dunder stage 3 | No source change | 12.375 | 14.008 |

Raw samples are in [agent.csv](agent.csv) and [selfhost-agent.csv](selfhost-agent.csv). All samples are retained. p50 is the median, averaging the two central samples. p95 is nearest rank, sample 29 of 30 after sorting.

## Provenance and reproduction

Stage 2 and stage 3 are byte-identical. The source files remained unchanged throughout the final builds and measurements. The checkout contained integration changes on top of `35a1675832c67178b075b004fd99a612094bfbd5`. [metadata.json](metadata.json) records binary SHA-256 hashes, every compiler source file hash, the ordered concatenation SHA-256 of selfhost and standard-library Dunder sources, tool hashes, machine details and complete summaries.

The measured stage 3 SHA-256 is `3737bc9c22ea04411a3ee51469a8e0af94dafdf42f45d2ce6959d179401c6275`. The measured bootstrap SHA-256 is `af4e09cb74d9d8bc22fe8c338eee02019a5c5e0361f9df9d2fbc23c1ce9d5e66`.

Run from the repository root after building the release bootstrap and a byte-identical stage 2/stage 3 pair. Substitute the local compiler paths.

```sh
rustc -O tools/compile-bench.rs -o /tmp/compile-bench
/tmp/compile-bench --no-phases /path/to/stage3 /tmp/selfhost.csv 30
/tmp/compile-bench /path/to/release/dunder /tmp/bootstrap.csv 30
python3 tools/agent-bench.py /path/to/release/dunder /tmp/agent.csv --rounds 30
python3 tools/agent-bench.py /path/to/stage3 /tmp/selfhost-agent.csv --rounds 30
```
