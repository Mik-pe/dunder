# Whole-function optimization milestones

These measurements preserve three architectural comparisons on the unchanged
runtime and compiler cohorts. Each directory records exact compiler and source
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
