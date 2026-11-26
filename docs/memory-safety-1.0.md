# Dunder 1.0 Architecture Specification: Memory Safety, Borrow Checking, Calls & IR Pipeline

**Status:** Peer-reviewed architectural blueprint (baseline `dunder 0.13.1`, HEAD `ee96969`)  
**Scope:** Language semantics, ownership/borrow model, concurrency & runtime boundaries, unidirectional compiler IR pipeline, and staged `0.13.2 -> 1.0` self-host migration plan.  
**Core Goals Preserved:** Self-hosted in `.ddr`, fast native compilation (~300k LOC/s clean, <100 ms p95 incremental), zero LLVM / zero GC, ergonomic syntax without lifetime soup, unboxed POD values (`<= 128B`), structured concurrency with per-thread arenas, and all 6 targets (`arm64-macos`, `arm64-linux`, `x86_64-linux`, `x86_64-windows`, `wasm32`, `wasm32-wasi`).

---

## 1. Empirical Audit Baseline (`0.13.1`)

An adversarial soundness audit of `0.13.1` (`build/selfhost/arm64-macos/dunder`, 215 standalone `.ddr` reproducers tested under default build flags) established the following ground truth.

### 1.1 Confirmed Soundness Holes (12 Exploits + Raw-FD Boundary)

| ID | Sev | Area | Root Cause (`0.13.1`) | Empirically Observed Impact |
|---|---|---|---|---|
| **S-1** | P0 | Call Aliasing | `invoke_with_args` (`selfhost/frontend/compile.ddr:5127,5150,5168`) checks `wk == 5` / `sibling_kind == 5`, omitting `ft.mutable_option` (`Option<&mut T>`, kind `7`). | 100% safe live heap UAF & memory corruption (`clash(.Some(&mut s), &s[0..25])`, slices, `View`, implicit borrows, borrowed closures). |
| **S-2** | P0 | Stdlib Boundary | `stdlib/alloc.ddr:73-843` declares `OwnedBuf<A>`, `Bump<A>`, `Arena<A>`, `FrozenArena<A>`, `Capped<A>`, `Block`, and `Layout` as non-opaque `pub struct`. | 100% safe OOB read (4 KiB past allocation) & arbitrary heap overwrite via mutating `buf.len`/`buf.cap` before `as_bytes()`/`push_byte()`. |
| **S-3** | P0 | Intra-Expr Aliasing | `record_mutation` logs `borrows.root(place)` (the `&mut` alias `r`), while `mutated_overlap` (`ownership_check.ddr:65-78`) compares against `source_root` (`v`, where `r != v`). | 100% safe live heap UAF across calls (`read_both(&r[0], grow(&mut r))`), slice bounds (`&r[0..grow(&mut r)]`), and struct literals. |
| **S-4** | P0 | Iteration | `iter_mode == 5` (`compile.ddr:10148-10229,10413`) on a temporary `Iterator` rvalue carrying a stored borrow leaves `locked_path = -1` and `is_borrowed = false`. | 100% safe live heap UAF with pure stdlib `for g in text.graphemes(s.as_str())` and custom borrowed cursors. |
| **S-5** | P0 | `defer` / Moves | `defer` bodies are skipped at registration and ownership-checked only once at block end (`finish_block_defers:8824`), never at early exits (`return`, `?`, `break`, `continue`). | 100% safe use-after-move NULL-deref **SIGSEGV** (`exit=139`) across `return`, `?`, and `break`. |
| **S-6** | P0 | Binary Ops | `&str ==` and `&str !=` (`compile.ddr:8480-8507`) evaluate LHS descriptor then RHS with zero `mutated_overlap` check. | 100% safe live heap UAF (`"HELLO_WORLD_ORIGINAL_DATA".as_str() == nuke(&mut s)` evaluates `true` against freed/overwritten slab). |
| **S-7** | P0 | FFI / Concurrency | Program-wide `has_callbacks` (`compile.ddr:11645`) saves/restores `x19..x24` (`ffi_abi.ddr:152-171`) or `r15`/token (`x86.ddr:564-590`) in a single unsynchronized global 64-byte slot on every `@extern` call. | Concurrent `task scope` workers calling `@thread_safe` FFI overwrite each other's per-thread arena registers -> live heap corruption crash (`exit=109`). |
| **S-8** | P1 | Cleanup Order | `unwind_exit` (`compile.ddr:8657-8720`) executes outer `defer` blocks *before* `c.body.cleanup` calls `task_close_slot` (`13352`) / `task_close_locals` (`9417`). | Violates LIFO structured concurrency; outer `defer` runs concurrently with still-active workers on early exit. |
| **S-9** | P1 | `@resource` | `bind_struct` (`compile.ddr:10664-10681`) permits full destructuring `Res { id, data } := move r` on `@resource(dispose)` structs and releases fields directly. | Silently bypasses `dispose(&mut self)` destructor in safe code. |
| **S-10** | P1 | Concurrency Traits | Safe `impl Sendable for T {}` accepted on scalar `@resource` (`compile.ddr:1747` only checks for `ptr` fields); `Published<T>` (`stdlib/sync.ddr:1808-1937`) requires only `T: Sendable` (no `Sync`). | Concurrent cross-thread `&T` aliasing on non-thread-safe `@resource` types via `Published<T>` in 100% safe code. |
| **S-11** | P1 | Closures | `bind_closure` (`compile.ddr:3670-3688`) omits `ft.cloneable` on `Fn` environments; `compute_declared_flags` always treats `Fn` (kind `11`) as RC-cloneable. | `Fn` closures capturing non-cloneable `@resource(dispose)` allow `.clone()`, violating resource uniqueness. |
| **S-12** | P1 | Region Escape | `scratch.resolve` (`scratch.ddr:55-69`) matches lifted closure `fn` (kind `9`) against call-site `Fn`/`FnMut`/`FnOnce` (kinds `11..13`), and `forward_loan:4710` drops body signatures (`>= declared.type_kinds.len`). | Passing a scratch/scoped borrow to a closure calling `unsafe { addr(&p) }` silently bypasses `E4005`. |
| **S-13** | P1 | OS Boundary | `stdlib/sys.ddr:274-301` exposes `sys.net_read`, `sys.net_write`, `sys.net_close`, `sys.net_poll` taking raw `i32` FDs as safe functions (`@requires(network)` only). | Safe code with `capability network` can close/corrupt arbitrary OS FDs (`stdout`, `fs.File`, `Channel`/`Waker` wake pipes). |

### 1.2 Confirmed Usability Cliffs (False Positives)

- **C-1 (`Option<&mut T>` match reborrow):** `match move opt { .Some(m) => { m.clear() } }` or `f(&mut m)` on `Option<&mut []T>` fails with bogus `E4001: use of moved value 'm'` (while direct reassignment `m = [...]` compiles).
- **C-2 (Token-scan NLL field-name collision):** `r := &v[0]; print(r); v.push(3); _ := Foo { r: 1 }` fails with `E4003` because `reads_local` (`ownership_check.ddr:97-157`) scans raw `.Name` tokens and matches struct field label `r` (likewise `for r`, `match` bindings, closure params).
- **C-3 (Token-scan NLL disjoint branches):** `r := &v[0]; if c { v.push(3) } else { print(r) }` fails with `E4003` because linear token scanning cannot distinguish mutually exclusive CFG branches.

### 1.3 Empirically Refuted Hypotheses & Remaining Structural Hazards

To keep engineering focus strictly on verified reality, four initial audit hypotheses were **empirically refuted** on `0.13.1`:
1. User modules **cannot** call `__sys_*` directly (blocked by `E2006: reserved compiler identifier`). *Remaining structural hazard:* internal dispatch at `compile.ddr:3001-3123` still leaves `ret = expected` untyped for 16 intrinsics and omits arg checks (latent stdlib footgun).
2. `Channel<T>` / `Published<T>` **do** deep-clone (`same_ptr=false`) and freeze (`rc=-2`) body-synthesized `[]string`..`[][][]string`. *Remaining structural hazard:* splitting `TypeRegistry.declared` vs `body` remains a recurring trap for helper synthesis.
3. `[]Holder` and `(Holder, i32)` stored-borrow escapes are properly rejected (`E5004`, `E4003`).
4. `borrow_h(&h).cb` / `rh.r.cb` `FnOnce`/`FnMut` calls properly materialize transient `_` locals (`place != 0`), and `take_s(move s)` properly moves with `0` retains. *Remaining structural hazard:* `call_loaded_callable` (`compile.ddr:4859-4892`) still relies on a fragile `place == 0` guard.

