# Sparse SSA measurements

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
