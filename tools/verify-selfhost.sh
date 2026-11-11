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
"$compiler" test tests/mir_lower.ddr -o "$work/mir-lower"
"$compiler" test tests/mir_retained.ddr -o "$work/mir-retained"
"$compiler" test tests/mir_dataflow.ddr -o "$work/mir-dataflow"
"$compiler" test tests/mir_preserve.ddr -o "$work/mir-preserve"
"$compiler" test tests/mir_scalar.ddr -o "$work/mir-scalar"
python3 tools/test-mir.py "$compiler"
python3 tools/test-mir-promotion.py "$compiler"
python3 tools/test-mir-scalar.py "$compiler"
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