### 1.4 Additional Confirmed Runtime, Platform & Backend Debt

- **R-1 (Windows cross-thread allocator race):** `stdlib/sync.ddr:43-101` (`sync_mmap`/`sync_munmap`) and `stdlib/net.ddr:1265-1270` (`waker_munmap`) fall back to per-thread `sys.pool_alloc_try`/`pool_free` on `x86_64-windows` despite multi-threaded `CreateThread` workers -> cross-arena freelist corruption & UAF. `alloc.System` is also unimplemented on Windows (`alloc.ddr:271-295`).
- **R-2 (Negative-RC arbitrary code execution):** `runtime.ddr:383-396` and `backend_emit.ddr:449-454` encode frozen compound clone functions as `-(fn_ptr)` in `[ptr - 8]` and execute an unvalidated `blr x16` on `rc < -2`. Any header corruption becomes immediate RCE.
- **R-3 (Hand-encoded task runtime & platform quirks):** `task_scope_emit.ddr` + `task_native.ddr` contain ~1,500 LOC of duplicated numeric ARM64 opcodes; macOS switches `sp` to an `mmap` stack while leaving `x29` on libdispatch's stack (`task_native.ddr:661-668`); x86_64 maps an unused 8 MiB stack + 16 MiB guard per worker (`task_native.ddr:535`).
- **R-4 (Abort-only panics):** Traps (`panic`, `assert`, bounds 103, overflow 101, UTF-8 106, float 107, OOM 109, bad callback 111, bad task 112) call `exit_group`/`ExitProcess` immediately without unwinding or running `defer`/`dispose`.
- **IR-1..IR-5 (Pipeline inversion & backend emulation):**
  - Hardcoded 8-byte `BorrowHome` escape extent + `Kind.Place` barrier exemption (`mir_ssa_cells.ddr:147`, `mir_opt.ddr:543`), masked only because `borrowed_view`/`bind_stack_closure` emit raw `arm.address` which triggers `place_escaped = true` and kills SSA/frame promotion for the entire function.
  - `mir_verify.ddr` checks encoding/ranges, not operand types, SSA dominance, callee signatures, or memory extents.
  - `selfhost/opt/*` duplicates a second scalar-only tree IR + `prefixing` replay + `stack_option_argument` 192-token lookahead because zero MIR-level inlining exists.
  - ARM64 ISA is baked into MIR, forcing `x86_mir.ddr` (`[r15]` bank) and `wasm_mir.ddr` (WASM globals `0..31`) to emulate ARM64's 32 registers and condition flags.
  - `backend_emit.ddr:187-196` clobbers `x0` before checking `refcount == 1` if `@resource(dispose)` enum cleanup is enabled.

---

## 2. Rewrite vs. Evolve Verdict (First-Principles Triage)

### Why Patching `compile.ddr` + Lifted MIR Cannot Reach 1.0 Soundness
`selfhost/frontend/compile.ddr` (13,643 LOC) fuses token-stream parsing, typechecking, 8 disjoint ad-hoc lock/borrow tables (`local_loans`, `for_locks`, `pattern_loans`, `pattern_borrowed`, `defers.locked_locals`, `borrowed_carriers`, `mutations`, `observing_lease` ARM64 instruction splicing), drop elaboration, and ARM64 word emission into one pass, then **lifts** those machine words back into ARM64-shaped MIR (`mir_lift.ddr`).

Patching this design cannot achieve a 1.0 soundness guarantee:
1. **Combinatorial check duplication:** Every expression form must manually preserve `FunctionContext.loan`, emit `mutated_overlap`, and synchronize 4+ lock tables. One missed path equals live 100%-safe heap UAF (5 independent P0 UAF/SIGSEGV bugs in one audit: **S-1, S-3, S-4, S-5, S-6**).
2. **Lexical token scanning is structurally unsound for control flow:** `reads_local`, `safe_borrowed_string_stream`, `c.loans.active`, `stack_option_argument`, and block-end `defer` checking cannot model CFG reachability, early exits, shadowing, or mutually exclusive branches (**S-5, C-2, C-3**).
3. **Inverted IR direction:** Lifting ARM64 machine words back into MIR destroys types, aggregate extents, borrow semantics, and target independence, forcing both x86_64 and WASM to emulate ARM64.

### Component Triage

| Subsystem | Verdict | 1.0 Disposition |
|---|---|---|
| `compile.ddr` single-pass checker/emitter, 8 lock tables, `ownership_lease.ddr`, `deferred_blocks.ddr`, `scratch.ddr` | **REWRITE (Ground-Up)** | Replace with unidirectional `AST -> Typed HIR -> High-Level Typed CFG MIR -> CFG Borrow Checker -> MIR Drop Elaboration`. |
| Token-rewriting generics, closures, variants (`generics.ddr`, `closures.ddr`, `variant_functions.ddr`, `open_bodies.ddr`) | **REWRITE (Ground-Up)** | Replace with typed HIR monomorphization and first-class HIR closure capture structs. |
| ARM64-lifted MIR & legacy passes (`mir_lift`, `mir_owners`, `mir_expr_stack`, `mir_retained`, `mir_preserve`, `mir_call_inputs`, `mir_source_call`) | **REWRITE & DELETE** | Replace with target-independent typed SSA MIR built downward from HL-MIR, gated by full semantic `MirVerifier`. |
| Duplicate scalar optimizer (`selfhost/opt/*`), `prefixing` replay, `stack_option_argument`, `catalog.predicates`/`converters` | **DELETE** | Replaced by a single typed SSA MIR inliner + SROA/GVN/bounds/DCE pipeline. |
| `borrowed_view.ddr` 48B stack descriptors & `return_optional_view` | **DELETE** | Replaced by first-class 2-word `Copy` fat values `(ptr, len)` for `&str` and `&[]T`. |
| `backend_emit.ddr` + `runtime.ddr` + `program_emit.ddr` + `task_scope_emit.ddr` + `task_native.ddr` | **REWRITE TO SSA MIR / `.ddr`** | Required prerequisite before native x86_64/WASM ISel (eliminates ARM64-ABI coupling, `blr -rc`, and 1,500 LOC of raw ARM64 opcodes). |
| `arm64.ddr`, `x86.ddr`, `wasm_bytes.ddr`, `mir_link.ddr` | **SPLIT & REUSE ENCODERS** | Extract stateless `arm64_enc.ddr` and `x86_enc.ddr`; retain `wasm_bytes.ddr` LEB128/section primitives (delete global-bank helpers); replace `LinkedImage { code: arm.Code }` with target-neutral `ObjectModule` feeding existing `macho.ddr`, `elf.ddr`, `pe.ddr`, `dwarf.ddr`, `wasm_memory.ddr`, `wasm_abi.ddr`, `ffi_arm64.ddr`. |
| `lexer.ddr`, `parser.ddr`, `ast.ddr` | **EVOLVE** | Complete full expression/pattern/type AST coverage and unify as the sole parser for `build`, `check`, `fmt`, and `lsp`. |
| `ownership_state.ddr` sparse place tree | **SALVAGE CONCEPT** | Reuse canonical sparse `PlaceId` trie & field-disjointness logic inside CFG `mir_init` and `mir_borrowck`. |
| `modules.ddr`, `fragments.ddr`, `compile_parallel.ddr`, `task_data/effects/certify.ddr`, core stdlib containers | **EVOLVE / TARGETED FIX** | Keep architecture; seal `std.alloc` (`pub opaque struct`), mark `sys.net_*` `unsafe fn`, add `Sync`, fix Windows `VirtualAlloc`, cache per-function HIR->MIR->Object units. |

---

## 3. First-Principles 1.0 Language Model (Beta -> 1.0 Contract)

### 3.1 Places, Ownership, Origins & Variance

Dunder 1.0 retains **zero-lifetime-soup ergonomics** (no mandatory `'a` parameters on ordinary functions, implicit read-borrows on non-`move` owned parameters, `from` return-source clauses) backed by one formal Place/Loan/Origin model:

