#!/bin/sh
# Rebuild the host compiler, then exercise its public CLI and target runtimes.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
compiler=$(./tools/bootstrap.sh)
work=$(mktemp -d "${TMPDIR:-/tmp}/dunder-verify.XXXXXX")
trap 'rm -rf "$work"' EXIT
trap 'exit 1' HUP INT TERM
"$compiler" test dogfood/tests.ddr -o "$work/dogfood"
"$compiler" test tests/mir.ddr -o "$work/mir"
"$compiler" test tests/mir_verify.ddr -o "$work/mir-verify"
"$compiler" test tests/mir_lower.ddr -o "$work/mir-lower"
"$compiler" test tests/mir_object.ddr -o "$work/mir-object"
python3 tools/test-mir-objects.py "$compiler"
# Metadata/emitter units execute portably; OS probes and source tasks execute
# only on the implemented native host provider.
python3 tools/test-mir-link.py "$compiler"
python3 tools/test-mir-fragments.py "$compiler"
python3 tools/test-compiler-session.py "$compiler"
"$compiler" test tests/mir_pipeline.ddr -o "$work/mir-pipeline"
"$compiler" build-tests tests/mir_pipeline.ddr --target wasm32 -o "$work/mir-pipeline.wasm"
node tools/test-wasm.mjs --execute "$work/mir-pipeline.wasm"
python3 tools/test-mir-pipeline.py "$compiler" --report build/mir-pipeline-integrated.json
for ssa_module in mir_call_inputs mir_liveness_inputs mir_ssa_cells mir_ssa mir_ssa_sparse; do
    "$compiler" test "tests/$ssa_module.ddr" -o "$work/$ssa_module"
    "$compiler" build-tests "tests/$ssa_module.ddr" --target wasm32 -o "$work/$ssa_module.wasm"
    node tools/test-wasm.mjs --execute "$work/$ssa_module.wasm"
done
python3 tools/test-mir-frame-exits.py "$compiler" --output build/mir-frame-exits-integrated
python3 tools/test-mir-call-flags.py "$compiler" --output build/mir-call-flags-integrated
python3 tools/test-mir-ssa.py "$compiler" --report build/mir-ssa-integrated.json
python3 tools/test-ssa-source.py "$compiler"
python3 tools/test-record-take.py "$compiler"
python3 tools/test-record-take-source.py "$compiler"
for task_module in task_data task_effects task_native task_scope_emit; do
    "$compiler" test "tests/$task_module.ddr" -o "$work/$task_module"
    "$compiler" build-tests "tests/$task_module.ddr" --target wasm32 -o "$work/$task_module.wasm"
    node tools/test-wasm.mjs --execute "$work/$task_module.wasm"
done
if [ "$(uname -s)" = Darwin ] && [ "$(uname -m)" = arm64 ]; then
    python3 tools/test-task-native.py "$compiler"
    python3 tools/test-task-scope.py "$compiler"
    python3 tools/test-task-source.py "$compiler"
    # bootstrap.sh already proves the standard parallel entry's fixed point.
    # Compare its 1/2/N paths with the separately bootstrapped serial entry.
    serial_compiler="$root/build/selfhost/arm64-macos/dunder-serial"
    python3 tools/test-parallel-compiler.py "$compiler" --baseline "$serial_compiler" --output build/parallel-compiler-qa
else
    echo 'BLOCKED native task execution: first provider requires ARM64/macOS'
fi
"$compiler" test tests/mir_retained.ddr -o "$work/mir-retained"
"$compiler" test tests/mir_dataflow.ddr -o "$work/mir-dataflow"
"$compiler" test tests/mir_preserve.ddr -o "$work/mir-preserve"
"$compiler" test tests/mir_scalar.ddr -o "$work/mir-scalar"
"$compiler" test tests/mir_memory.ddr -o "$work/mir-memory"
"$compiler" test tests/mir_value_prop.ddr -o "$work/mir-values"
"$compiler" build-tests tests/mir_verify.ddr --target wasm32 -o "$work/mir-verify.wasm"
node tools/test-wasm.mjs --execute "$work/mir-verify.wasm"
"$compiler" build-tests tests/mir_value_prop.ddr --target wasm32 -o "$work/mir-values.wasm"
node tools/test-wasm.mjs --execute "$work/mir-values.wasm"
python3 tools/test-mir.py "$compiler"
python3 tools/test-mir-promotion.py "$compiler"
python3 tools/test-mir-entry.py "$compiler"
python3 tools/test-mir-scalar.py "$compiler"
python3 tools/test-mir-memory.py "$compiler"
python3 tools/test-mir-values.py "$compiler"
python3 tools/test-diagnostics.py "$compiler"
python3 tools/test-check.py "$compiler"
python3 tools/test-optimizer.py "$compiler" --verify-selection
python3 tools/test-opt-emitter.py "$compiler"
python3 tools/test-register-comparisons.py "$compiler"
python3 tools/test-array-addresses.py "$compiler"
python3 tools/test-expression-emitter.py "$compiler"
python3 tools/test-inline-catalog.py "$compiler"
python3 tools/test-inline-producer.py "$compiler"
python3 tools/test-fragment-dependencies.py "$compiler"
python3 tools/test-utf8-next.py "$compiler"
python3 tools/test-inline-streaming.py "$compiler"
python3 tools/test-const-div.py "$compiler" --verify-selection
python3 tools/test-frame-free.py "$compiler"
python3 tools/test-range-emitter.py "$compiler"
python3 tools/test-range-analysis.py "$compiler"
python3 tools/test-array-capacity.py "$compiler"
python3 tools/test-machine-ir.py "$compiler"
python3 tools/test-ownership-leases.py "$compiler"
python3 tools/test-tabell.py "$compiler"
python3 tools/test-cli.py "$compiler"
python3 tools/test-resolver.py "$compiler"
python3 tools/test-conditionals.py "$compiler"
python3 tools/test-runtime-profile.py "$compiler"
python3 tools/profile-dogfood.py check "$compiler"
python3 tools/test-files.py "$compiler"
python3 tools/test-ffi.py "$compiler"
python3 tools/test-native.py "$compiler"
python3 tools/runtime-contracts.py "$compiler"
node tools/test-wasm.mjs "$compiler"
node tools/test-wasm-scalars.mjs "$compiler"
node tools/test-wasm-abi.mjs "$compiler"
python3 tools/runtime-contracts.py "$compiler" --target wasm32
