# Frozen a683 text/sort instruction attribution

Read-only source audit; no compiler source changes, runtime timing series, or runtime benchmark executions. Four small frozen-compiler builds plus two clang assembly emissions. Compiler SHA256 a6833f5784f29ed944ccabbffdc50d007a611622c230bf832897f5ad2bf747a8. All four output image hashes exactly match the retained 30-round memory/values ablation receipt in the main checkout. See manifest.json.

`text-on.asm` and `sort-on.asm` are actual complete measured-image disassemblies, not isolated substitutes. `*-c.s` comes from unchanged benchmarks/runtime/reference.c with clang -std=c11 -O3 -ftrapv -DCASE_text/sort. `selected-paths.json` and `*-selected-path.asm` describe chosen static paths; counts are not performance predictions or sampled dynamic frequencies.

## Text

inspect entry 0x100000d44. Its real rune loop 0x100000e2c..0x100000f78 is call-free, including inline valid-UTF8 decoding. Word count, character sum, cursor, retained snapshot use x25..28. Length at FP-0x38 is reloaded every iteration. Rune is stored to FP-0x48 at ec4, loaded at ed0/ee8/f00/f18; inside bool remains at FP-0x20. Sum evaluation still uses expression STP/LDP at ecc/ed4. ASCII nonseparator with already-true inside follows 44 instructions: 7 FP accesses, 2 expression-stack accesses and 1 input byte load, 4 padding NOPs. The corresponding C control path is 10 instructions and no frame traffic, but has different C-string termination semantics. Dunder embedded NUL must remain a valid scalar and may not terminate iteration. C validates multibyte UTF8 in-loop; Dunder's retained valid snapshot justifies the existing decoder's fewer checks.

## Sort

array.sort entry 0x100000fc8 caches descriptor length/data before loop. Successful shift path 0x100001024..0x100001064 is 17 instructions, including 2 NOPs, 2 heap loads, 1 heap store, 3 index decrements, 3 address additions. Condition loads values[j-1] at 1038. No store/call intervenes before body reloads the same value at 1054, then stores it at 1058. C LBB0_12 shift path is 7 instructions with 1 load+1 store and postdecrement addressing. This count does not imply any speedup ratio. The Dunder sort inner body is already frame-free and call-free; merely widening the current four-home promotion would not remove this duplicated memory value.

## Recommended next mechanism

Introduce explicit virtual value identity / block parameters (SSA), then coalescing + allocation across the actual CFG. First concrete subset: private scalar-frame cells and expression-stack values, including narrow read views with explicit zero/sign extension; do not equate overlapping 8-byte stores and 1/4-byte loads without those views. Reuse loop temporaries in caller-save physical registers subject to actual clobbers, instead of four whole-function fixed homes only. Same representation must support available normal managed-array loads keyed by address value, width/extension and memory version, so the first sort load survives to the body. This is not automatic from scalar SSA: managed object provenance and invalidation by possibly aliasing stores/calls must be explicit; unknown/volatile/exposed memory remains conservative.

Keep checked arithmetic and NZCV order, zero-trip behavior, short circuit, retained snapshot lifetime, and all mutable-array writes. Do not hoist reads/cleanup/trapping arithmetic speculatively. Current mir_value_prop learns Constant/Copy/MOVK facts only; other writes kill identity. Current mir_opt accepts only 8-byte raw accesses and four homes, so narrow rune/bool views are not solved by the existing policy. LICM alone cannot eliminate these observations: array data/length already leave sort loop; text length is invariant but rune/inside traffic and duplicated sort reads remain. Padding/block layout is secondary measurable evidence (4 NOPs vs2 on selected paths), not the primary architectural gap. POD/ownership traffic is outside these inspected inner loops; no attribution here supports prioritizing it for these two cases.

## Required general before/after dump

- Version, compiler/source/dependency hashes, target, ABI/memory/terminal epochs, pass flags, cache stage; cache-restored functions must be identified.
- Stable function declaration ID, entry label, source spans, pass stage, stable op IDs plus explicit order/block membership; include deleted and inserted ops with rewrite reason IDs.
- All semantic opcode/operand roles, widths/extensions, def/use values (future SSA), NZCV deps, effects, call clobbers/preservation. Raw origin words/ranges separately, never in place of semantics.
- Complete CFG including external edge kinds/contracts, initializer entry transfer, reachability, SCC/natural-loop and dominator facts. Backward addresses alone do not establish a loop.
- Byte-range place identity/escape/overlap facts, object lifetime/provenance, managed heap memory versions and load-availability invalidations. source_objects-v1 currently only separates user objects from private frame cells; it is not a no-alias promise between arbitrary heap elements.
- Virtual-to-physical maps, live ranges, coalescing/refusal reasons, spill locations, call-save inserts; per-pass bounded work/status and actual changes.
- Final op-to-emitted-word/native-byte ranges, branch targets/relocations/imports, retained/canonical mode and final image hash. Shared ARM vocabulary word count is not a native count on x86/Wasm.

Existing tests/fixtures/mir-oracle.ddr dumps synthetic scalar operations/places/labels/order. It lacks whole-source compiler entry hooks, per-pass snapshots/CFG, source spans, provenance/escape and allocation/rewrite reasons. Extend a generic opt-in dump stream at lower_source_function boundaries rather than recognizing benchmark names. Dumps are reproducible evidence, not a semantic proof: accepted rewrites still need verifier/dataflow witnesses and differential functional checks.