1. **Canonical Places**: Every memory location is `Place = LocalId × [Projection*]` with `Projection ∈ { Deref, Field(i), Index(*), Downcast(v) }`. Distinct struct fields are disjoint sibling places; array/slice indexing collapses conservatively to `Index(*)` locking the container.
2. **Ownership (`Copy` vs `Move`)**:
   - `Copy`: scalars, `bool`, `rune`, `ptr`, pure `fn`, fieldless enums, `&T`, `&str`, `&[]T`, and nested `Copy` structs/tuples/enums/fixed arrays `[T; N]` (unboxed inline PODs up to 128B on stack and in containers).
   - `Move`: `string`, `[]T`, `Map`, `Set`, `&mut T`, capturing closures, `@resource`, and aggregates containing any `Move` field.
   - Assignment, return, construction, and `move` parameters transfer ownership. Partial field moves are allowed on ordinary structs and **forbidden** on `@resource(dispose)`.
3. **First-Class 2-Word `Copy` Fat Views (`&str`, `&[]T`)**:
   - `&str` and `&[]T` are **2-word `Copy` fat values `(ptr, len)`** (`16B` on 64-bit targets, `8B` on WASM32) — never pointers to 48-byte stack-allocated fake array headers.
   - Coercing `&([]T) -> &[]T` or `&string -> &str` lowers to a 2-field load `(arr.data, arr.len)` or `(s + 8, s.len)`.
   - `&str` and `&[]T` pass and return in **2 registers** (`x0, x1` on ARM64; `rax, rdx` on SysV x86_64; multi-value `(i32, i32)` on WASM) with zero `sret` and zero stack descriptors.
   - `Option<&str>`, `Result<&str, E>`, `(&str, &str)`, and user structs holding `&str`/`&[]T` become ordinary aggregates governed uniformly by size/align ABI rules (`borrowed_view.ddr` and `return_optional_view` are deleted).
