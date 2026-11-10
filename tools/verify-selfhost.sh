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
python3 tools/test-diagnostics.py "$compiler"
python3 tools/test-tabell.py "$compiler"
python3 tools/test-cli.py "$compiler"
python3 tools/test-files.py "$compiler"
python3 tools/test-ffi.py "$compiler"
python3 tools/test-native.py "$compiler"
python3 tools/runtime-contracts.py "$compiler"
node tools/test-wasm.mjs "$compiler"
node tools/test-wasm-scalars.mjs "$compiler"
node tools/test-wasm-abi.mjs "$compiler"
python3 tools/runtime-contracts.py "$compiler" --target wasm32
