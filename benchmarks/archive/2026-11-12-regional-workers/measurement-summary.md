# Worker lowering: measurement summary

Frozen source `073856c3a8d5a54644d6855bdaae25e184a0be34`; old compiler `9e5b774adff87835bbdf1cee07f19c2801facd6d4f08a1148d5a79c5bbe04de1`, candidate `7d94803b76b85729987746b0efb4538bb553bd0c079e6fe2eb0d694145bc9fea`. Both use default SSA OFF. All 1,260 samples / 42 groups / 30 rounds per case-variant are independently validated against CSV and command ledger. No repeats, discarded samples or outlier filtering. Source/tool/compiler/HEAD/ref guards unchanged.

All six native consumer images, three supported Wasm consumer images, and four runtime image cases are byte-identical across revisions/capacities. Serial/parallel compiler output hashes match the frozen bootstrap products. Runtime/C series is unnecessary and no runtime improvement is claimed. Selfhost/main.ddr and main_parallel.ddr are distinct entry builds, never the whole bootstrap.

## A: previous standard versus new standard, default four workers

| Operation/case | Old median / p95 ms | New median / p95 ms | Median change | Paired median / p95 ratio | Old RSS median / p95 MiB | New RSS median / p95 MiB |
|---|---:|---:|---:|---:|---:|---:|
| build/generated | 615.366 / 621.225 | 620.344 / 626.787 | +0.81% | 1.00826 / 1.01741 | 77.20 / 77.20 | 75.17 / 75.19 |
| check/generated | 187.539 / 188.966 | 187.233 / 188.036 | -0.16% | 0.99908 / 1.00480 | 55.78 / 55.78 | 55.81 / 55.81 |
| build/selfhost | 809.697 / 813.078 | 822.310 / 824.979 | +1.56% | 1.01555 / 1.02199 | 66.72 / 66.72 | 63.48 / 63.48 |
| check/selfhost | 229.982 / 230.980 | 231.350 / 232.909 | +0.59% | 1.00585 / 1.01207 | 33.44 / 33.44 | 33.44 / 33.44 |
| build/selfhost-parallel | 957.866 / 963.539 | 971.303 / 974.699 | +1.40% | 1.01406 / 1.01773 | 71.05 / 71.05 | 67.20 / 67.20 |
| check/selfhost-parallel | 384.798 / 389.932 | 387.112 / 388.178 | +0.60% | 1.00579 / 1.00994 | 38.78 / 38.78 | 38.81 / 38.81 |
| build/tabell | 17.892 / 18.187 | 18.151 / 18.686 | +1.45% | 1.01310 / 1.05861 | 8.06 / 8.06 | 7.53 / 7.53 |
| check/tabell | 5.214 / 5.618 | 5.195 / 5.598 | -0.37% | 0.99828 / 1.04790 | 3.80 / 3.80 | 3.80 / 3.80 |
| build/hello | 5.239 / 5.662 | 5.319 / 5.652 | +1.53% | 1.01743 / 1.10833 | 3.97 / 3.97 | 4.00 / 4.00 |
| check/hello | 2.030 / 3.026 | 2.019 / 3.175 | -0.54% | 0.99221 / 1.11628 | 2.98 / 2.98 | 3.03 / 3.03 |
| build/owned | 5.566 / 6.062 | 5.601 / 5.989 | +0.64% | 1.00663 / 1.10016 | 4.22 / 4.22 | 4.25 / 4.27 |
| check/owned | 2.094 / 3.190 | 2.063 / 3.121 | -1.45% | 0.97508 / 1.08254 | 3.11 / 3.11 | 3.14 / 3.14 |

## B: same candidate, jobs 1 / 2 / 4

| Case | jobs1 median / p95 ms | jobs2 median / p95 ms | jobs4 median / p95 ms | Median speedup 1→2 / 1→4 |
|---|---:|---:|---:|---:|
| generated | 982.428 / 987.013 | 778.834 / 783.550 | 623.820 / 625.539 | 1.2614× / 1.5749× |
| selfhost | 1023.754 / 1025.300 | 961.848 / 964.762 | 823.461 / 826.993 | 1.0644× / 1.2432× |
| selfhost-parallel | 1186.908 / 1189.227 | 1126.950 / 1132.113 | 971.125 / 975.632 | 1.0532× / 1.2222× |
| tabell | 20.488 / 21.590 | 20.321 / 20.626 | 18.153 / 19.042 | 1.0082× / 1.1286× |
| hello | 5.343 / 5.875 | 5.313 / 5.740 | 5.390 / 5.656 | 1.0058× / 0.9912× |
| owned | 5.970 / 6.796 | 6.025 / 7.551 | 5.743 / 8.545 | 0.9908× / 1.0395× |

| Case | jobs2/jobs1 paired median / p95 | jobs4/jobs1 paired median / p95 | jobs1 RSS median / p95 MiB | jobs2 RSS median / p95 MiB | jobs4 RSS median / p95 MiB |
|---|---:|---:|---:|---:|---:|
| generated | 0.79328 / 0.79830 | 0.63545 / 0.63789 | 71.50 / 71.50 | 74.23 / 74.23 | 75.17 / 75.19 |
| selfhost | 0.93964 / 0.94225 | 0.80444 / 0.80763 | 57.23 / 57.23 | 62.00 / 62.00 | 63.48 / 63.48 |
| selfhost-parallel | 0.94944 / 0.95256 | 0.81809 / 0.82187 | 58.89 / 58.89 | 65.27 / 65.44 | 67.19 / 67.20 |
| tabell | 0.99108 / 1.02646 | 0.88556 / 0.93415 | 5.50 / 5.50 | 7.34 / 7.36 | 7.53 / 7.53 |
| hello | 1.01207 / 1.09181 | 1.00525 / 1.19066 | 4.00 / 4.00 | 4.00 / 4.00 | 4.00 / 4.00 |
| owned | 1.02221 / 1.08709 | 0.95525 / 1.03429 | 4.08 / 4.08 | 4.08 / 4.08 | 4.27 / 4.27 |

## Scope and raw evidence

Clock covers fresh process startup through wait4 exit, including source work, clone/start/join, linking and publication. Validation and hashing are outside recorded intervals. The OS filesystem cache is warm; incremental is disabled. No profiling flags or phase timers are enabled. RSS is wait4.ru_maxrss, normalized to bytes, including all worker threads; it is not allocator, retained-memory or per-function data. MiB is bytes / 1,048,576.

A is the integrated architecture package cost, not isolated causality for one mechanism. Check is a serial negative control. B changes requested worker capacity on the exact same binary/source; all six orders occur five times each. Ratio of medians and round-paired ratios are reported separately. p95 is observation 29 of 30. All tails and per-sample RSS remain in raw CSV/ledger; validation JSON also includes min/max.

Generated build p95 throughput: A_candidate=161178 lines/s, B_jobs1=102353 lines/s, B_jobs2=128931 lines/s, B_jobs4=161499 lines/s.

Artifacts: revision-performance/ and workers-performance/ each retain metadata.json, samples.csv, summary.csv and commands.jsonl; sibling .log files retain controller output. measurement-guard.json and verification-freeze.json capture the frozen inputs. all-cohorts-validation.json contains independent statistics/image/ledger checks and SHA-256 of raw artifacts.