4. **Positional Origins & Zero-Soup Surface Elision**:
   - Every borrow type (`&@o T`, `&mut @o T`, `&@o []T`, `&@o str`, `&@o Fn`, `&mut @o FnMut`) and every struct/enum containing borrow fields carries positional **Origin slots** (`OriginId` representing a set of loans).
   - **Pre-mono Generic Parameter Mode (`ParamMode::BorrowOrCopy`)**: Non-`move` parameters `x: T` in generic functions are represented in pre-monomorphization HIR as `ParamMode::BorrowOrCopy` (NOT eagerly desugared to `&@o_x T`). Pre-mono typechecking allows reading `x` generically and requires `T: Copy` to return/consume `x` by value (otherwise `move x: T`). At monomorphization, `ParamMode::BorrowOrCopy` resolves to by-value `T` when `T: Copy` and to implicit shared borrow `&@o_x T` when `T: Move`.
   - **Return Elision & `from` Clauses**:
     - Single borrow-bearing input automatically binds return origins to that input (`fn window(d: &[]u8, a: i32, b: i32) -> &[]u8`).
     - Multiple borrow-bearing inputs use `from p` or `from (p1, p2)`.
     - Structs/enums holding stored borrows receive one internal origin slot per borrow-bearing field; methods may project a field's origin via `from self.<field>`.
     - **Callable types include `from_map`**: Return-origin mappings `(param_idx, field_idx) -> ret_origin_slot` are part of canonical callable `TypeId`, including a dedicated **`PARAM_ENV`** sentinel for closures returning borrows derived from their captured environment (`f := fn[&v]() -> &i32 { &v[0] }` propagates `v`'s loan into the call result even after `f` itself goes dead).
5. **Variance & Invariance Under `&mut` (Mandatory Soundness Rule)**:
   - Origins in covariant positions (`&@o T`, return values, immutable struct fields) emit unidirectional outlives constraints `O_src -> O_dst`.
   - **Any origin appearing inside `T` behind `&mut T` or `&mut FnMut` is INVARIANT**: every assignment, reborrow, coercion, or call involving `&mut T` emits **bidirectional constraints (`O_a -> O_b` AND `O_b -> O_a`)** for all nested origins inside `T`. This prevents smuggling a shorter-lived `&local` through a `&mut Holder` into an outer `Holder`.
   - **Explicit 1.0 Rule on Stored-Borrow Rebinding & Cross-Parameter Mutation**:
     - **Allowed in 1.0:** Local `mut r: &T` rebinding (`mut r := &a; r = &b`) and same-function `&mut` rebinding of local structs holding borrows. Full overwrite of a non-borrowed `mut` local kills its previous origin contents so `a` is unlocked immediately after `r = &b`.
     - **Forbidden in 1.0:** Overwriting stored-borrow fields through a `&mut` *function parameter* (e.g. `fn set(dst: &mut Holder, val: &i32) { dst.r = val }`) is statically rejected unless/until explicit input-to-input origin syntax (`into dst` / `where val: dst`) is added post-1.0. This keeps Dunder 1.0's surface syntax minimal (`from` only) with zero soundness holes.
   - Local `if`/`match` expressions returning borrows (`r := if c { &a } else { &b }`) and arbitrary stored `&mut T` in structs/enums (`Option<&mut T>`, `Result<&mut T, E>`, `struct MutCursor { buf: &mut []u8 }`) work uniformly with zero special cases.
6. **Non-Lending `Iterator` Contract**:
   - Standard `trait Iterator { type Item; fn next(&mut self) -> Option<Self::Item> }` is **non-lending**: for a borrowed iterator such as `impl Iterator for Graphemes<@o> { type Item = &@o str }`, `Option<Item>`'s origin is bound to `Self`'s **type-level origin slot** (`O_self[text]`), completely independent of `&mut self`'s borrow lifetime. Multiple `next()` items can therefore coexist while `self` advances, and `for g in text.graphemes(s.as_str())` keeps `s` locked across the entire loop.
   - An iterator whose `next(&mut self)` yields a borrow tied to `&mut self` (`from self`, e.g. `&mut self.buf[i]`) is **statically rejected** from implementing non-lending `Iterator` (preventing simultaneous `&mut` aliasing across consecutive `next()` calls). A separate `LendingIterator` trait may be added post-1.0.
7. **Loans, Reborrows & `TwoPhaseMut`**:
   - Every borrow construct (`&p`, `&mut p`, slices, `s.as_str()`, `bytes_view`, `for`, `match`, implicit call-site borrows, borrowed closure captures) produces a `Loan { place, kind: Shared | Mut | TwoPhaseMut, origin }`.
   - `&mut` reborrows (`&*r`, `&mut *r`, `m.clear()` on `m: &mut []T`) create a child loan on the deref-projected place `*r` chained to `r`'s origin. Reborrowing `*r` is permitted whenever no conflicting *child* loan derived from `*r` is active (resolving both **S-3** and **C-1**).
   - **`TwoPhaseMut` Exact Rules**: Used *strictly* for implicit `&mut self` method receivers (`v.push(v.len)`):
     - While reserved (during argument evaluation): allows `Read` and `SharedRef` of the receiver; **rejects** `Write`, `Move`, `MutRef`, and any **second `TwoPhaseMut`** (rejecting `v.push(v.pop())`).
     - At the `Call` terminator activation point: transitions to full `Mut` and **re-checks** against all active loans (rejecting `v.push_ref(&v[0])` where an argument still holds a shared loan into `v`).
8. **Drop-Check (`dropck`) for `@resource(dispose)`**:
   - `Terminator::Drop(x)` marks `O(x)` **live** iff `ty(x)` (or any transitively owned field) has `FLAG_DISPOSE`. This prevents dropping or mutating a borrowed target before a `@resource(dispose)` guard's `dispose(&self)` observes it at scope exit (including reverse declaration-order drops).
   - Plain structs, enums, and views without `FLAG_DISPOSE` are `may_dangle`: their implicit `Drop` does NOT keep their origins live, preserving full NLL ergonomics.
9. **Scratch Regions (`region R(cap)`) & Raw-Pointer Escape (`E4005`)**:
   - `region R(cap) { ... }` introduces a lexical origin `@R` bounded by `RegionExit(R)`; `R.alloc(n)` returns a slice tied to `@R`. Standard origin-outlives constraints forbid `@R` from escaping the block.
   - An unconditional HIR call-graph pass computes `exposes_param_addr(fn_id, param_idx)` across **all** functions, traits, monomorphized generics, and closures (keyed by canonical callable `TypeId`), replacing `c.loans.active` token pre-scans and eliminating **S-12**.

### 3.2 Calls, Closures & `@resource(dispose)` Semantics

- **Strict Left-to-Right Evaluation**: Call arguments, binary operators (including `&str ==`/`!=`), slice bounds, struct/enum literals, and `replace(&mut dst, val)` lower to sequential MIR temporaries `t0, t1, ...`. Any loan held in `t0` remains active throughout `t1`'s evaluation and the consuming operation, making intra-expression aliasing bugs (**S-1, S-3, S-6**) impossible by construction.
- **Closures as Typed Capture Structs**: Every closure lowers in HIR to an anonymous struct `ClosureEnv_N { cap0: T0, ... }` + entry `fn`.
  - `Copy`, `Clone`, `Sendable`, `Sync`, and `Drop` are derived structurally from `ClosureEnv_N`'s capture types.
  - `Fn` implements `Clone` **iff** every captured field is `Clone` (allowing a `Fn` to capture a `@resource`, while statically rejecting `.clone()` on it — fixing **S-11**). `FnMut` and `FnOnce` are never `Clone`.
  - Borrowed closures (`&Fn`, `&mut FnMut`) carry the union of their captured origins and live in a typed `StackSlot` (zero heap allocation).
- **`@resource(dispose)` Rules**:
  - Both partial field moves (`r.field`) and full struct destructuring (`Res { a, b } := move r`) are forbidden in safe code everywhere on `@resource(dispose)` structs (**S-9**). Destructuring a `@resource(dispose)` struct is permitted **only inside `unsafe { ... }` within the declaring module** (e.g., `OwnedBuf::into_allocator` and `Arena::freeze` in `stdlib/alloc.ddr`, where the caller explicitly upholds the cleanup contract); safe code must use `replace(&mut self.field, replacement)` or an explicit `.close(&mut self)` with a closed-state sentinel before `dispose(&mut self)`.
  - Enum payloads containing `@resource(dispose)` are supported cleanly via MIR drop elaboration (fixing `backend_emit.ddr:187-196`).

### 3.3 Sealed Safe/Unsafe, Capability & Concurrency Contract

1. **Sealed Safe/Unsafe & Capability Boundary**:
   - Compiler intrinsics (`__sys_*`, `__sync_*`) are private to blessed stdlib modules and declared with explicit typed signatures (eliminating untyped `ret = expected`).
   - All stdlib types maintaining memory/FD/allocation invariants (`OwnedBuf`, `Bump`, `Arena`, `FrozenArena`, `Capped`, `Block`, `Layout`, `File`, `TcpStream`, etc.) MUST be `pub opaque struct` (**S-2**).
   - Raw OS file-descriptor wrappers (`sys.net_read`, `sys.net_write`, `sys.net_close`, `sys.net_poll`, `sys.fd_*`) are `unsafe fn` (**S-13**). Safe code with `capability network` uses owned `@resource` handles in `std.net`.
   - `@thread_safe` FFI declarations continue to require `capability unsafe` (already enforced at `compile.ddr:2299-2300, 2440-2441`).
2. **`Sendable` + `Sync` Trait Hierarchy**:
   - `Sendable`: safe to transfer ownership across thread/arena boundaries.
   - `Sync`: safe to share `&T` concurrently across threads.
   - **Auto-derivation:** Non-opaque structs/enums/tuples/arrays/closures automatically derive `Sendable` / `Sync` from their fields (excluding `ptr`, borrowed refs, and non-`Sendable`/`Sync` fields).
   - **Verified safe `impl Sendable / Sync for T {}`:** Allowed in safe modules (no `capability unsafe`) for non-`@resource` `opaque struct`s (e.g. `MethodIndex`, `Map`, `Set`, `toml.Tree`) and generic wrappers (`Reverse<T>`) as a SemVer stability opt-in; the compiler **structurally verifies** every private field (rejecting `@resource` and `ptr`).
   - **`unsafe impl Sendable / Sync for T {}`:** Required **only** for unchecked trust assertions (`@resource`, raw `ptr`, interior mutability — e.g. `fs.File`, `TcpStream`, `AtomicI64`, `Channel`, `Published`).
   - **Bounds:** `Published<T>` and `task scope (shared ...)` require `T: Sendable + Sync` (**S-10**).
   - **`Sync` + Non-Atomic RC Precondition (Mandatory Runtime Invariant):** `string`, `[]T`, `Map`, and `Set` are `Sync` *only* because `Published<T>` and `task scope (shared ...)` freeze all reachable RC headers (`rc <= -2`) before another thread observes `&T`. Any `unsafe` concurrency primitive exposing `&T` across threads for `T: Sync` MUST either freeze `T` first or disallow non-atomic-RC cloning (`runtime.ddr:376`).
3. **Per-Thread Arena Isolation & Safe RC Header Sentinels**:
   - Cross-thread synchronization structures (`AtomicI64`, `Semaphore`, `CancelToken`, `Channel`, `Published`, `Waker`) MUST allocate strictly from OS pages (`mmap`/`munmap` on macOS/Linux; `VirtualAlloc`/`VirtualFree` on `x86_64-windows`) — never from the per-thread arena (**R-1**).
   - **Preserved RC Header Encoding (`[ptr - 8]`)** replacing raw `-fn_ptr` `blr x16` (**R-2**):
     - `rc > 0`: normal per-thread mutable RC (**zero hot-path overhead**; fast path branches on `rc < 0` only).
     - `rc == 0`: static/immortal literal (no-op `retain` & `release`).
     - `rc == -1`: **freed-object poison sentinel** (traps on `retain` & `release` to catch double-free/UAF — strictly preserved!).
     - `rc == -2`: frozen immutable string leaf (`retain` executes `string_isolated_clone` into caller's local arena so cloned strings outlive the frozen snapshot; `release` is a no-op).
     - `rc <= -3`: frozen compound payload; `desc_id = (-rc) - 3`, bounds-checked against `TYPE_DESC_COUNT` (trap 112 on OOB) indexing a read-only `.rodata` `TYPE_DESC_TABLE[desc_id]` (`release` is a no-op). On WASM, integer `desc_id` maps directly to `__indirect_function_table` `call_indirect` if WASM gains concurrency.
4. **Unified LIFO Cleanup & Explicit Panic/Abort Contract**:
   - `owned locals`, `defer`, `region`, and `task scope` live on ONE lexical unwind stack. On `return`, `?`, `break`, `continue`, or block fallthrough, scopes unwind strictly inner-to-outer — guaranteeing `TaskScopeJoin` always completes before outer `defer` or outer `dispose` runs (**S-8**).
   - **1.0 Panic Contract (Abort-Only):** `panic()`, `assert()`, and runtime traps (`101`..`112`) abort the process immediately without unwinding (`defer`/`dispose` do NOT run on panic). Recoverable errors MUST use `Result<T, E>` + `?`, which run full LIFO cleanup.

### 3.4 Breaking Surface-Language Changes Before 1.0

1. **`std.alloc` opacity:** `OwnedBuf<A>`, `Bump<A>`, `Arena<A>`, `FrozenArena<A>`, `Capped<A>`, `Block`, and `Layout` become `pub opaque struct` (replace direct field reads/writes with `.len()`, `.cap()`, `.size()`, `.align()`, `unsafe from_raw_parts`).
2. **`std.sys` raw FDs:** `sys.net_read`, `sys.net_write`, `sys.net_close`, and `sys.net_poll` become `unsafe fn`.
3. **Concurrency traits:** Add `Sync`; require `unsafe impl Sendable` / `unsafe impl Sync` for unchecked overrides (`@resource`, `ptr`, interior mutability); require `T: Sendable + Sync` on `Published<T>` and `task scope (shared ...)`. *(Verified safe `impl Sendable`/`Sync` on non-`@resource` `opaque struct`s remains valid; `@thread_safe` already requires `capability unsafe`.)*
4. **`@resource(dispose)` destructuring:** `Res { ... } := move r` on `@resource(dispose)` structs is forbidden in safe code everywhere and permitted only inside `unsafe { ... }` within the declaring module (e.g., `OwnedBuf::into_allocator` / `Arena::freeze`).
5. **`Fn` closure cloning:** `.clone()` on a `Fn` capturing any non-`Clone` (`@resource`) value becomes a compile error.
6. **Callable types with `from`:** Function-pointer / closure types returning borrows from one of multiple inputs include `from` in the type signature.
7. **Stored-borrow mutation via `&mut` params:** Overwriting stored-borrow fields through `&mut` function parameters is rejected in 1.0.
8. **`defer` & `task scope` semantics:** Early-exit use-after-move in `defer` is rejected; outer `defer` runs strictly *after* inner `task scope` join.
9. **Panic contract:** Documented explicitly as abort-only (no `defer`/`dispose` on panic).

---

## 4. First-Principles Compiler & IR Architecture (Self-Hosted Dunder)

### 4.1 Unidirectional Pipeline (Zero Lifting, Zero Token Rewriting)

```text
Source (.ddr)
  -> Lexer + Parser (selfhost/frontend/ast.ddr)           [single parser for build/check/fmt/lsp]
  -> Name Resolution & Declaration Freeze (hir_decl)      [module graph, traits, items]
  -> Type Inference, Trait Checking & HIR Lowering        [ParamMode::BorrowOrCopy, closure structs, from-origins]
  -> HIR Monomorphization & TypeTable Seal (hir_mono)     [single unified TypeTable; zero declared/body split]
  -> High-Level Typed CFG MIR (mir_build)                 [explicit CFG, Places, Borrows, Calls, Drops, shared Defer, TaskScope]
       |-- MirVerifier (HL invariants)
       |-- Definite-Init & Partial-Move Check (mir_init)
       |-- CFG Borrow & Origin Checker (mir_borrowck)     [Polonius-lite bitsets, invariance, dropck; zero token scan]
       |-- Raw-Pointer / Scratch Escape Check (mir_escape)[unconditional over all callables incl. closures]
       |-- Drop & Defer Elaboration (mir_drop_elab)       [minimal drop flags; unconditional drops; deletes mir_owners]
       `-- MirVerifier (post-drop invariants)
  -> Target-Independent SSA MIR (mir_ssa_lower)           [typed virtual values, StackSlot(size, align), zero ARM64 ISA]
       |-- Unified SSA MIR Inliner                        [deletes selfhost/opt/*, prefixing, stack_option_argument]
       |-- SROA / Mem2Reg / CopyProp / GVN / ConstFold
       |-- Bounds & Overflow Check Elision / DCE
       `-- MirVerifier (SSA dominance, types, effects after every pass)
  -> Target ABI & Native LIR/ISel (parallel per function, cached in fragments.ddr)
       |-- arm64_isel -> stateless arm64_enc.ddr -> ObjectModule -> macho.ddr / elf.ddr / dwarf.ddr
       |-- x86_isel   -> stateless x86_enc.ddr   -> ObjectModule -> elf.ddr / pe.ddr / dwarf.ddr (native SysV/Win64)
       `-- wasm_isel  -> wasm_bytes.ddr (LEB128) -> ObjectModule -> wasm_memory.ddr / wasm_abi.ddr (native locals)
  -> Runtime & Type Helpers (compiled through SSA MIR -> Target LIR)
  -> Target-Neutral Linker (mir_link.ddr consuming ObjectModule)
```

**Parallelism & Incremental Build Invariant:** `hir_mono` outputs an immutable frozen `HirProgram` + `TypeTable` (replacing `frozen_source.ddr`). Every function body from `mir_build` through `*_isel` runs independently across `task scope` workers and caches deterministically by content hash in `fragments.ddr`.

### 4.2 Concrete Self-Hosted Dunder Data Structures (Flat Arena / SoA)

Designed for Dunder's unboxed PODs (`<= 128B`), zero pointer chasing, and zero cyclic RC:

```dunder
// --- Sealed Unified TypeTable (post-monomorphization; replaces declared/body split) ---
pub enum TypeKind {
    Scalar, Struct, Enum, Tuple, Array, FixedArr,
    Ref, MutRef, FatSlice, FatStr,        // FatSlice & FatStr are 2-word Copy PODs (ptr, len)
    FnPtr, ClosureEnv, RegionSlice,
}

pub enum ParamMode { CopyVal, MoveVal, SharedBorrow, MutBorrow, BorrowOrCopy }

pub const PARAM_ENV: i32 = -1             // sentinel in FromEdge for closure capture environment

pub struct FromEdge {
    param_idx: i32,                       // 0..N-1 or PARAM_ENV (-1)
    field_slot: i16,                      // -1 for whole param, or borrow-field origin slot
    ret_slot: i16,                        // target origin slot on return type
}

pub struct TypeEntry {
    kind: TypeKind,
    size: i32, align: i32,
    flags: u32,                           // COPY | RESOURCE | DISPOSE | SENDABLE | SYNC | OPAQUE | C_LAYOUT | POD_INLINE
    inner_a: i32, inner_b: i32,           // elem/pointee/field_start/param_start
    count: i32,                           // field_count / param_count / fixed_len
    origin_count: i16,                    // positional origin slots on this type
    from_start: i32, from_len: i16,       // slice into TypeTable.from_edges (for callables)
}

// --- High-Level Typed CFG MIR (per function, flat i32-indexed arenas) ---
pub struct MirLocal {
    ty: i32,                              // TypeId in sealed TypeTable
    size: i32, align: i32,                // exact layout (eliminates 8-byte BorrowHome hack)
    scope: i32,                           // ScopeId
    flags: u16,                           // MUT | PARAM | MOVE_PARAM | TEMP | USER | ADDR_TAKEN | DROP_FLAG | DEFER_FLAG
    origin_start: i32,                    // slice into MirBody.local_origins
    span_start: i32, span_end: i32,
}

pub enum ProjKind { Deref, Field, Index, Downcast } // 4B Copy POD
pub struct PlaceProj { kind: ProjKind, index: i32 } // 8B Copy POD
pub struct MirPlace {
    local: i32,
    proj_start: i32, proj_len: i16,
    ty: i32,                              // cached projected TypeId
}

pub enum BorrowKind { Shared, Mut, TwoPhaseMut }
pub struct Loan {
    place: i32,                           // PlaceId
    kind: BorrowKind,
    origin: i32,                          // OriginId created by this borrow
    def_bb: i32, def_stmt: i32,
    region_scope: i32,                    // -1 or ScopeId of enclosing `region R`
    span_start: i32, span_end: i32,
}

pub enum ScopeKind { Lexical, Defer, Region, TaskScope }
pub struct ScopeNode {
    parent: i32,                          // -1 for function root
    kind: ScopeKind,
    cleanup_bb: i32,                      // shared defer_bb / task_scope join / region exit
    active_flag_local: i32,               // -1 or bool local reset at enclosing loop entry
    depth: i16,
}

pub enum Operand { Copy(i32), Move(i32), Const(i32) } // 8B Copy POD

pub enum StmtKind {
    Assign,                               // place = rvalue
    StorageLive, StorageDead,
    SetDiscriminant,                      // enum tag write
    SetDeferActive,                       // bool local = true/false (reset at loop-body entry)
    SetUnwindTarget,                      // integer route selector for shared defer_bb SwitchInt
    Nop,
}

pub enum RvalueKind {
    Use, Ref, Slice, BytesView,
    Binary, Unary, Cast,
    Aggregate, Closure,
    Len, Discriminant,
    AddrOf,                               // unsafe raw pointer (consumed by mir_escape)
}

pub enum TermKind {
    Goto, SwitchInt,
    Call,                                 // Direct / Indirect / Extern / Intrinsic, args, dest, normal_bb
    Drop,                                 // place_id, next_bb (elaborated in mir_drop_elab)
    TaskScopeEnter, TaskScopeJoin,        // TaskScopeJoin always emitted before outer Defer/Drop
    RegionEnter, RegionExit,
    Return, Trap,
}

pub struct BasicBlock {
    stmt_start: i32, stmt_end: i32,
    term_id: i32,
    pred_start: i32, pred_end: i32,
    scope: i32,
}

pub struct OutlivesEdge {
    from_origin: i32, to_origin: i32,
    bb: i32, stmt_idx: i32,
    bidirectional: bool,                  // true for invariant origins behind &mut T / &mut FnMut
}

pub struct MirBody {
    fn_id: i32,
    locals: []MirLocal,
    places: []MirPlace,
    projs: []PlaceProj,
    loans: []Loan,
    scopes: []ScopeNode,
    stmts: []MirStmt,
    rvalues: []MirRvalue,
    ops: []Operand,
    terms: []MirTerm,
    blocks: []BasicBlock,
    local_origins: []i32,
    outlives: []OutlivesEdge,
    origin_count: i32,
}

// --- Target-Neutral Linker Object (replaces mir_link.ddr's arm.Code-backed LinkedImage) ---
pub struct ObjectModule {
    target: TargetKind,
    text: []u8, rodata: []u8, data: []u8,
    symbols: []ObjSymbol,
    relocs: []ObjReloc,
    imports: []ObjImport,
    type_descs: []TypeDescEntry,          // populates .rodata TYPE_DESC_TABLE
    wasm_fn_types: []WasmSig,             // per-signature WASM call_indirect types
    debug_lines: []DebugLine,
}
```

### 4.3 Concrete CFG Borrow-Check & Drop-Elaboration Algorithm

All dataflow sets use packed `[]u64` bitsets (typically 1–2 `u64` words per function -> microseconds in native Dunder).

1. **CFG & Shared `defer` Construction (`mir_build.ddr`)**:
   - Maintain a compile-time `scope_stack: []ScopeId`.
   - For each `defer` block, emit **one shared `defer_bb`** (preventing exponential MIR blowup on nested `defer`s with many `?`/`return` sites) terminated by a `SwitchInt` on an integer `unwind_target` local routing to the next enclosing cleanup/exit target.
   - Any conditional `defer_active_flag` local inside a loop body MUST be reset via `SetDeferActive(flag, false)` at **loop-body entry** (not function entry) so iteration `k+1` cannot observe iteration `k`'s flag.
   - Every `return`, `?`, `break`, `continue`, and block exit walks `scope_stack` inner-to-outer emitting `Drop`, `TaskScopeJoin`, `RegionExit`, or branching to `defer_bb` with the appropriate `unwind_target`. `TaskScopeJoin` is structurally guaranteed to precede outer `defer_bb` and outer `Drop` (**S-5, S-8**).
   - Lower `for x in expr` over `Iterator` explicitly to `mut iter = expr; loop { match iter.next() { .Some(x) => body, .None => break } }` so `iter`'s origin slots stay live across the entire loop CFG (**S-4**).

2. **Pass 1 — Canonical Place Trie & Definite-Init (`mir_init.ddr`)**:
   - Build a sparse `PlaceNode` trie over `(local, field*, downcast*)` appearing in `MirBody`.
   - Forward CFG worklist over `(maybe_init, maybe_uninit)` bitsets:
     - `Assign(p, _)`: sets `p` and descendants to `(1, 0)`.
     - `Operand::Move(p)`: requires `maybe_uninit[p_or_descendants] == 0`; rejects partial/destructuring moves if any ancestor has `FLAG_DISPOSE` outside `unsafe { ... }` in the declaring module (**S-9**); sets `p` and descendants to `(0, 1)`.
     - `Copy`/`Ref`/`Slice`: requires `maybe_uninit[p_or_ancestors_or_descendants] == 0`.
     - Reaching `defer_bb` from any early exit joins all reaching states -> moving `s` before `return`/`?`/`break` triggers `E4001` immediately (**S-5**).

3. **Pass 2 — CFG NLL / Polonius-Lite Borrow Checker (`mir_borrowck.ddr`)**:
   - **Step A — Backward Origin Liveness (`live_origins[bb_exit]`)**:
     - Standard backward CFG liveness over `MirLocal` IDs (zero token scanning -> eliminates **C-2, C-3**).
     - **Drop-check (`dropck`) rule:** `Terminator::Drop(x)` treats `x` as a live use of `O(x)` **iff** `ty(x)` (or any transitively owned field) has `FLAG_DISPOSE`; non-`dispose` structs/views are `may_dangle` and do not mark `O(x)` live at `Drop`.
     - Map live locals to `live_origins = ⋃_{x live} origins_of(x) ∪ O_ext` (return/external origins).
   - **Step B — Outlives & Invariance Constraint Collection**:
     - `Ref`/`Slice`/`BytesView` creating `Loan L` with `O_L` assigned to `dst`: seed `L ∈ O_L` and emit `O_L -> O_dst`.
     - `Aggregate`/`Closure`/`Assign(dst, Use(src))`: emit `O_src -> O_dst` for covariant origin slots; emit **bidirectional `O_src <-> O_dst`** for any origin slot inside `T` behind `&mut T` or `&mut FnMut`.
     - `Call(f, args, dest)`: for every `FromEdge` in `f`'s `TypeEntry` (including `PARAM_ENV` for closure captures), emit `O(arg/env, field_slot) -> O(dest, ret_slot)` (and bidirectional edges for invariant `&mut` parameter arguments).
     - Reborrowing `r2 = &(*r1).f`: emit `O(r1) -> O(r2)` and link `L(r2)`'s place to `(*r1).f`.
   - **Step C — Block-Boundary Forward Loan Propagation (Fast Compile Guardrail)**:
     - **Memory/speed invariant:** Materialize `contains[bb_entry][O] : BitSet<LoanId>` **at basic-block entries ONLY** (never per statement — `O(BBs × Origins × Loans/64)`).
     - Within each basic block, stream statement-by-statement in a single reusable scratch matrix:
       - On full overwrite `Assign(r, rhs)` of a non-borrowed `mut` local `r`, **kill** `contains[O_r]` before applying `rhs`'s outlives edges (enabling precise `mut r: &T` rebinding).
       - Seed new loans, close transitively over active `OutlivesEdge`s (both directions when `bidirectional == true`), and kill dead origins not in `live_origins ∪ O_ext`.
     - Iterate worklist over CFG blocks until `contains[bb_entry]` reaches fixpoint.
   - **Step D — Uniform Conflict & Escape Check (Single Streaming Pass Over Each Block)**:
     - At each statement/terminator `P`, compute `active_loans = ⋃_{O ∈ live_origins[P] ∪ O_ext} contains[O]`.
     - Resolve every `Place` through `Deref` chains to its supporting prefixes (walking `&mut` alias `r` to its underlying root while preserving the deref step).
     - Check every access `(kind, access_place)` against `L ∈ active_loans`:
       - Reborrowing `*r` through `L`'s own `&mut` binding `r` is allowed unless a conflicting *child* loan derived from `*r` is active (**S-3, C-1**).
       - If `L.kind == TwoPhaseMut` and `P` is before `L`'s `Call` activation: allow `Read`/`SharedRef`; reject `Write`/`Move`/`MutRef`/second `TwoPhaseMut`. At `L`'s `Call` activation: re-check full `Mut` conflict against all other active loans.
       - Otherwise, if `places_overlap(access_place, L.place)` AND (`L.kind == Mut` || `kind ∈ {Write, Move, MutRef}`) -> reject `E4003` (**S-1, S-3, S-4, S-6**).
     - Check `Return` (every loan in `O_ret` must come from a declared `from` source) and `RegionExit(R)` (no loan with `region_scope == R` may remain active).

4. **Pass 3 — MIR Drop Elaboration (`mir_drop_elab.ddr`)**:
   - Uses Pass 1's `(maybe_init, maybe_uninit)` at each `Drop(p)`:
     - `maybe_init == 0`: delete `Drop(p)`.
     - `maybe_uninit == 0`: emit unconditional typed drop call (zero null check, zero prologue zeroing).
     - Conditional (`1, 1`): emit 1-byte `DROP_FLAG` local toggled on move/init guarding the drop call.
   - Deletes `mir_owners.ddr` and `compile.ddr`'s jump-to-end zeroing prologues completely.

5. **Semantic `MirVerifier` (Enforced After HL-MIR, Drop-Elab, and Every SSA Pass)**:
   - CFG integrity (`preds`, valid jump/switch targets, single terminator per block).
   - Strict `TypeId` equality across all `Assign`, `Rvalue`, `Operand`, `Place` projections, and `Call` signatures.
   - Exact `StackSlot(size, align)` bounds on every memory access (no 8B `BorrowHome` assumptions, no barrier exemptions).
   - SSA dominance & phi/block-param arity in SSA MIR.
   - Post-drop cleanliness (zero `Drop` terminators remaining; balanced `TaskScopeEnter`/`TaskScopeJoin` and `RegionEnter`/`RegionExit` on all paths).

### 4.4 Call, View, FFI Callback & Backend Decoupling Architecture

1. **Internal Call & View ABI**:
   - `&str` and `&[]T` pass/return in 2 registers (`(ptr, len)`); aggregate `sret` triggers strictly by byte size (`> 16B` on ARM64/SysV x86_64, `> 8B` on Win64).
   - Closures pass `env_ptr` as hidden ABI arg 0; owned closures carry `[entry_fn, drop_fn, caps...]`; borrowed closures use a typed `StackSlot`.
   - Native per-target calling conventions in Phase 4 (`x0..x7` on ARM64; SysV `rdi..r9`/`xmm0..7` and Win64 `rcx..r9` + 32B shadow on x86_64; native typed locals + per-signature `call_indirect` type indices on WASM).

2. **Thread-Safe, Re-Entrant FFI Callback Bridge (Zero Custom Linker TLS Changes)**:
   - Custom linkers (`elf.ddr`, `macho.ddr`, `pe.ddr`) emit no native TLS sections (`PT_TLS`/TLV/`.tls`). Instead, use OS dynamic TLS keys via already-imported system libraries:
     - macOS / Linux: `pthread_key_create`, `pthread_getspecific`, `pthread_setspecific`.
     - Windows (`kernel32.dll`): `TlsAlloc`, `TlsGetValue`, `TlsSetValue`.
     - WASM: single-threaded linear memory with `__indirect_function_table` (no TLS required).
   - Initialize one global `cb_key` at `_start` **only when `has_callbacks` is true**.
   - At every `@extern` call site when `has_callbacks` is true (required even when the call takes no `callback(...)` arg, e.g. `objc_msgSend(app, sel("run"))` invoking a callback previously registered via `class_addMethod`; leaf C calls may opt out later via `@no_callback`):
     1. Allocate a stack-local **`CallbackFrame { prev: ptr, saved_regs: [x19..x24] / r15 }`** in the calling Dunder frame (zero heap allocation, unbounded re-entrancy, zero thread-exit destructors).
     2. Read `frame.prev = tls_get(cb_key)`, write `tls_set(cb_key, &frame)`.
     3. Invoke the foreign function.
     4. Reload `x19..x24` (or `r15`) from `frame` (preserving any arena growth done inside callbacks!) and restore `tls_set(cb_key, frame.prev)`.
   - On callback trampoline entry: `frame = tls_get(cb_key)`; if `frame == NULL` -> **trap 111** (rejects foreign OS threads safely); load Dunder runtime registers from `frame`; call Dunder callback body; write updated `x19..x21` back to `frame` before returning to C. Fixes **S-7** with zero cross-thread contention.

3. **Phase 3 Backend & Runtime Decoupling Prerequisites (Required Before Phase 4 Native ISel)**:
   - **Port Runtime & Generated Type Helpers to SSA MIR / `.ddr`**: Today `selfhost/runtime/{runtime.ddr, program_emit.ddr}` and `selfhost/frontend/backend_emit.ddr` emit raw ARM64 `arm.Code` expecting `x0..x7`/`x19..x24`, translated via `[r15]` bank on x86_64 and globals `0..31` on WASM. Switching x86_64/WASM user code to native ABIs requires porting `runtime.ddr`, `program_emit.ddr`, `backend_emit.ddr`, `task_scope_emit.ddr`, and `task_native.ddr` to emit **SSA MIR** (or pure `.ddr` compiled through SSA MIR -> target LIR), leaving only tiny target-specific `_start`/syscall/TLS stubs.
   - **Extract Stateless Encoders & `ObjectModule`**: Split `arm64.ddr` into stateless `arm64_enc.ddr`; extract standalone stateless `x86_enc.ddr` (and rewrite `_start`, `memory_equal`, `memory_zero`, `windows_arguments` to native SysV/Win64 regs); delete `wasm_bytes.ddr`'s ARM64 global-bank helpers (`read`/`write`/`base`/`set_base`); replace `mir_link.ddr`'s `LinkedImage { code: arm.Code }` with target-neutral `ObjectModule`.

---

## 5. Complete Traceability Matrix & Staged Self-Host Roadmap (`0.13.2 -> 1.0`)

### 5.1 Traceability Matrix (Every Confirmed Bug, Cliff & Architectural Flaw)

| ID | Issue (`0.13.1`) | Phase 0 Stopgap (`0.13.2`) | Permanent 1.0 First-Principles Invariant |
|---|---|---|---|
| **S-1** | `Option<&mut T>` call-arg aliasing bypass (P0 UAF) | Include `ft.mutable_option` in `invoke_with_args` | Uniform `Origin` slots on all types + sequential MIR arg temps + single `active_loans` conflict rule. |
| **S-2** | `std.alloc` non-opaque `pub struct` OOB R/W (P0) | Make `OwnedBuf`, `Bump`, `Arena`, `FrozenArena`, `Capped`, `Block`, `Layout` `pub opaque struct` | Sealed Trust Boundary: all invariant-bearing stdlib types are `pub opaque struct`. |
| **S-3** | `&mut` alias mutation missed by `mutated_overlap` (P0 UAF) | Log resolved `source_root` + grouped roots in `record_mutation` | Deref-chain resolution in `mir_borrowck` (`*r` child loans conflict with `&mut *r` / `r` mutation) + sequential MIR temps. |
| **S-4** | `for x in <temp Iterator>` drops locks (P0 UAF) | Lock temp `iter_mode == 5` stored-borrow provenance | `for` lowers to explicit `mut iter = expr; loop { iter.next() }` in MIR; `iter`'s origin slots stay live across loop CFG. |
| **S-5** | `defer` early-exit use-after-move (P0 SIGSEGV) | Check `defer` against `ownership` at `return`/`?`/`break`/`continue` | Shared `defer_bb` wired onto every exit CFG edge before `mir_init` and `mir_borrowck`. |
| **S-6** | `&str ==` / `!=` skips `mutated_overlap` (P0 UAF) | Add `mutated_overlap` check to `&str` comparisons | Strict left-to-right MIR temp lowering (`t0 = lhs; t1 = rhs; eq(t0, t1)`): `t0`'s loan is active during `t1`. |
| **S-7** | Global callback save slot races workers (P0 crash 109) | OS dynamic TLS key + stack-linked `CallbackFrame` | OS dynamic TLS (`pthread_key_*`/`Tls*`) + stack-linked `CallbackFrame { prev, regs }`; zero global mutable slot. |
| **S-8** | Outer `defer` runs before `task scope` join (P1) | Move `task_close_slot`/`locals` before outer `defer` | Unified LIFO `ScopeNode` stack in `mir_build`: `TaskScopeJoin` always precedes outer `defer_bb` and `Drop`. |
| **S-9** | Destructuring `@resource(dispose)` skips `dispose()` (P1) | Reject `@resource(dispose)` destructuring in safe code in `bind_struct` (allow only inside `unsafe {}` in declaring module) | `mir_init` rejects partial/destructuring moves on any `FLAG_DISPOSE` ancestor in safe code (permitted only inside `unsafe {}` in declaring module). |
| **S-10** | Safe `impl Sendable` on `@resource` + `Published<T>` without `Sync` (P1) | Require `unsafe impl Sendable` on `@resource`; add `Sync` bound | Split `Sendable`/`Sync`; verified safe `impl` only for non-`@resource` `opaque struct`; `unsafe impl` for overrides; `Published<T: Sendable + Sync>`. |
| **S-11** | `Fn` capturing `@resource` allows `.clone()` (P1) | Check capture `Clone` on `Fn.clone()` | Closures are typed `ClosureEnv_N` structs; `Clone` derived structurally iff all captures are `Clone`. |
| **S-12** | Scratch/scoped raw-pointer escape via closures (P1) | Normalize closure kind `11..13` <-> `9` + body sigs in `scratch.Loans` | Unconditional HIR call-graph `exposes_param_addr` over canonical callable `TypeId`s (incl. closures). |
| **S-13** | Raw-FD `sys.net_*` callable in safe code (P1) | Mark `sys.net_read/write/close/poll` `unsafe fn` | All raw-FD syscall wrappers are `unsafe fn`; safe callers use `std.net`/`std.fs` `@resource` types. |
| **C-1** | `match move opt` on `Option<&mut []T>` rejects `m.clear()` | Fix `match move` `&mut` binding reborrow | `m` is an initialized `&mut []T` local; `m.clear()` lowers to `&mut *m` reborrow, permitted by `mir_borrowck`. |
| **C-2** | Token-scan NLL false positive on `Foo { r: 1 }` | *(Resolved in Phase 2)* | CFG liveness over `MirLocal` IDs; zero token scanning. |
| **C-3** | Token-scan NLL false positive on disjoint `if`/`else` | *(Resolved in Phase 2)* | Backward CFG liveness kills `r`'s origin on branches without forward use. |
| **R-1** | Windows `sync_mmap`/`waker_mmap` uses per-thread pool | Switch Windows `sync_mmap`/`waker_mmap` to `VirtualAlloc`/`VirtualFree` | All cross-thread sync allocations + `alloc.System` use OS pages on macOS/Linux/Windows. |
| **R-2** | Negative RC `< -2` runs raw `blr -rc` (RCE hazard) | Switch `rc <= -3` to bounds-checked `TYPE_DESC_TABLE[(-rc)-3]` | Preserved sentinels (`-1` freed trap, `-2` frozen string clone, `<= -3` `.rodata` `TYPE_DESC_TABLE` index). |
| **R-3** | Hand-encoded ARM64 task runtime + macOS `x29` + x86 8MiB stack | *(Resolved in Phase 3)* | Structured SSA MIR / `.ddr` task runtime + clean per-target thread entry stubs. |
| **R-4** | Undocumented abort-only panics | Document in `docs/memory.md` | Explicit 1.0 abort-only panic contract; `Result` + `?` for recoverable LIFO cleanup. |
| **IR-1..5** | 48B view descriptors, 8B `BorrowHome`, `selfhost/opt/*`, ARM64-in-MIR, `arm.Code` coupling | *(Resolved in Phases 1–4)* | 2-word `(ptr, len)` views, typed `StackSlot(size, align)`, full `MirVerifier`, SSA MIR inliner, `ObjectModule`, native target ISel. |

### 5.2 Staged Self-Host Migration & Bootstrap Plan (`0.13.2 -> 1.0`)

Every phase MUST maintain `tools/bootstrap.sh` (seed -> stage1 -> stage2 == stage3 byte-identical), `tools/verify-selfhost.sh`, parallel builds, and all 6 targets:

#### Phase 0 (`0.13.2`) — Immediate P0/P1 Stopgap Patch Release
- Land all Phase 0 stopgaps from the Traceability Matrix (**S-1..S-13, C-1, R-1, R-2, R-4**).
- Add all 12+ adversarial reproducers as permanent regression tests in `tests/`.
- Verify via `tools/bootstrap.sh` + `tools/verify-selfhost.sh` on macOS ARM64 and Kiri Linux ARM64/x86_64.

#### Phase 1 (`0.14`) — Dual-Check V2 Frontend (`AST -> Typed HIR -> HL-MIR`, Zero Throwaway Codegen)
*Do NOT build a throwaway `HIR -> legacy compile.ddr` emitter.*
1. Complete `selfhost/frontend/ast.ddr` + `parser.ddr` for full language coverage and unify parsing across `build`, `check`, `fmt`, and `lsp`.
2. Build `selfhost/hir/` (name resolution, type inference with `ParamMode::BorrowOrCopy`, trait checking, non-lending `Iterator` enforcement, first-class closure structs with `PARAM_ENV`, HIR monomorphization, single sealed `TypeTable`, typed internal `__sys_*` declarations).
3. Build `selfhost/mir2/` HL-MIR pipeline (`mir_build` with shared `defer_bb` & LIFO `ScopeNode`, `mir_verify`, `mir_init`, Polonius-lite `mir_borrowck` with invariance & `dropck`, `mir_escape`, `mir_drop_elab`).
4. Expose as `--check-v2` running side-by-side with legacy `compile.ddr` (which continues codegen untouched). Validate 100% parity across `selfhost/`, `stdlib/`, `tests/`, and `../dunder-hundmat`, confirming **C-1..C-3** are resolved and zero false regressions exist.

#### Phase 2 (`0.15`) — Single Thin Cutover Bridge & Legacy Frontend Deletion
1. Implement a single thin (~600–900 LOC) **`Post-Drop HL-MIR -> arm.Code`** bridge (post-drop HL-MIR has only ~15 flat statements/terminators, zero borrow state, zero drop-flag computation, and zero token lookahead) plus 2-word `(ptr, len)` views (`&str`, `&[]T`).
2. Switch all 6 targets to the V2 frontend through this thin bridge, verifying 2-stage byte-identical self-hosting on all 3 bootstrap hosts (`arm64-macos`, `arm64-linux`, `x86_64-linux`).
3. **Delete in one clean cutover:** `compile.ddr`, `generics.ddr`, `closures.ddr`, `variant_functions.ddr`, `open_bodies.ddr`, `ownership_check.ddr`, `local_borrows.ddr`, `borrows.ddr`, `ownership_lease.ddr`, `deferred_blocks.ddr`, `scratch.ddr`, `borrowed_view.ddr`, `mir_owners.ddr`, and `selfhost/opt/*`.
4. Refresh frozen bootstrap seeds.

#### Phase 3 (`0.16`) — Target-Independent SSA MIR Optimizer & Runtime/Encoder Decoupling
1. Lower Post-Drop HL-MIR -> Target-Independent SSA MIR (typed virtual values, explicit `StackSlot(size, align)`, zero ARM64 regs/flags) with `MirVerifier` enforced after every pass.
2. Implement unified SSA MIR inliner, SROA/mem2reg, GVN/const-prop, bounds/overflow check elision, and DCE. Delete `mir_lift.ddr`, `mir_expr_stack.ddr`, `mir_retained.ddr`, `mir_preserve.ddr`, `mir_call_inputs.ddr`, `mir_source_call.ddr`, `stack_option_argument`, and `catalog.predicates`/`converters`.
3. **Backend/runtime decoupling prerequisites:**
   - Port `backend_emit.ddr` (generated clone/drop/iso helpers), `selfhost/runtime/{runtime.ddr, program_emit.ddr}`, and `selfhost/tasks/{task_scope_emit.ddr, task_native.ddr}` to emit SSA MIR / pure `.ddr` (fixing macOS `x29` and x86_64 unused worker stack).
   - Extract stateless `arm64_enc.ddr` and `x86_enc.ddr`; strip ARM64 global-bank ops from `wasm_bytes.ddr`; replace `mir_link.ddr`'s `arm.Code`-backed `LinkedImage` with target-neutral `ObjectModule`.
4. Gate: byte-identical selfhost + `--exhaustive` MIR oracle + `dunder-hundmat` benchmark parity.

#### Phase 4 (`0.17`) — Native Target LIR/ISel for ARM64, x86_64 & WASM
1. Lower SSA MIR directly to native per-target LIR + regalloc:
   - `arm64_isel` -> `arm64_enc.ddr` -> `ObjectModule` -> `macho.ddr` / `elf.ddr` / `dwarf.ddr`.
   - `x86_isel` (native SysV & Win64 ABI, deleting `[r15]` virtual register bank emulation) -> `x86_enc.ddr` -> `ObjectModule` -> `elf.ddr` / `pe.ddr` / `dwarf.ddr`.
   - `wasm_isel` (native typed Wasm locals + per-signature `call_indirect`, deleting global `0..31` emulation) -> `wasm_bytes.ddr` -> `wasm_memory.ddr` / `wasm_abi.ddr`.
2. Update `fragments.ddr` and `compile_parallel.ddr` to cache and compile per-function `HIR -> MIR -> ObjectModule` fragments.
3. Gate: full 6-target verification (macOS ARM64, Kiri Linux ARM64 & x86_64, native Windows x86_64, Node WASM/WASI) + <100 ms p95 incremental build check.

#### Phase 5 (`0.18 -> 1.0 RC`) — Adversarial Verification, Spec Lock & Seed Freeze
1. Re-run full adversarial soundness suite (215+ probes + CFG borrow/drop/concurrency fuzzing across all 6 targets) and full `dunder-hundmat` dogfooding.
2. Synchronize `docs/memory.md`, `docs/arkitektur.md`, `docs/ffi.md`, `docs/tasks.md`, and `docs/syntax.md` with finalized 1.0 contracts.
3. Generate and verify final byte-identical 1.0 bootstrap seeds across all 3 hosts and tag `1.0.0`.
